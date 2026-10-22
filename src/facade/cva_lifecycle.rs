use crate::archive_vector_store::ArchiveVectorStore;
use crate::community_store::CommunityStore;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::conversation_compaction_store::ConversationCompactionStore;
use crate::cva_global_validation::validate_semantic_global_versions;
use crate::dream_duplicate_index::DuplicateIndex;
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
use crate::lexical_index::LexicalIndex;
use crate::memory_store::MemoryStore;
use crate::memory_vector_store::MemoryVectorStore;
use crate::packed_vector_store::PackedVectorStore;
use crate::processing_epoch_store::ProcessingEpochStore;
use crate::project_file_binding_store::ProjectFileStore;
use crate::project_history_store::ProjectHistoryStore;
use crate::rel_metadata_store::RelMetadataStore;
use crate::relationship_store::RelationshipStore;
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

    fn initialize(mut container: Container) -> Result<Self, CvaError> {
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
        freshness
            .bind_history_source(&container, 0)
            .map_err(CvaError::Freshness)?;
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
            owner_maintenance,
            rewrite_lineage,
            external_index: crate::external_index::store::ExternalIndexStore::default(),
            container,
            archive,
            memories,
            duplicate_index: DuplicateIndex::empty(),
            processing_epochs,
            dream_pairs,
            graph,
            communities,
            insomnia,
            import_epochs,
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
        let container = Container::open(path)?;
        Self::open_from_container(container)
    }

    /// Replays a coherent read-only snapshot without acquiring writer authority.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, CvaError> {
        Self::open_from_container(Container::open_read_only(path)?)
    }

    pub(crate) fn open_from_container(container: Container) -> Result<Self, CvaError> {
        let crate::owner_lifecycle::ReliquaryReplay {
            mut container,
            owner_maintenance,
            rewrite_lineage,
            archive_state,
            memory_state,
            graph_state,
            community_state,
            insomnia_state,
            import_epochs,
            packed_state,
            memory_vector_state,
            archive_vector_state,
            profile_state,
            generation_state,
            interaction_streams,
            compaction_state,
            echo,
            ego,
            entity_state,
            entity_resolutions,
            relationship_state,
            project_history,
            project_files,
            rel_metadata,
            mut freshness,
            mut dream_freshness,
            defer_dream_replay,
            processing_epochs,
            dream_pairs,
            mut external_index,
        } = crate::owner_lifecycle::ReliquaryReplay::read(container)?;
        let archive = archive_state.finish()?;
        archive.validate_references(project_files.accepted_facts())?;
        crate::archive_prune_admission::validate(
            &archive,
            &crate::owner_lifecycle::HistorySource::new(&container).map_err(|error| error.0)?,
            &owner_maintenance,
        )?;
        let memories = memory_state.finish(&mut container)?;
        crate::memory_prune_admission::validate(&memories, &owner_maintenance)?;
        let entities = entity_state.finish(&container)?;
        let relationships = relationship_state.finish(&container)?;
        crate::owner_lifecycle::validate_current_relationship_references(
            container.owner_id().as_deref(),
            &memories,
            &entities,
            &relationships.current(),
        )?;

        ego.validate_memory_version(memories.memory_version())?;
        crate::memory_provenance_admission::validate(&memories, &archive)?;
        processing_epochs.validate(&memories)?;
        crate::owner_lifecycle::validate_dream_pairs(&dream_pairs, &memories)?;
        for link in archive.file_memory_link_records() {
            if !memories.contains_historical_memory(link.memory_id) {
                return Err(crate::ArchiveError::MissingFileMemoryTarget.into());
            }
        }
        let mut graph = graph_state.finish()?;
        crate::owner_lifecycle::validate_rebuilt_graph(&mut graph, &memories, &entities)?;
        crate::history::validate_source_history(
            &owner_maintenance,
            &crate::owner_lifecycle::HistorySource::new(&container)
                .map_err(|error| CvaError::Repack(error.to_string()))?,
            &graph,
        )
        .map_err(CvaError::Repack)?;
        graph
            .validate_rewrite_baseline(container.owner_uuid(), rewrite_lineage.rewrite_genesis())?;
        let communities = community_state.finish(&graph, container.owner_uuid())?;
        let mut insomnia = insomnia_state.finish()?;
        crate::insomnia_admission::validate_references(&insomnia, &archive, &memories)?;
        crate::insomnia_admission::rebuild_schedule(&mut insomnia, &archive)?;
        if defer_dream_replay {
            dream_freshness = DreamFreshnessJournal::new(container.owner_uuid());
            crate::cva_dream_journal_replay::replay_with_population(
                &mut container,
                &archive,
                &memories,
                &mut dream_freshness,
            )?;
        }
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
            let mut cursor = container.physical_cursor()?;
            while let Some(frame) = cursor.next_frame()? {
                let object = frame.object;
                let payload = frame.payload.as_slice();
                let location = object.legacy_bytes();
                let end = u64::from_le_bytes(location[..8].try_into().unwrap())
                    + 8
                    + u64::from_le_bytes(location[8..].try_into().unwrap());
                if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
                    .map_err(|message| CvaError::Freshness(message.into()))?
                    && let Some(cohort) =
                        Self::validate_insomnia_birth_cohort(&archive, &memories, &completion)?
                {
                    for member in cohort.members {
                        freshness
                            .derive_birth(owner, member.id, cohort.turn, false)
                            .map_err(CvaError::Freshness)?;
                        dream_freshness.derive_publication_birth(member.id);
                    }
                }
                if let Some(publication) = crate::memory_external_publication::decode(payload)? {
                    if let Some(cohort) =
                        Cva::validate_external_birth_cohort(&archive, &memories, &publication)?
                    {
                        for member in cohort.members {
                            freshness
                                .derive_birth(owner, member.id, cohort.turn, false)
                                .map_err(CvaError::Freshness)?;
                            dream_freshness.derive_publication_birth(member.id);
                        }
                    }
                }
                freshness
                    .ingest_scanned(payload, end)
                    .map_err(CvaError::Freshness)?;
            }
        } else {
            let mut cursor = container.physical_cursor()?;
            while let Some(frame) = cursor.next_frame()? {
                let object = frame.object;
                let payload = frame.payload.as_slice();
                let location = object.legacy_bytes();
                let end = u64::from_le_bytes(location[..8].try_into().unwrap())
                    + 8
                    + u64::from_le_bytes(location[8..].try_into().unwrap());
                freshness
                    .ingest_scanned(payload, end)
                    .map_err(CvaError::Freshness)?;
            }
        }
        freshness.finish_rebuild().map_err(CvaError::Freshness)?;
        if let Some(a) = freshness.semantic_activation() {
            let at = a
                .boundary
                .as_ref()
                .map_or(Some(0), |(c, n)| archive.activity_position_for_turn(c, n));
            if at != Some(a.accepted_turn) {
                return Err(CvaError::Freshness(
                    "semantic activation boundary is unavailable".into(),
                ));
            }
        }
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
        freshness_due
            .configure_initial_decay_carries(freshness.initial_decay_carries())
            .map_err(|error| CvaError::Freshness(error.to_string()))?;
        let lexical_index = LexicalIndex::default();
        let packed_vectors = packed_state.finish()?;
        let compatibility_profiles = profile_state.finish()?;
        crate::external_index_admission::validate_backing(
            &mut external_index,
            &packed_vectors,
            &compatibility_profiles,
        );
        let accepted_bodies = memory_vector_state
            .referenced_body_ids()
            .filter(|body| memories.contains_body(*body))
            .collect();
        let memory_vectors = memory_vector_state.finish(
            &accepted_bodies,
            &compatibility_profiles,
            &packed_vectors,
        )?;
        let fragment_versions = archive_vector_state
            .referenced_fragment_ids()
            .filter_map(|id| {
                archive
                    .fragment_archive_version(id)
                    .map(|version| (id, version))
            })
            .collect();
        let archive_vectors = archive_vector_state.finish(&packed_vectors, &fragment_versions)?;
        let vector_generations = generation_state.finish(
            archive.archive_version(),
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
        let conversation_compactions = compaction_state.finish_unrecovered()?;
        import_epochs.validate_owner(container.owner_uuid())?;
        crate::import_epoch_admission::validate_receipts(&import_epochs, &archive.import_receipts)?;
        let mut result = Self {
            owner_maintenance,
            rewrite_lineage,
            external_index,
            container,
            archive,
            memories,
            duplicate_index: DuplicateIndex::empty(),
            processing_epochs,
            dream_pairs,
            graph,
            communities,
            insomnia,
            import_epochs,
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
        };
        result.validate_pruned_packed_vectors()?;
        result.validate_pruned_descriptions()?;
        result.validate_historical_import_settlements()?;
        result.validate_import_completion_settlements()?;
        result.entity_resolutions.validate_local()?;
        crate::owner_lifecycle::validate_entity_resolution_references(
            &result.memories,
            &result.entities,
            &result.entity_resolutions.records(),
        )?;
        Ok(result)
    }
}
