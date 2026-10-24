use super::PhylacteryProfileStore;
use crate::community_store::{CommunityOpenState, CommunityStore};
use crate::compatibility_profile_rebuild::CompatibilityProfileOpenState;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::dream_pair_history::DreamPairStore;
use crate::ego_store::EgoStore;
use crate::entity_rebuild::EntityOpenState;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_rebuild::GraphOpenState;
use crate::graph_store::GraphStore;
use crate::memory_rebuild::MemoryOpenState;
use crate::memory_store::MemoryStore;
use crate::memory_vector_rebuild::MemoryVectorOpenState;
use crate::memory_vector_store::MemoryVectorStore;
use crate::owner_maintenance::OwnerMaintenanceStore;
use crate::packed_vector_rebuild::PackedVectorOpenState;
use crate::packed_vector_store::PackedVectorStore;
use crate::processing_epoch_store::ProcessingEpochStore;
use crate::relationship_rebuild::RelationshipOpenState;
use crate::relationship_store::RelationshipStore;
use crate::rewrite_lineage::RewriteLineageStore;
use crate::{Container, ContainerIdentity, FileKind};
use std::path::{Path, PathBuf};

use super::{OwnerPhylacteryError, read_validation};

/// Replayed durable PHY constituents. Runtime-owned Retrieval and Dream caches
/// remain outside this storage-only owner state.
pub struct PhylacteryOwner {
    pub(crate) owner_maintenance: OwnerMaintenanceStore,
    pub(crate) rewrite_lineage: RewriteLineageStore,
    pub(crate) container: Container,
    pub(crate) memories: MemoryStore,
    pub(crate) graph: GraphStore,
    pub(crate) communities: CommunityStore,
    pub(crate) dream_pairs: DreamPairStore,
    pub(crate) processing_epochs: ProcessingEpochStore,
    pub(crate) packed_vectors: PackedVectorStore,
    pub(crate) memory_vectors: MemoryVectorStore,
    pub(crate) compatibility_profiles: CompatibilityProfileStore,
    pub(crate) profile: PhylacteryProfileStore,
    pub(crate) ego: EgoStore,
    pub(crate) entities: EntityStore,
    pub(crate) entity_resolutions: EntityResolutionStore,
    pub(crate) relationships: RelationshipStore,
}

impl PhylacteryOwner {
    pub(crate) fn owner_id(&self) -> Option<String> {
        self.container.owner_id()
    }

    pub(crate) fn sync(&self) -> Result<(), crate::ContainerError> {
        self.container.sync()
    }

    pub(crate) fn mounted_container_facts(
        &self,
    ) -> Result<
        (
            PathBuf,
            Option<(u64, u64)>,
            Option<FileKind>,
            Option<String>,
        ),
        crate::ContainerError,
    > {
        super::reliquary::mounted_container_facts(&self.container)
    }
}

pub(crate) fn create_phylactery(
    path: impl AsRef<Path>,
) -> Result<PhylacteryOwner, OwnerPhylacteryError> {
    let container = Container::create_with_identity(
        path,
        ContainerIdentity {
            file_kind: FileKind::Phylactery,
            scope: None,
        },
    )?;
    initialize_phylactery(container)
}

pub(crate) fn create_phylactery_with_uuid(
    path: impl AsRef<Path>,
    owner_uuid: [u8; 16],
) -> Result<PhylacteryOwner, OwnerPhylacteryError> {
    let container = Container::create_with_identity_and_uuid(
        path,
        ContainerIdentity {
            file_kind: FileKind::Phylactery,
            scope: None,
        },
        owner_uuid,
    )?;
    initialize_phylactery(container)
}

pub(crate) fn open_phylactery(
    path: impl AsRef<Path>,
) -> Result<PhylacteryOwner, OwnerPhylacteryError> {
    open_phylactery_from_container(Container::open(path)?)
}

pub(crate) fn open_phylactery_read_only(
    path: impl AsRef<Path>,
) -> Result<PhylacteryOwner, OwnerPhylacteryError> {
    open_phylactery_from_container(Container::open_read_only(path)?)
}

