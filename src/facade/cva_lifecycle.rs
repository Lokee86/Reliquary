use crate::archive_vector_rebuild::ArchiveVectorOpenState;
use crate::archive_vector_store::ArchiveVectorStore;
use crate::community_owner::{CommunityOwner, CommunityReplay};
use crate::compatibility_profile_rebuild::CompatibilityProfileOpenState;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::cva_global_validation::validate_semantic_global_versions;
use crate::cva_open_route::{OpenRecordRoute, classify_open_record};
use crate::dream_duplicate_index::DuplicateIndex;
use crate::dream_pair_history::DreamPairStore;
use crate::ego_store::EgoStore;
use crate::entity_rebuild::EntityOpenState;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::freshness_storage::FreshnessStore;
use crate::graph_owner::GraphOwner;
use crate::graph_replay::GraphReplay;
use crate::insomnia::rebuild::InsomniaOpenState;
use crate::insomnia::store::InsomniaStore;
use crate::lexical_index::LexicalIndex;
use crate::memory_rebuild::MemoryOpenState;
use crate::memory_store::MemoryStore;
use crate::memory_vector_rebuild::MemoryVectorOpenState;
use crate::memory_vector_store::MemoryVectorStore;
use crate::packed_vector_rebuild::PackedVectorOpenState;
use crate::packed_vector_store::PackedVectorStore;
use crate::processing_epoch_store::ProcessingEpochStore;
use crate::rel_metadata_store::RelMetadataStore;
use crate::relationship_rebuild::RelationshipOpenState;
use crate::relationship_store::RelationshipStore;
use crate::vector_generation_rebuild::VectorGenerationOpenState;
use crate::vector_generation_store::VectorGenerationStore;
use crate::{Archive, Cva, CvaError};
use reliquary_archive::archive_rebuild::ArchiveOpenState;
use reliquary_archive::conversation_compaction_store::{
    ConversationCompactionOpenState, ConversationCompactionStore,
};
use reliquary_archive::interaction_stream_store::InteractionStreamStore;
use reliquary_archive::project_file_binding_store::ProjectFileStore;
use reliquary_archive::project_history_store::ProjectHistoryStore;
use reliquary_container::Container;
use reliquary_echo::EchoStore;
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
            reliquary_container::ContainerIdentity {
                file_kind: reliquary_container::FileKind::Reliquary,
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
            reliquary_container::ContainerIdentity {
                file_kind: reliquary_container::FileKind::Reliquary,
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
        scope: reliquary_container::ReliquaryScopeKind,
        owner_uuid: [u8; 16],
    ) -> Result<Self, CvaError> {
        let container = Container::create_with_identity_and_uuid(
            path,
            reliquary_container::ContainerIdentity {
                file_kind: reliquary_container::FileKind::Reliquary,
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
        let mut graph = GraphOwner::empty();
        let communities = CommunityOwner::default();
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
        freshness.bind_history_source(&container, 0)?;
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
            external_index: crate::external_source::store::ExternalIndexStore::default(),
            external_publications:
                crate::external_source::publication::ExternalPublicationState::default(),
            container,
            import_write_admission: None,
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
        let header = Container::open_read_only_for_scan(path.as_ref())?;
        if header.owner_uuid().is_none() {
            return Err(reliquary_container::ContainerError::InvalidIdentity.into());
        }
        if header.identity().map(|identity| identity.file_kind)
            != Some(reliquary_container::FileKind::Reliquary)
        {
            return Err(CvaError::InvalidContainerIdentity(
                "file is not a Reliquary",
            ));
        }
        let lineage_owner = header.owner_uuid();
        drop(header);
        let mut owner_maintenance = crate::owner_maintenance::OwnerMaintenanceStore::default();
        let mut rewrite_lineage = crate::rewrite_lineage::RewriteLineageStore::default();
        let mut archive_state = ArchiveOpenState::new();
        let mut memory_state = MemoryOpenState::new();
        let mut graph_state = GraphReplay::new();
        let mut community_state = CommunityReplay::new();
        let mut insomnia_state = InsomniaOpenState::new();
        let mut import_epochs = crate::imports::ImportEpochStore::default();
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
        let mut dream_freshness = DreamFreshnessJournal::default();
        let mut defer_dream_replay = false;
        let mut processing_epochs = ProcessingEpochStore::default();
        let mut dream_pairs = DreamPairStore::default();
        let mut external_index = crate::external_source::store::ExternalIndexStore::default();
        let mut external_publications =
            crate::external_source::publication::ExternalPublicationState::default();
        let mut container = Container::open_for_replay(path.as_ref())?;
        freshness.bind_history_source(&container, 0)?;
        {
            let mut replay = |chunk, payload: &[u8], latest_global| {
                if let Some(owner_uuid) = lineage_owner {
                    rewrite_lineage
                        .ingest(payload, owner_uuid)
                        .map_err(CvaError::Repack)?;
                } else if payload.starts_with(crate::rewrite_lineage::REWRITE_GENESIS_MAGIC) {
                    return Err(CvaError::Repack(
                        "RewriteGenesis requires a versioned owner identity".into(),
                    ));
                }
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
                    OpenRecordRoute::Publication => {
                        let envelope = reliquary_container::publication_envelope::decode(payload)?
                            .ok_or(crate::MemoryError::CorruptRecord(
                                "missing publication envelope",
                            ))?;
                        if !memory_state.ingest_publication_envelope(envelope, latest_global)?
                            && !entity_state.ingest_publication_envelope(envelope, latest_global)?
                            && !relationship_state
                                .ingest_publication_envelope(envelope, latest_global)?
                            && !archive_state.ingest_publication_envelope(
                                chunk,
                                envelope,
                                latest_global,
                            )?
                            && !graph_state.ingest_publication_envelope(envelope, latest_global)?
                        {
                            return Err(crate::MemoryError::CorruptRecord(
                                "unsupported publication envelope body",
                            )
                            .into());
                        }
                    }
                    OpenRecordRoute::Fallback => {
                        owner_maintenance
                            .ingest(payload)
                            .map_err(CvaError::Repack)?;
                        import_epochs.ingest(payload)?;

                        external_index.ingest(payload);
                        archive_state.ingest(chunk, payload, latest_global)?;
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
                        if let Some(publication) =
                            crate::external_source::publication::decode(payload)?
                            && publication.birth_turn.is_some()
                            && !publication.records.is_empty()
                        {
                            freshness.freeze_derived_birth_policy();
                        }
                        if payload.starts_with(b"CVAFRP01") {
                            freshness.ingest(payload).map_err(CvaError::Freshness)?;
                        }
                        if payload.starts_with(b"CVDREAM") && !payload.starts_with(b"CVDREAM1") {
                            if !payload.starts_with(b"CVDREAM2") {
                                return Err(CvaError::Freshness(
                                    "unsupported Dream journal version".into(),
                                ));
                            }
                            if lineage_owner.is_none() {
                                return Err(CvaError::Freshness(
                                    "CVDREAM2 Begin requires a versioned V2 owner".into(),
                                ));
                            }
                            let record = crate::dream_begin_codec::decode(payload)
                                .map_err(CvaError::Freshness)?;
                            if lineage_owner != Some(record.owner_uuid) {
                                return Err(CvaError::Freshness(
                                    "Dream Freshness pass owner mismatch".into(),
                                ));
                            }
                            dream_freshness
                                .bind_owner(Some(record.owner_uuid))
                                .map_err(CvaError::Freshness)?;
                            defer_dream_replay = true;
                        }
                        if !defer_dream_replay {
                            dream_freshness
                                .ingest(payload)
                                .map_err(CvaError::Freshness)?;
                        }
                        processing_epochs.ingest(payload)?;

                        dream_pairs.ingest(payload)?;
                    }
                }
                Ok::<(), CvaError>(())
            };
            let mut cursor = container.physical_cursor()?;
            while let Some(frame) = cursor.next_frame()? {
                replay(frame.object, &frame.payload, frame.latest_global)?;
            }
            let state = cursor.finish_open_scan()?;
            container.accept_replay_scan(state)?;
        }
        if let Some(identity) = container.identity()
            && identity.file_kind != reliquary_container::FileKind::Reliquary
        {
            return Err(CvaError::InvalidContainerIdentity(
                "file is not a Reliquary",
            ));
        }
        let import_write_admission = if import_epochs.open().is_some() {
            Some(container.close_write_admission()?)
        } else {
            None
        };
        freshness
            .bind_owner(container.owner_uuid())
            .map_err(CvaError::Freshness)?;
        if !freshness.rewrite_baselines.is_empty() {
            let genesis = rewrite_lineage.rewrite_genesis().ok_or_else(|| {
                CvaError::Freshness(
                    "Freshness rewrite baselines require a RewriteGenesis record".into(),
                )
            })?;
            for baseline in freshness.rewrite_baselines.values() {
                if baseline.owner_uuid != genesis.destination_owner
                    || baseline.history_generation != genesis.generation
                    || baseline.policy_digest != genesis.policy_digest
                    || Some(baseline.owner_uuid) != container.owner_uuid()
                {
                    return Err(CvaError::Freshness(
                        "Freshness rewrite baseline does not match RewriteGenesis".into(),
                    ));
                }
            }
        }
        dream_freshness
            .bind_owner(container.owner_uuid())
            .map_err(CvaError::Freshness)?;
        let archive = archive_state.finish()?;
        archive.validate_references(&project_files)?;
        crate::cva_archive_history_prune::validate_archive_markers(&archive, &owner_maintenance)?;
        crate::cva_archive_history_prune::validate_archive_prune_history(
            &archive,
            &mut container,
            &owner_maintenance,
        )?;
        let memories = memory_state.finish(&mut container)?;
        crate::memory_history_prune::validate_prune_history(
            &memories,
            &mut container,
            owner_maintenance.manifests(),
        )?;
        let entities = entity_state.finish(&container)?;
        let relationships = relationship_state.finish(&container)?;
        relationships.validate_local_references(
            container.owner_id().as_deref(),
            &memories,
            &entities,
        )?;

        ego.validate_memory_version(memories.memory_version())?;
        crate::cva_memory_publish::validate_store_provenance(&memories, &archive)?;
        processing_epochs.validate(&memories)?;
        dream_pairs.validate(&memories)?;
        for link in archive.all_file_memory_links() {
            if !memories.contains_historical_memory(crate::MemoryId(link.memory_id.0)) {
                return Err(crate::ArchiveError::MissingFileMemoryTarget.into());
            }
        }
        let graph = graph_state.finish(&memories, &entities)?;
        owner_maintenance
            .validate_source_history(&mut container, &graph)
            .map_err(CvaError::Repack)?;
        graph
            .validate_rewrite_baseline(container.owner_uuid(), rewrite_lineage.rewrite_genesis())?;
        let communities = community_state.finish(&graph, container.owner_uuid())?;
        let mut insomnia = insomnia_state.finish()?;
        insomnia.validate(&archive, &memories)?;
        insomnia.rebuild_schedule(&archive)?;
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
                let payload = frame.payload;
                let end = frame.end;
                if let Some(completion) = crate::insomnia::completion::decode_completion(&payload)
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
                if let Some(publication) = crate::external_source::publication::decode(&payload)? {
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
                    .ingest_scanned(&payload, end)
                    .map_err(CvaError::Freshness)?;
            }
        } else {
            let mut cursor = container.physical_cursor()?;
            while let Some(frame) = cursor.next_frame()? {
                let payload = frame.payload;
                let end = frame.end;
                freshness
                    .ingest_scanned(&payload, end)
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
        external_index.validate_backing(&packed_vectors, &compatibility_profiles);
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
        // Reconstruct the live projection without reclaiming physical chunks on open.
        let conversation_compactions = compaction_state.finish_without_reclaim()?;
        import_epochs.validate_owner(container.owner_uuid())?;
        import_epochs.validate_receipts(archive.import_receipts())?;
        let mut result = Self {
            owner_maintenance,
            rewrite_lineage,
            external_index,
            external_publications,
            container,
            import_write_admission,
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
        result
            .entity_resolutions
            .validate(&result.memories, &result.entities)?;
        Ok(result)
    }

    /// Explicitly canonicalize Entity-resolution references after entity retirement.
    /// Opening a REL/PHY never performs this durable maintenance automatically.
    pub fn repair_retired_entity_references(&mut self) -> Result<usize, CvaError> {
        let repaired = self.entity_resolutions.repair_retired_entity_references(
            &mut self.container,
            &self.memories,
            &self.entities,
        )?;
        self.entity_resolutions
            .validate(&self.memories, &self.entities)?;
        if repaired > 0 {
            self.container.sync()?;
        }
        Ok(repaired)
    }
}
