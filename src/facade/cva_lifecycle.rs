use crate::archive_rebuild::ArchiveOpenState;
use crate::archive_vector_rebuild::ArchiveVectorOpenState;
use crate::archive_vector_store::ArchiveVectorStore;
use crate::community_store::{CommunityOpenState, CommunityStore};
use crate::compatibility_profile_rebuild::CompatibilityProfileOpenState;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::conversation_compaction_store::{
    ConversationCompactionOpenState, ConversationCompactionStore,
};
use crate::cva_global_validation::validate_semantic_global_versions;
use crate::cva_open_route::{OpenRecordRoute, classify_open_record};
use crate::dream_duplicate_index::DuplicateIndex;
use crate::dream_pair_history::DreamPairStore;
use crate::echo_store::EchoStore;
use crate::ego_store::EgoStore;
use crate::entity_rebuild::EntityOpenState;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::file_memory_link_store::validate_file_memory_targets;
use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::freshness_storage::FreshnessStore;
use crate::graph_rebuild::GraphOpenState;
use crate::graph_store::GraphStore;
use crate::insomnia::rebuild::InsomniaOpenState;
use crate::insomnia::store::InsomniaStore;
use crate::interaction_stream_store::InteractionStreamStore;
use crate::lexical_index::LexicalIndex;
use crate::memory_rebuild::MemoryOpenState;
use crate::memory_store::MemoryStore;
use crate::memory_vector_rebuild::MemoryVectorOpenState;
use crate::memory_vector_store::MemoryVectorStore;
use crate::packed_vector_rebuild::PackedVectorOpenState;
use crate::packed_vector_store::PackedVectorStore;
use crate::processing_epoch_store::ProcessingEpochStore;
use crate::project_file_binding_store::ProjectFileStore;
use crate::project_history_store::ProjectHistoryStore;
use crate::rel_metadata_store::RelMetadataStore;
use crate::relationship_rebuild::RelationshipOpenState;
use crate::relationship_store::RelationshipStore;
use crate::vector_generation_rebuild::VectorGenerationOpenState;
use crate::vector_generation_store::VectorGenerationStore;
use crate::{Archive, Container, Cva, CvaError};
use std::path::Path;

impl Cva {
    pub fn create(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::create_rel(path, None)
    }

    pub fn create_project(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::create_rel(path, Some("Project"))
    }

    pub fn create_organization(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::create_rel(path, Some("Organization"))
    }

    pub fn create_connection(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::create_rel(path, Some("Connection"))
    }

    pub fn open_project(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::open(path)
    }

    pub fn open_organization(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::open(path)
    }

    pub fn open_connection(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::open(path)
    }

    fn create_rel(path: impl AsRef<Path>, type_label: Option<&str>) -> Result<Self, CvaError> {
        let container = Container::create_with_identity(
            path,
            crate::ContainerIdentity {
                file_kind: crate::FileKind::Reliquary,
                scope: None,
            },
        )?;
        let mut cva = Self::initialize(container)?;
        if let Some(type_label) = type_label {
            cva.set_rel_metadata(Some(type_label.to_owned()), Vec::new())?;
            cva.sync()?;
        }
        Ok(cva)
    }

    pub(crate) fn create_rel_with_uuid(
        path: impl AsRef<Path>,
        owner_uuid: [u8; 16],
        type_label: Option<String>,
    ) -> Result<Self, CvaError> {
        let container = Container::create_with_identity_and_uuid(
            path,
            crate::ContainerIdentity {
                file_kind: crate::FileKind::Reliquary,
                scope: None,
            },
            owner_uuid,
        )?;
        let mut cva = Self::initialize(container)?;
        if type_label.is_some() {
            cva.set_rel_metadata(type_label, Vec::new())?;
        }
        Ok(cva)
    }