pub(crate) fn initialize_phylactery(
    mut container: Container,
) -> Result<PhylacteryOwner, OwnerPhylacteryError> {
    let owner_maintenance = OwnerMaintenanceStore::default();
    let rewrite_lineage = RewriteLineageStore::default();
    let memories = MemoryStore::empty();
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

    Ok(PhylacteryOwner {
        owner_maintenance,
        rewrite_lineage,
        container,
        memories,
        graph,
        communities,
        dream_pairs,
        processing_epochs,
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

pub(crate) fn open_phylactery_from_container(
    mut container: Container,
) -> Result<PhylacteryOwner, OwnerPhylacteryError> {
    if container.owner_uuid().is_none() {
        return Err(crate::ContainerError::InvalidIdentity.into());
    }
    if container.identity().map(|identity| identity.file_kind) != Some(FileKind::Phylactery) {
        return Err(OwnerPhylacteryError::InvalidContainerIdentity(
            "file is not a Phylactery",
        ));
    }
    let lineage_owner = container.owner_uuid();
    let mut owner_maintenance = OwnerMaintenanceStore::default();
    let mut rewrite_lineage = RewriteLineageStore::default();
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

    {
        let mut cursor = container.physical_cursor()?;
        while let Some(frame) = cursor.next_frame()? {
            let chunk = frame.object;
            let payload = frame.payload.as_slice();
            let latest_global = frame.latest_global;
            if let Some(owner_uuid) = lineage_owner {
                rewrite_lineage
                    .ingest(payload, owner_uuid)
                    .map_err(OwnerPhylacteryError::Repack)?;
            } else if payload.starts_with(crate::rewrite_lineage::REWRITE_GENESIS_MAGIC) {
                return Err(OwnerPhylacteryError::Repack(
                    "RewriteGenesis requires a versioned owner identity".into(),
                ));
            }
            reject_rel_only_payload(payload)?;
            owner_maintenance
                .ingest(payload)
                .map_err(OwnerPhylacteryError::Repack)?;
            if let Some(envelope) = crate::container::publication_envelope::decode(payload)? {
                if !memory_state.ingest_publication_envelope(envelope, latest_global)?
                    && !entity_state.ingest_publication_envelope(envelope, latest_global)?
                    && !relationship_state.ingest_publication_envelope(envelope, latest_global)?
                    && !graph_state.ingest_publication_envelope(envelope, latest_global)?
                {
                    return Err(crate::MemoryError::CorruptRecord(
                        "unsupported publication envelope body",
                    )
                    .into());
                }
            } else {
                crate::owner_lifecycle::memory_frames::ingest(
                    &mut memory_state,
                    chunk,
                    payload,
                    latest_global,
                )?;
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
                .map_err(OwnerPhylacteryError::Profile)?;
            processing_epochs.ingest(payload)?;
            dream_pairs.ingest(payload)?;
            ego.ingest(payload)?;
            entity_resolutions.ingest(payload)?;
        }
        let clock_state = cursor.finish_open_scan()?;
        container.apply_scan_clock(clock_state);
    }
    if container.identity()
        != Some(ContainerIdentity {
            file_kind: FileKind::Phylactery,
            scope: None,
        })
    {
        return Err(OwnerPhylacteryError::InvalidContainerIdentity(
            "file is not a Phylactery",
        ));
    }

    let memories = memory_state.finish(&mut container)?;
    let entities = entity_state.finish(&container)?;
    let relationships = relationship_state.finish(&container)?;
    read_validation::validate_current_relationship_references(
        container.owner_id().as_deref(),
        &memories,
        &entities,
        &relationships.current(),
    )?;
    if entities
        .records()
        .iter()
        .any(|entity| entity.kind == crate::entity_principal_identity::PRINCIPAL_ENTITY_KIND)
    {
        return Err(crate::EntityError::InvalidField("Phylactery principal Entity").into());
    }

    ego.validate_memory_version(memories.memory_version())?;
    read_validation::validate_phylactery_provenance(&memories)?;
    processing_epochs.validate(&memories)?;
    read_validation::validate_dream_pairs(&dream_pairs, &memories)?;
    let mut graph = graph_state.finish()?;
    read_validation::validate_rebuilt_graph(&mut graph, &memories, &entities)?;
    graph.validate_rewrite_baseline(container.owner_uuid(), rewrite_lineage.rewrite_genesis())?;
    let communities = community_state.finish(&graph, container.owner_uuid())?;
    read_validation::validate_phylactery_global_versions(
        &memories,
        &entities,
        &relationships,
        &graph,
    )?;
    let packed_vectors = packed_state.finish()?;
    let compatibility_profiles = profile_state.finish()?;
    let accepted_bodies = memory_vector_state
        .referenced_body_ids()
        .filter(|body| memories.contains_body(*body))
        .collect();
    let memory_vectors =
        memory_vector_state.finish(&accepted_bodies, &compatibility_profiles, &packed_vectors)?;

    Ok(PhylacteryOwner {
        owner_maintenance,
        rewrite_lineage,
        container,
        memories,
        graph,
        communities,
        dream_pairs,
        processing_epochs,
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

fn reject_rel_only_payload(payload: &[u8]) -> Result<(), OwnerPhylacteryError> {
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
        return Err(OwnerPhylacteryError::InvalidContainerIdentity(
            "Phylactery contains a Reliquary-only owner record",
        ));
    }
    Ok(())
}

impl PhylacteryOwner {
    pub(crate) fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.container.owner_uuid()
    }
    pub(crate) fn latest_global_version(&self) -> u64 {
        self.container.latest_version()
    }
    pub(crate) fn history_source(
        &self,
    ) -> Result<super::HistorySource<'_>, super::HistoryReadFailure> {
        super::HistorySource::new(&self.container)
    }
}
