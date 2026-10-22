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

pub(crate) struct ReliquaryOwner {
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
