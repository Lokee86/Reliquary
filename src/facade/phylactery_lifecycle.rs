use crate::community_store::{CommunityOpenState, CommunityStore};
use crate::compatibility_profile_rebuild::CompatibilityProfileOpenState;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::dream_duplicate_index::DuplicateIndex;
use crate::dream_pair_history::DreamPairStore;
use crate::ego_store::EgoStore;
use crate::entity_rebuild::EntityOpenState;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_rebuild::GraphOpenState;
use crate::graph_store::GraphStore;
use crate::lexical_index::LexicalIndex;
use crate::memory_rebuild::MemoryOpenState;
use crate::memory_store::MemoryStore;
use crate::memory_vector_rebuild::MemoryVectorOpenState;
use crate::memory_vector_store::MemoryVectorStore;
use crate::packed_vector_rebuild::PackedVectorOpenState;
use crate::packed_vector_store::PackedVectorStore;
use crate::phylactery_profile_store::PhylacteryProfileStore;
use crate::processing_epoch_store::ProcessingEpochStore;
use crate::relationship_rebuild::RelationshipOpenState;
use crate::relationship_store::RelationshipStore;
use crate::{Container, ContainerIdentity, FileKind, Phylactery, PhylacteryError};
use std::path::Path;

impl Phylactery {
    pub fn create(path: impl AsRef<Path>) -> Result<Self, PhylacteryError> {
        let container = Container::create_with_identity(
            path,
            ContainerIdentity {
                file_kind: FileKind::Phylactery,
                scope: None,
            },
        )?;
        Self::initialize(container)
    }

    #[cfg(test)]
    pub(crate) fn create_legacy_typed(path: impl AsRef<Path>) -> Result<Self, PhylacteryError> {
        let container = Container::create_with_legacy_identity(
            path,
            ContainerIdentity {
                file_kind: FileKind::Phylactery,
                scope: None,
            },
        )?;
        Self::initialize(container)
    }

    pub(crate) fn create_with_uuid(
        path: impl AsRef<Path>,
        owner_uuid: [u8; 16],
    ) -> Result<Self, PhylacteryError> {
        let container = Container::create_with_identity_and_uuid(
            path,
            ContainerIdentity {
                file_kind: FileKind::Phylactery,
                scope: None,
            },
            owner_uuid,
        )?;
        Self::initialize(container)
    }

