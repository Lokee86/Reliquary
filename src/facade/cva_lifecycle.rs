use crate::cva_global_validation::validate_semantic_global_versions;
use crate::dream_duplicate_index::DuplicateIndex;
use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::lexical_index::LexicalIndex;
use crate::{Container, Cva, CvaError};
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

    fn initialize(container: Container) -> Result<Self, CvaError> {
        Ok(Self::from_owner(
            crate::owner_lifecycle::initialize_reliquary(container)?,
        ))
    }

    fn from_owner(owner: crate::owner_lifecycle::ReliquaryOwner) -> Self {
        let crate::owner_lifecycle::ReliquaryOwner {
            owner_maintenance,
            rewrite_lineage,
            external_index,
            container,
            archive,
            memories,
            graph,
            communities,
            processing_epochs,
            dream_pairs,
            insomnia,
            import_epochs,
            packed_vectors,
            memory_vectors,
            archive_vectors,
            compatibility_profiles,
            vector_generations,
            conversation_compactions,
            interaction_streams,
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
        } = owner;
        Self {
            owner_maintenance,
            rewrite_lineage,
            external_index,
            container,
            archive,
            memories,
            graph,
            communities,
            processing_epochs,
            dream_pairs,
            insomnia,
            import_epochs,
            packed_vectors,
            memory_vectors,
            archive_vectors,
            compatibility_profiles,
            vector_generations,
            conversation_compactions,
            interaction_streams,
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
            duplicate_index: DuplicateIndex::empty(),
            lexical_index: LexicalIndex::default(),
            freshness_due: crate::FreshnessDueDispatcher::default(),
            freshness_notifications: std::collections::BTreeMap::new(),
        }
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
        let archive =
            crate::owner_lifecycle::finish_reliquary_archive(archive_state, &project_files)?;
        crate::archive_prune_admission::validate(
            &archive,
            &crate::owner_lifecycle::HistorySource::new(&container).map_err(|error| error.0)?,
            &owner_maintenance,
        )?;
        let memories = memory_state.finish(&mut container)?;
        crate::memory_prune_admission::validate(&memories, &owner_maintenance)?;
        let crate::owner_lifecycle::ReliquaryRelations {
            entities,
            relationships,
        } = crate::owner_lifecycle::finish_reliquary_relations(
            &container,
            entity_state,
            relationship_state,
            &memories,
            &ego,
            &archive,
            &processing_epochs,
            &dream_pairs,
        )?;
        let graph =
            crate::owner_lifecycle::finish_reliquary_graph(graph_state, &memories, &entities)?;
        crate::history::validate_source_history(
            &owner_maintenance,
            &crate::owner_lifecycle::HistorySource::new(&container)
                .map_err(|error| CvaError::Repack(error.to_string()))?,
            &graph,
        )
        .map_err(CvaError::Repack)?;
        graph
            .validate_rewrite_baseline(container.owner_uuid(), rewrite_lineage.rewrite_genesis())?;
        let crate::owner_lifecycle::ReliquaryAcceptedReplay {
            communities,
            insomnia,
        } = crate::owner_lifecycle::finish_reliquary_accepted_replay(
            &container,
            community_state,
            &graph,
            insomnia_state,
            &archive,
            &memories,
            defer_dream_replay,
            &mut dream_freshness,
            &mut freshness,
        )?;
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
        let crate::owner_lifecycle::ReliquaryVectors {
            packed: packed_vectors,
            memory: memory_vectors,
            archive: archive_vectors,
            profiles: compatibility_profiles,
            generations: vector_generations,
        } = crate::owner_lifecycle::finish_reliquary_vectors(
            packed_state,
            profile_state,
            memory_vector_state,
            archive_vector_state,
            generation_state,
            &mut external_index,
            &memories,
            &archive,
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
        crate::owner_lifecycle::validate_import_receipts(&import_epochs, &archive.import_receipts)?;
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
