//! Concrete REL owner constituents and ordered empty-owner initialization.
use super::OwnerError;
use crate::archive_vector_store::ArchiveVectorStore;
use crate::community_store::CommunityStore;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::conversation_compaction_store::ConversationCompactionStore;
use crate::dream_pair_history::DreamPairStore;
use crate::echo_store::EchoStore;
use crate::ego_store::EgoStore;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::freshness_storage::FreshnessStore;
use crate::graph_store::GraphStore;
use crate::insomnia::store::InsomniaStore;
use crate::interaction_stream_store::InteractionStreamStore;
use crate::memory_store::MemoryStore;
use crate::memory_vector_store::MemoryVectorStore;
use crate::packed_vector_store::PackedVectorStore;
use crate::processing_epoch_store::ProcessingEpochStore;
use crate::project_file_binding_store::ProjectFileStore;
use crate::project_history_store::ProjectHistoryStore;
use crate::rel_metadata_store::RelMetadataStore;
use crate::relationship_store::RelationshipStore;
use crate::vector_generation_store::VectorGenerationStore;
use crate::{Archive, Container};
use std::path::{Path, PathBuf};

/// Concrete mutable REL owner state shared by the public Cva adapter and
/// mounted runtime execution. Runtime code depends on this lower owner type.
pub struct ReliquaryOwner {
    pub(crate) owner_maintenance: crate::owner_maintenance::OwnerMaintenanceStore,
    pub(crate) rewrite_lineage: crate::rewrite_lineage::RewriteLineageStore,
    pub(crate) external_index: crate::external_index::store::ExternalIndexStore,
    pub(crate) container: Container,
    pub(crate) archive: Archive,
    pub(crate) memories: MemoryStore,
    pub(crate) graph: GraphStore,
    pub(crate) communities: CommunityStore,
    pub(crate) processing_epochs: ProcessingEpochStore,
    pub(crate) dream_pairs: DreamPairStore,
    pub(crate) insomnia: InsomniaStore,
    pub(crate) import_epochs: crate::imports::ImportEpochStore,
    pub(crate) packed_vectors: PackedVectorStore,
    pub(crate) memory_vectors: MemoryVectorStore,
    pub(crate) archive_vectors: ArchiveVectorStore,
    pub(crate) compatibility_profiles: CompatibilityProfileStore,
    pub(crate) vector_generations: VectorGenerationStore,
    pub(crate) conversation_compactions: ConversationCompactionStore,
    pub(crate) interaction_streams: InteractionStreamStore,
    pub(crate) echo: EchoStore,
    pub(crate) ego: EgoStore,
    pub(crate) entities: EntityStore,
    pub(crate) entity_resolutions: EntityResolutionStore,
    pub(crate) relationships: RelationshipStore,
    pub(crate) project_history: ProjectHistoryStore,
    pub(crate) project_files: ProjectFileStore,
    pub(crate) rel_metadata: RelMetadataStore,
    pub(crate) freshness: crate::freshness_storage::FreshnessStore,
    pub(crate) dream_freshness: crate::freshness_dream_journal::DreamFreshnessJournal,
}

impl ReliquaryOwner {
    pub(crate) fn owner_id(&self) -> Option<String> {
        self.container.owner_id()
    }

    pub(crate) fn sync(&self) -> Result<(), crate::ContainerError> {
        self.container.sync()
    }

    pub(crate) fn backing_len(&self) -> Result<u64, std::io::Error> {
        self.container
            .backing_metadata()
            .map(|metadata| metadata.len())
    }

    pub(crate) fn mounted_container_facts(
        &self,
    ) -> Result<
        (
            PathBuf,
            Option<(u64, u64)>,
            Option<crate::FileKind>,
            Option<String>,
        ),
        crate::ContainerError,
    > {
        mounted_container_facts(&self.container)
    }

    pub(crate) fn inspect_mounted_container(
        path: &Path,
    ) -> Result<
        (
            PathBuf,
            Option<(u64, u64)>,
            Option<crate::FileKind>,
            Option<String>,
        ),
        crate::ContainerError,
    > {
        let canonical_path = std::fs::canonicalize(path)?;
        let mut file = std::fs::File::open(&canonical_path)?;
        let (identity, owner_id) = Container::inspect_header(&mut file)?;
        Ok((
            canonical_path,
            Container::inspect_file_identity(&file)?,
            identity.map(|identity| identity.file_kind),
            owner_id,
        ))
    }

    pub(crate) fn persist_publication_intent(
        &mut self,
        id: crate::MemoryId,
        mutation_id: &str,
        turn: u64,
        admit_at_birth: bool,
    ) -> Result<(), OwnerError> {
        let payload = self
            .dream_freshness
            .publication_intent_payload(id, mutation_id, turn, admit_at_birth)
            .map_err(OwnerError::Freshness)?;
        if !payload.is_empty() {
            self.container.append(&payload)?;
            self.container.sync()?;
            self.dream_freshness
                .ingest(&payload)
                .map_err(OwnerError::Freshness)?;
        }
        Ok(())
    }

    pub(crate) fn persist_accepted_freshness_birth(
        &mut self,
        id: crate::MemoryId,
        turn: u64,
        admit_at_birth: bool,
    ) -> Result<(), OwnerError> {
        let owner = self.owner_uuid().ok_or_else(|| {
            OwnerError::Freshness(
                "legacy owner requires explicit migration before enrollment".into(),
            )
        })?;
        let mut record = crate::freshness::FreshnessRecord::created();
        if admit_at_birth {
            record
                .admit(turn)
                .map_err(|error| OwnerError::Freshness(error.to_string()))?;
        }
        self.freshness
            .persist_policy_if_needed(&mut self.container, owner)
            .map_err(OwnerError::Freshness)?;
        let payload = self.freshness.encode_initialize(owner, id, turn, record);
        self.container.append(&payload)?;
        self.container.sync()?;
        self.freshness
            .ingest_appended(&payload)
            .map_err(OwnerError::Freshness)
    }