    fn initialize(mut container: Container) -> Result<Self, PhylacteryError> {
        let memories = MemoryStore::empty();
        let lexical_index = LexicalIndex::default();
        let mut graph = GraphStore::empty();
        let communities = CommunityStore::default();
        let processing_epochs = ProcessingEpochStore::default();
        let dream_pairs = DreamPairStore::default();
        let packed_vectors = PackedVectorStore::default();
        let memory_vectors = MemoryVectorStore::default();
        let compatibility_profiles = CompatibilityProfileStore::default();
        let profile = PhylacteryProfileStore::default();
        let ego = EgoStore::phylactery();
        let entities = EntityStore::empty();
        let entity_resolutions = EntityResolutionStore::default();
        let relationships = RelationshipStore::empty();

        memories.initialize(&mut container)?;
        graph.initialize(&mut container)?;
        entities.initialize(&mut container)?;
        relationships.initialize(&mut container)?;
        packed_vectors.initialize(&mut container)?;
        memory_vectors.initialize(&mut container)?;
        compatibility_profiles.initialize(&mut container)?;
        container.sync()?;

        Ok(Self {
            container,
            memories,
            lexical_index,
            graph,
            communities,
            duplicate_index: DuplicateIndex::empty(),
            processing_epochs,
            dream_pairs,
            packed_vectors,
            memory_vectors,
            compatibility_profiles,
            profile,
            ego,
            entities,
            entity_resolutions,
            relationships,
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, PhylacteryError> {
        let mut memory_state = MemoryOpenState::new();
        let mut graph_state = GraphOpenState::new();
        let mut community_state = CommunityOpenState::new();
        let mut packed_state = PackedVectorOpenState::new();
        let mut memory_vector_state = MemoryVectorOpenState::new();
        let mut profile_state = CompatibilityProfileOpenState::new();
        let mut phylactery_profile = PhylacteryProfileStore::default();
        let mut processing_epochs = ProcessingEpochStore::default();
        let mut dream_pairs = DreamPairStore::default();
        let mut ego = EgoStore::phylactery();
        let mut entity_state = EntityOpenState::new();
        let mut entity_resolutions = EntityResolutionStore::default();
        let mut relationship_state = RelationshipOpenState::new();

        let container = Container::open_scanned(path, |chunk, payload, latest_global| {
            reject_rel_only_payload(payload)?;
            memory_state.ingest(chunk, payload, latest_global)?;
            graph_state.ingest(chunk, payload, latest_global)?;
            entity_state.ingest(chunk, payload, latest_global)?;
            relationship_state.ingest(chunk, payload, latest_global)?;
            community_state.ingest(payload)?;
            packed_state.ingest(chunk, payload)?;
            memory_vector_state.ingest(chunk, payload)?;
            profile_state.ingest(chunk, payload)?;
            phylactery_profile
                .ingest(payload)
                .map_err(PhylacteryError::Profile)?;
            processing_epochs.ingest(payload)?;
            dream_pairs.ingest(payload)?;
            ego.ingest(payload)?;
            entity_resolutions.ingest(payload)?;
            Ok::<(), PhylacteryError>(())
        })?;
        if container.identity()
            != Some(ContainerIdentity {
                file_kind: FileKind::Phylactery,
                scope: None,
            })
        {
            return Err(PhylacteryError::InvalidContainerIdentity(
                "file is not a Phylactery",
            ));
        }

        let mut container = container;
        let memories = memory_state.finish(&mut container)?;
        let entities = entity_state.finish()?;
        let relationships = relationship_state.finish();
        relationships.validate_local_references(
            container.owner_id().as_deref(),
            &memories,
            &entities,
        )?;
        if entities
            .records()
            .iter()
            .any(|entity| entity.kind == crate::entity_principal::PRINCIPAL_ENTITY_KIND)
        {
            return Err(crate::EntityError::InvalidField("Phylactery principal Entity").into());
        }
        let lexical_index = LexicalIndex::default();
        ego.validate_memory_version(memories.memory_version())?;
        validate_phylactery_provenance(&memories)?;
        processing_epochs.validate(&memories)?;
        dream_pairs.validate(&memories)?;
        let graph = graph_state.finish(&memories, &entities)?;
        let communities = community_state.finish(&graph, container.owner_uuid())?;
        validate_global_versions(&memories, &entities, &relationships, &graph)?;
        let packed_vectors = packed_state.finish()?;
        let compatibility_profiles = profile_state.finish()?;
        let memory_vectors =
            memory_vector_state.finish(&memories, &compatibility_profiles, &packed_vectors)?;
        let repaired_entity_resolutions = entity_resolutions.repair_retired_entity_references(
            &mut container,
            &memories,
            &entities,
        )?;
        entity_resolutions.validate(&memories, &entities)?;
        if repaired_entity_resolutions > 0 {
            container.sync()?;
        }

        Ok(Self {
            container,
            memories,
            lexical_index,
            graph,
            communities,
            duplicate_index: DuplicateIndex::empty(),
            processing_epochs,
            dream_pairs,
            packed_vectors,
            memory_vectors,
            compatibility_profiles,
            profile: phylactery_profile,
            ego,
            entities,
            entity_resolutions,
            relationships,
        })
    }
}

fn reject_rel_only_payload(payload: &[u8]) -> Result<(), PhylacteryError> {
    const REL_ONLY_PREFIXES: [&[u8; 8]; 26] = [
        b"CVAAFMT2",
        b"CVACONT1",
        b"CVANODE1",
        b"CVABRCH1",
        b"CVAFRAG1",
        b"CVAEPIS1",
        b"CVAFILE1",
        b"CVATURN1",
        b"CVAFMEM1",
        b"CVAAREC1",
        b"CVAINSF1",
        b"CVAINSW1",
        b"CVAINSC5",
        b"CVAINSC4",
        b"CVAINSC3",
        b"CVAINSC2",
        b"CVAINSC1",
        b"CVAINSA1",
        b"CVAAVFM1",
        b"CVAAVEC1",
        b"CVAVGFM2",
        b"CVAVGEN2",
        b"CVAVGRC2",
        b"CVAWKFM1",
        b"CVAWKSP1",
        b"CVAISTR1",
    ];
    if payload.len() >= 8
        && REL_ONLY_PREFIXES
            .iter()
            .any(|prefix| payload[..8] == prefix[..])
    {
        return Err(PhylacteryError::InvalidContainerIdentity(
            "Phylactery contains a Reliquary-only owner record",
        ));
    }
    Ok(())
}

fn validate_phylactery_provenance(memories: &MemoryStore) -> Result<(), PhylacteryError> {
    if memories.records().iter().any(|record| {
        record.source_episode_id.is_some()
            || record.source_node_id.is_some()
            || record.content_source_conversation_id.is_some()
            || record.content_source_node_id.is_some()
            || record.grounding_source_conversation_id.is_some()
            || record.grounding_source_node_id.is_some()
    }) {
        return Err(crate::MemoryError::InvalidProvenance.into());
    }
    Ok(())
}

fn validate_global_versions(
    memories: &MemoryStore,
    entities: &EntityStore,
    relationships: &RelationshipStore,
    graph: &GraphStore,
) -> Result<(), PhylacteryError> {
    let mut versions: Vec<_> = memories
        .records()
        .iter()
        .map(|record| record.global_version)
        .chain(
            entities
                .records()
                .iter()
                .map(|record| record.global_version),
        )
        .chain(
            relationships
                .records()
                .iter()
                .map(|record| record.global_version),
        )
        .chain(graph.transaction_global_versions().iter().copied())
        .collect();
    versions.sort_unstable();
    if let Some(version) = versions
        .windows(2)
        .find(|pair| pair[0] == pair[1])
        .map(|pair| pair[0])
    {
        return Err(PhylacteryError::SemanticGlobalVersionConflict(version));
    }
    Ok(())
}