    pub(crate) fn create_legacy_scope_with_uuid(
        path: impl AsRef<Path>,
        scope: crate::ReliquaryScopeKind,
        owner_uuid: [u8; 16],
    ) -> Result<Self, CvaError> {
        let container = Container::create_with_identity_and_uuid(
            path,
            crate::ContainerIdentity {
                file_kind: crate::FileKind::Reliquary,
                scope: Some(scope),
            },
            owner_uuid,
        )?;
        Self::initialize(container)
    }

    #[cfg(test)]
    pub(crate) fn create_legacy_cva(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::initialize(Container::create(path)?)
    }

    #[cfg(test)]
    pub(crate) fn create_legacy_typed(
        path: impl AsRef<Path>,
        scope: crate::ReliquaryScopeKind,
    ) -> Result<Self, CvaError> {
        Self::initialize(Container::create_with_legacy_identity(
            path,
            crate::ContainerIdentity {
                file_kind: crate::FileKind::Reliquary,
                scope: Some(scope),
            },
        )?)
    }

    fn initialize(mut container: Container) -> Result<Self, CvaError> {
        let archive = Archive::empty();
        let memories = MemoryStore::empty();
        let mut graph = GraphStore::empty();
        let communities = CommunityStore::default();
        let processing_epochs = ProcessingEpochStore::default();
        let dream_pairs = DreamPairStore::default();
        let insomnia = InsomniaStore::empty();
        let lexical_index = LexicalIndex::default();
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
        freshness.bind_history_path(container.path(), 0);
        let dream_freshness = DreamFreshnessJournal::new(container.owner_uuid());
        let freshness_due = crate::FreshnessDueDispatcher::default();
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
        Ok(Self {
            container,
            archive,
            memories,
            duplicate_index: DuplicateIndex::empty(),
            processing_epochs,
            dream_pairs,
            graph,
            communities,
            insomnia,
            lexical_index,
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
            freshness_due,
            freshness_notifications: std::collections::BTreeMap::new(),
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        let mut archive_state = ArchiveOpenState::new();
        let mut memory_state = MemoryOpenState::new();
        let mut graph_state = GraphOpenState::new();
        let mut community_state = CommunityOpenState::new();
        let mut insomnia_state = InsomniaOpenState::new();
        let mut packed_state = PackedVectorOpenState::new();
        let mut memory_vector_state = MemoryVectorOpenState::new();
        let mut archive_vector_state = ArchiveVectorOpenState::new();
        let mut profile_state = CompatibilityProfileOpenState::new();
        let mut generation_state = VectorGenerationOpenState::new();
        let mut interaction_streams = InteractionStreamStore::default();
        let mut compaction_state = ConversationCompactionOpenState::default();
        let mut echo = EchoStore::default();
        let mut ego = EgoStore::reliquary();
        let mut entity_state = EntityOpenState::new();
        let mut entity_resolutions = EntityResolutionStore::default();
        let mut relationship_state = RelationshipOpenState::new();
        let mut project_history = ProjectHistoryStore::default();
        let mut project_files = ProjectFileStore::default();
        let mut rel_metadata = RelMetadataStore::default();
        let mut freshness = FreshnessStore::default();
        freshness.bind_history_path(path.as_ref(), 0);
        let mut dream_freshness = DreamFreshnessJournal::default();
        let mut processing_epochs = ProcessingEpochStore::default();
        let mut dream_pairs = DreamPairStore::default();
        let mut container = Container::open_scanned(path, |chunk, payload, latest_global| {
            match classify_open_record(payload) {
                OpenRecordRoute::Archive => {
                    archive_state.ingest(chunk, payload, latest_global)?;
                }
                OpenRecordRoute::Memory => {
                    memory_state.ingest(chunk, payload, latest_global)?;
                }
                OpenRecordRoute::Graph => {
                    graph_state.ingest(chunk, payload, latest_global)?;
                }
                OpenRecordRoute::ContainerVersion => {}
                OpenRecordRoute::Fallback => {
                    archive_state.ingest(chunk, payload, latest_global)?;
                    memory_state.ingest(chunk, payload, latest_global)?;
                    graph_state.ingest(chunk, payload, latest_global)?;
                    entity_state.ingest(chunk, payload, latest_global)?;
                    relationship_state.ingest(chunk, payload, latest_global)?;
                    community_state.ingest(payload)?;
                    insomnia_state.ingest(chunk, payload)?;
                    packed_state.ingest(chunk, payload)?;
                    memory_vector_state.ingest(chunk, payload)?;
                    archive_vector_state.ingest(chunk, payload)?;
                    profile_state.ingest(chunk, payload)?;
                    generation_state.ingest(chunk, payload, latest_global)?;
                    interaction_streams.ingest(payload)?;
                    compaction_state.ingest(chunk, payload)?;
                    echo.ingest(payload)?;
                    ego.ingest(payload)?;
                    entity_resolutions.ingest(payload)?;
                    project_history.ingest(payload)?;
                    project_files.ingest(payload)?;
                    rel_metadata
                        .ingest(payload)
                        .map_err(CvaError::RelMetadata)?;
                    if let Some(completion) =
                        crate::insomnia::completion::decode_completion(payload)
                            .map_err(|message| CvaError::Freshness(message.into()))?
                        && completion.freshness_birth_turn.is_some()
                        && !completion.freshness_birth_memory_ids.is_empty()
                    {
                        freshness.freeze_derived_birth_policy();
                    }
                    if payload.starts_with(b"CVAFRP01") {
                        freshness.ingest(payload).map_err(CvaError::Freshness)?;
                    }
                    dream_freshness
                        .ingest(payload)
                        .map_err(CvaError::Freshness)?;
                    processing_epochs.ingest(payload)?;
                    dream_pairs.ingest(payload)?;
                }
            }
            Ok::<(), CvaError>(())
        })?;
        if let Some(identity) = container.identity()
            && identity.file_kind != crate::FileKind::Reliquary
        {
            return Err(CvaError::InvalidContainerIdentity(
                "file is not a Reliquary",
            ));
        }
        freshness
            .bind_owner(container.owner_uuid())
            .map_err(CvaError::Freshness)?;
        dream_freshness
            .bind_owner(container.owner_uuid())
            .map_err(CvaError::Freshness)?;
        let archive = archive_state.finish()?;
        archive.validate_references(&project_files)?;
        let memories = memory_state.finish(&mut container)?;
        let entities = entity_state.finish()?;
        let relationships = relationship_state.finish();
        relationships.validate_local_references(
            container.owner_id().as_deref(),
            &memories,
            &entities,
        )?;
        ego.validate_memory_version(memories.memory_version())?;
        memories.validate_provenance(&archive)?;
        processing_epochs.validate(&memories)?;
        dream_pairs.validate(&memories)?;
        validate_file_memory_targets(&archive, &memories)?;
        let graph = graph_state.finish(&memories, &entities)?;
        let communities = community_state.finish(&graph, container.owner_uuid())?;
        let mut insomnia = insomnia_state.finish()?;
        insomnia.validate(&archive, &memories)?;
        insomnia.rebuild_schedule(&archive)?;
        freshness.begin_rebuild();
        if let Some(owner) = container.owner_uuid() {
            for (id, mutation, turn, admitted) in dream_freshness.pending_initializations() {
                if memories.first_mutation_id(id) == Some(mutation.as_str()) {
                    let first_global = memories.first_global_version(id).ok_or_else(|| {
                        CvaError::Freshness("birth lacks accepted first Memory revision".into())
                    })?;
                    let archive_version = archive
                        .record_versions()
                        .iter()
                        .filter(|record| record.global_version < first_global)
                        .map(|record| record.archive_version)
                        .max()
                        .unwrap_or(0);
                    if turn != archive.activity_cut_at_archive_version(archive_version)? {
                        return Err(CvaError::Freshness(
                            "publication birth differs from accepted first Memory activity cut"
                                .into(),
                        ));
                    }
                    freshness
                        .derive_birth(owner, id, turn, admitted)
                        .map_err(CvaError::Freshness)?;
                } else if memories.contains_memory(id) {
                    return Err(CvaError::Freshness(
                        "publication intent differs from accepted first Memory mutation".into(),
                    ));
                }
            }
            container.visit_payloads::<CvaError>(|object, payload| {
                let location = object.legacy_bytes();
                let end = u64::from_le_bytes(location[..8].try_into().unwrap())
                    + 8
                    + u64::from_le_bytes(location[8..].try_into().unwrap());
                if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
                    .map_err(|message| CvaError::Freshness(message.into()))?
                    && let Some(turn) = completion.freshness_birth_turn
                {
                    if completion.freshness_policy_version
                        != Some(crate::freshness::FRESHNESS_POLICY_VERSION as u64)
                        || turn > archive.rel_turn_count()
                    {
                        return Err(CvaError::Freshness(
                            "grouped birth proof has invalid policy or activity cut".into(),
                        ));
                    }
                    if !completion.freshness_birth_memory_ids.is_empty() {
                        let archive_version = archive
                            .record_versions()
                            .iter()
                            .filter(|record| {
                                record.global_version < completion.global_version_start
                            })
                            .map(|record| record.archive_version)
                            .max()
                            .unwrap_or(0);
                        if turn != archive.activity_cut_at_archive_version(archive_version)? {
                            return Err(CvaError::Freshness(
                                "grouped birth differs from accepted completion activity cut"
                                    .into(),
                            ));
                        }
                    }
                    for id in completion.freshness_birth_memory_ids {
                        let record = completion
                            .records
                            .iter()
                            .find(|record| record.id == id && record.revision == 1)
                            .ok_or_else(|| {
                                CvaError::Freshness(
                                    "grouped birth proof lacks accepted first Memory record".into(),
                                )
                            })?;
                        if memories.first_mutation_id(id) != Some(record.mutation_id.as_str()) {
                            return Err(CvaError::Freshness(
                                "grouped birth differs from accepted first Memory identity".into(),
                            ));
                        }
                        freshness
                            .derive_birth(owner, id, turn, false)
                            .map_err(CvaError::Freshness)?;
                        dream_freshness.derive_publication_birth(id);
                    }
                }
                freshness
                    .ingest_scanned(payload, end)
                    .map_err(CvaError::Freshness)?;
                Ok(())
            })?;
        } else {
            container.visit_payloads::<CvaError>(|object, payload| {
                let location = object.legacy_bytes();
                let end = u64::from_le_bytes(location[..8].try_into().unwrap())
                    + 8
                    + u64::from_le_bytes(location[8..].try_into().unwrap());
                freshness
                    .ingest_scanned(payload, end)
                    .map_err(CvaError::Freshness)
            })?;
        }
        freshness.finish_rebuild().map_err(CvaError::Freshness)?;
        let mut freshness_due = crate::FreshnessDueDispatcher::default();
        freshness_due
            .rebuild(
                freshness
                    .records()
                    .iter()
                    .map(|(id, record)| (*id, *record)),
                archive.rel_turn_count(),
                &freshness.policy(),
            )
            .map_err(|error| CvaError::Freshness(error.to_string()))?;
        let lexical_index = LexicalIndex::default();
        let packed_vectors = packed_state.finish()?;
        let compatibility_profiles = profile_state.finish()?;
        let memory_vectors =
            memory_vector_state.finish(&memories, &compatibility_profiles, &packed_vectors)?;
        let archive_vectors = archive_vector_state.finish(&archive, &packed_vectors)?;
        let vector_generations = generation_state.finish(
            &archive,
            &packed_vectors,
            &archive_vectors,
            &compatibility_profiles,
        )?;
        validate_semantic_global_versions(
            &archive,
            &memories,
            &entities,
            &relationships,
            &graph,
            &vector_generations,
        )?;
        let conversation_compactions = compaction_state.finish(&mut container)?;
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
            archive,
            memories,
            duplicate_index: DuplicateIndex::empty(),
            processing_epochs,
            dream_pairs,
            graph,
            communities,
            insomnia,
            lexical_index,
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
            freshness_due,
            freshness_notifications: std::collections::BTreeMap::new(),
        })
    }
}