    pub(crate) fn consume_publication_intent(
        &mut self,
        id: crate::MemoryId,
    ) -> Result<(), OwnerError> {
        if let Some(payload) = self
            .dream_freshness
            .consume_publication_intent_payload(id)
            .map_err(OwnerError::Freshness)?
        {
            self.container.append(&payload)?;
            self.container.sync()?;
            self.dream_freshness
                .ingest(&payload)
                .map_err(OwnerError::Freshness)?;
        }
        Ok(())
    }

    pub(crate) fn put_memory_vectors(
        &mut self,
        compatibility_profile_id: crate::CompatibilityProfileId,
        packed_vector_id: crate::PackedVectorId,
        memory_body_ids: Vec<crate::MemoryBodyId>,
    ) -> Result<crate::MemoryVectorId, crate::MemoryVectorError> {
        self.compatibility_profiles
            .get_active(compatibility_profile_id)
            .ok_or(crate::MemoryVectorError::MissingProfile)?;
        crate::memory_vector_store::validate_mapping_shape(
            &self.compatibility_profiles,
            &self.packed_vectors,
            compatibility_profile_id,
            packed_vector_id,
            &memory_body_ids,
        )?;
        let mut seen = std::collections::HashSet::new();
        for body in &memory_body_ids {
            if !self.memories.contains_body(*body) {
                return Err(crate::MemoryVectorError::MissingMemoryBody);
            }
            if !seen.insert(*body) {
                return Err(crate::MemoryVectorError::DuplicateMemoryBody);
            }
        }
        self.memory_vectors.put(
            &mut self.container,
            &self.compatibility_profiles,
            &self.packed_vectors,
            compatibility_profile_id,
            packed_vector_id,
            memory_body_ids,
        )
    }
}

pub(super) fn mounted_container_facts(
    container: &Container,
) -> Result<
    (
        PathBuf,
        Option<(u64, u64)>,
        Option<crate::FileKind>,
        Option<String>,
    ),
    crate::ContainerError,
> {
    let canonical_path = std::fs::canonicalize(container.path())?;
    Ok((
        canonical_path,
        container.backing_file_identity()?,
        container.identity().map(|identity| identity.file_kind),
        container.owner_id(),
    ))
}

pub(crate) fn initialize_reliquary(mut container: Container) -> Result<ReliquaryOwner, OwnerError> {
    let owner_maintenance = crate::owner_maintenance::OwnerMaintenanceStore::default();
    let rewrite_lineage = crate::rewrite_lineage::RewriteLineageStore::default();
    let archive = Archive::empty();
    let memories = MemoryStore::empty();
    let mut graph = GraphStore::empty();
    let communities = CommunityStore::default();
    let processing_epochs = ProcessingEpochStore::default();
    let dream_pairs = DreamPairStore::default();
    let insomnia = InsomniaStore::empty();
    let import_epochs = crate::imports::ImportEpochStore::default();
    let packed_vectors = PackedVectorStore::default();
    let memory_vectors = MemoryVectorStore::default();
    let archive_vectors = ArchiveVectorStore::default();
    let compatibility_profiles = CompatibilityProfileStore::default();
    let vector_generations = VectorGenerationStore::empty();
    let interaction_streams = InteractionStreamStore::default();
    let conversation_compactions = ConversationCompactionStore::empty();
    let echo = EchoStore::default();
    let ego = EgoStore::reliquary();
    let entities = EntityStore::empty();
    let entity_resolutions = EntityResolutionStore::default();
    let relationships = RelationshipStore::empty();
    let project_history = ProjectHistoryStore::default();
    let project_files = ProjectFileStore::default();
    let rel_metadata = RelMetadataStore::default();
    let mut freshness = FreshnessStore::new(container.owner_uuid());
    freshness
        .bind_history_source(&container, 0)
        .map_err(OwnerError::Freshness)?;
    let dream_freshness = DreamFreshnessJournal::new(container.owner_uuid());
    archive.initialize_history_format(&mut container)?;
    memories.initialize(&mut container)?;
    graph.initialize(&mut container)?;
    entities.initialize(&mut container)?;
    relationships.initialize(&mut container)?;
    insomnia.initialize(&mut container)?;
    packed_vectors.initialize(&mut container)?;
    memory_vectors.initialize(&mut container)?;
    archive_vectors.initialize(&mut container)?;
    compatibility_profiles.initialize(&mut container)?;
    vector_generations.initialize(&mut container)?;
    container.sync()?;
    Ok(ReliquaryOwner {
        owner_maintenance,
        rewrite_lineage,
        external_index: crate::external_index::store::ExternalIndexStore::default(),
        container,
        archive,
        memories,
        processing_epochs,
        dream_pairs,
        graph,
        communities,
        insomnia,
        import_epochs,
        packed_vectors,
        memory_vectors,
        archive_vectors,
        compatibility_profiles,
        vector_generations,
        interaction_streams,
        conversation_compactions,
        echo,
        ego,
        entities,
        entity_resolutions,
        relationships,
        project_history,
        project_files,
        rel_metadata,
        freshness,
        dream_freshness,
    })
}
