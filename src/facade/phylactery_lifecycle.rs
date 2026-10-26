use crate::community_owner::{CommunityOwner, CommunityReplay};
use crate::compatibility_profile_rebuild::CompatibilityProfileOpenState;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::dream_duplicate_index::DuplicateIndex;
use crate::dream_pair_history::DreamPairStore;
use crate::ego_store::EgoStore;
use crate::entity_rebuild::EntityOpenState;
use crate::entity_resolution_boundary::ResolutionAdmission;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_owner::GraphOwner;
use crate::graph_replay::GraphReplay;
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
use crate::{Phylactery, PhylacteryError};
use reliquary_container::{Container, ContainerIdentity, FileKind};
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
        let owner_maintenance = crate::owner_maintenance::OwnerMaintenanceStore::default();
        let rewrite_lineage = crate::rewrite_lineage::RewriteLineageStore::default();
        let memories = MemoryStore::empty();
        let lexical_index = LexicalIndex::default();
        let mut graph = GraphOwner::empty();
        let communities = CommunityOwner::default();
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
            owner_maintenance,
            rewrite_lineage,
            external_publications:
                crate::external_source::publication::ExternalPublicationState::default(),
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
        let header = Container::open_read_only_for_scan(path.as_ref())?;
        if header.owner_uuid().is_none() {
            return Err(reliquary_container::ContainerError::InvalidIdentity.into());
        }
        if header.identity().map(|identity| identity.file_kind)
            != Some(reliquary_container::FileKind::Phylactery)
        {
            return Err(PhylacteryError::InvalidContainerIdentity(
                "file is not a Phylactery",
            ));
        }
        let lineage_owner = header.owner_uuid();
        drop(header);
        let mut owner_maintenance = crate::owner_maintenance::OwnerMaintenanceStore::default();
        let mut rewrite_lineage = crate::rewrite_lineage::RewriteLineageStore::default();
        let mut memory_state = MemoryOpenState::new();
        let mut external_publications =
            crate::external_source::publication::ExternalPublicationState::default();
        let mut graph_state = GraphReplay::new();
        let mut community_state = CommunityReplay::new();
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

        let mut container = Container::open_for_replay(path)?;
        {
            let mut replay = |chunk, payload: &[u8], latest_global| {
                if let Some(owner_uuid) = lineage_owner {
                    rewrite_lineage
                        .ingest(payload, owner_uuid)
                        .map_err(PhylacteryError::Repack)?;
                } else if payload.starts_with(crate::rewrite_lineage::REWRITE_GENESIS_MAGIC) {
                    return Err(PhylacteryError::Repack(
                        "RewriteGenesis requires a versioned owner identity".into(),
                    ));
                }
                reject_rel_only_payload(payload)?;
                owner_maintenance
                    .ingest(payload)
                    .map_err(PhylacteryError::Repack)?;
                if let Some(envelope) = reliquary_container::publication_envelope::decode(payload)?
                {
                    if !memory_state.ingest_publication_envelope(envelope, latest_global)?
                        && !entity_state.ingest_publication_envelope(envelope, latest_global)?
                        && !relationship_state
                            .ingest_publication_envelope(envelope, latest_global)?
                        && !graph_state.ingest_publication_envelope(envelope, latest_global)?
                    {
                        return Err(crate::MemoryError::CorruptRecord(
                            "unsupported publication envelope body",
                        )
                        .into());
                    }
                } else {
                    if let Some((completion, body_spans)) =
                        crate::insomnia::completion::decode_completion_with_body_spans(payload)
                            .map_err(crate::MemoryError::CorruptRecord)?
                    {
                        memory_state.ingest_grouped_batch(
                            chunk,
                            payload,
                            crate::memory_rebuild::MemoryGroupedBatch::new(
                                completion.records,
                                body_spans,
                                completion.routing_metadata,
                            ),
                            latest_global,
                        )?;
                    } else if let Some(publication) =
                        crate::external_source::publication::decode(payload)?
                    {
                        memory_state.ingest_grouped_batch(
                            chunk,
                            payload,
                            crate::memory_rebuild::MemoryGroupedBatch::new(
                                publication.records.clone(),
                                publication.body_spans.clone(),
                                publication.routing.clone(),
                            ),
                            latest_global,
                        )?;
                        external_publications.accept(&publication)?;
                    } else {
                        memory_state.ingest(chunk, payload, latest_global)?;
                    }
                    entity_state.ingest(chunk, payload, latest_global)?;
                    graph_state.ingest(chunk, payload, latest_global)?;
                    relationship_state.ingest(chunk, payload, latest_global)?;
                }
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
            };
            let mut cursor = container.physical_cursor()?;
            while let Some(frame) = cursor.next_frame()? {
                replay(frame.object, &frame.payload, frame.latest_global)?;
            }
            let state = cursor.finish_open_scan()?;
            container.accept_replay_scan(state)?;
        }
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

        let memories = memory_state.finish(&mut container)?;
        let entities = entity_state.finish(&container)?;
        let relationships = relationship_state.finish(&container)?;
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
        graph
            .validate_rewrite_baseline(container.owner_uuid(), rewrite_lineage.rewrite_genesis())?;
        let communities = community_state.finish(&graph, container.owner_uuid())?;
        validate_global_versions(&memories, &entities, &relationships, &graph)?;
        let packed_vectors = packed_state.finish()?;
        let compatibility_profiles = profile_state.finish()?;
        let memory_vectors =
            memory_vector_state.finish(&memories, &compatibility_profiles, &packed_vectors)?;
        let mut result = Self {
            owner_maintenance,
            rewrite_lineage,
            external_publications,
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
        };
        result.validate_prune_evidence()?;
        result
            .entity_resolutions
            .owner_validate(&result.memories, &result.entities)?;
        Ok(result)
    }

    /// Explicitly canonicalize Entity-resolution references after entity retirement.
    /// Opening a REL/PHY never performs this durable maintenance automatically.
    pub fn repair_retired_entity_references(&mut self) -> Result<usize, PhylacteryError> {
        let repaired = self
            .entity_resolutions
            .owner_repair_retired_entity_references(
                &mut self.container,
                &self.memories,
                &self.entities,
            )?;
        self.entity_resolutions
            .owner_validate(&self.memories, &self.entities)?;
        if repaired > 0 {
            self.container.sync()?;
        }
        Ok(repaired)
    }
}

fn reject_rel_only_payload(payload: &[u8]) -> Result<(), PhylacteryError> {
    const REL_ONLY_PREFIXES: [&[u8; 8]; 31] = [
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
        b"CVAINSC8",
        b"CVAINSC7",
        b"CVAINSC6",
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
        b"CVAISTR2",
        b"CVAXIDX2",
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
    graph: &GraphOwner,
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
        .chain(memories.tombstone_global_versions())
        .chain(entities.tombstone_global_versions())
        .chain(relationships.tombstone_global_versions())
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
