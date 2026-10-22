//! One physical REL replay into concrete pending owner states.
//! Cross-owner historical checks remain at their original caller checkpoints;
//! no History, Import or Maintenance service is invoked during this scan.
use super::{
    OwnerError,
    open_route::{OpenRecordRoute, classify_open_record},
};
use crate::Container;
use crate::archive_rebuild::ArchiveOpenState;
use crate::archive_vector_rebuild::ArchiveVectorOpenState;
use crate::community_store::CommunityOpenState;
use crate::compatibility_profile_rebuild::CompatibilityProfileOpenState;
use crate::conversation_compaction_store::ConversationCompactionOpenState;
use crate::dream_pair_history::DreamPairStore;
use crate::echo_store::EchoStore;
use crate::ego_store::EgoStore;
use crate::entity_rebuild::EntityOpenState;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::freshness_storage::FreshnessStore;
use crate::graph_rebuild::GraphOpenState;
use crate::insomnia::rebuild::InsomniaOpenState;
use crate::interaction_stream_store::InteractionStreamStore;
use crate::memory_rebuild::MemoryOpenState;
use crate::memory_vector_rebuild::MemoryVectorOpenState;
use crate::packed_vector_rebuild::PackedVectorOpenState;
use crate::processing_epoch_store::ProcessingEpochStore;
use crate::project_file_binding_store::ProjectFileStore;
use crate::project_history_store::ProjectHistoryStore;
use crate::rel_metadata_store::RelMetadataStore;
use crate::relationship_rebuild::RelationshipOpenState;
use crate::vector_generation_rebuild::VectorGenerationOpenState;

pub(crate) struct ReliquaryReplay {
    pub(crate) container: Container,
    pub(crate) owner_maintenance: crate::owner_maintenance::OwnerMaintenanceStore,
    pub(crate) rewrite_lineage: crate::rewrite_lineage::RewriteLineageStore,
    pub(crate) archive_state: crate::archive_rebuild::ArchiveOpenState,
    pub(crate) memory_state: crate::memory_rebuild::MemoryOpenState,
    pub(crate) graph_state: crate::graph_rebuild::GraphOpenState,
    pub(crate) community_state: crate::community_store::CommunityOpenState,
    pub(crate) insomnia_state: crate::insomnia::rebuild::InsomniaOpenState,
    pub(crate) import_epochs: crate::imports::ImportEpochStore,
    pub(crate) packed_state: crate::packed_vector_rebuild::PackedVectorOpenState,
    pub(crate) memory_vector_state: crate::memory_vector_rebuild::MemoryVectorOpenState,
    pub(crate) archive_vector_state: crate::archive_vector_rebuild::ArchiveVectorOpenState,
    pub(crate) profile_state: crate::compatibility_profile_rebuild::CompatibilityProfileOpenState,
    pub(crate) generation_state: crate::vector_generation_rebuild::VectorGenerationOpenState,
    pub(crate) interaction_streams: crate::interaction_stream_store::InteractionStreamStore,
    pub(crate) compaction_state:
        crate::conversation_compaction_store::ConversationCompactionOpenState,
    pub(crate) echo: crate::echo_store::EchoStore,
    pub(crate) ego: crate::ego_store::EgoStore,
    pub(crate) entity_state: crate::entity_rebuild::EntityOpenState,
    pub(crate) entity_resolutions: crate::entity_resolution_store::EntityResolutionStore,
    pub(crate) relationship_state: crate::relationship_rebuild::RelationshipOpenState,
    pub(crate) project_history: crate::project_history_store::ProjectHistoryStore,
    pub(crate) project_files: crate::project_file_binding_store::ProjectFileStore,
    pub(crate) rel_metadata: crate::rel_metadata_store::RelMetadataStore,
    pub(crate) freshness: crate::freshness_storage::FreshnessStore,
    pub(crate) dream_freshness: crate::freshness_dream_journal::DreamFreshnessJournal,
    pub(crate) defer_dream_replay: bool,
    pub(crate) processing_epochs: crate::processing_epoch_store::ProcessingEpochStore,
    pub(crate) dream_pairs: crate::dream_pair_history::DreamPairStore,
    pub(crate) external_index: crate::external_index::store::ExternalIndexStore,
}

impl ReliquaryReplay {
    pub(crate) fn read(mut container: Container) -> Result<Self, OwnerError> {
        if container.owner_uuid().is_none() {
            return Err(crate::ContainerError::InvalidIdentity.into());
        }
        if container.identity().map(|identity| identity.file_kind)
            != Some(crate::FileKind::Reliquary)
        {
            return Err(OwnerError::InvalidContainerIdentity(
                "file is not a Reliquary",
            ));
        }
        let lineage_owner = container.owner_uuid();
        let mut owner_maintenance = crate::owner_maintenance::OwnerMaintenanceStore::default();
        let mut rewrite_lineage = crate::rewrite_lineage::RewriteLineageStore::default();
        let mut archive_state = ArchiveOpenState::new();
        let mut memory_state = MemoryOpenState::new();
        let mut graph_state = GraphOpenState::new();
        let mut community_state = CommunityOpenState::new();
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
        freshness
            .bind_history_source(&container, 0)
            .map_err(OwnerError::Freshness)?;
        let mut dream_freshness = DreamFreshnessJournal::default();
        let mut defer_dream_replay = false;
        let mut processing_epochs = ProcessingEpochStore::default();
        let mut dream_pairs = DreamPairStore::default();
        let mut external_index = crate::external_index::store::ExternalIndexStore::default();
        {
            let mut cursor = container.physical_cursor()?;
            while let Some(frame) = cursor.next_frame()? {
                let chunk = frame.object;
                let payload = frame.payload.as_slice();
                let latest_global = frame.latest_global;
                if let Some(owner_uuid) = lineage_owner {
                    rewrite_lineage
                        .ingest(payload, owner_uuid)
                        .map_err(OwnerError::Repack)?;
                } else if payload.starts_with(crate::rewrite_lineage::REWRITE_GENESIS_MAGIC) {
                    return Err(OwnerError::Repack(
                        "RewriteGenesis requires a versioned owner identity".into(),
                    ));
                }
                match classify_open_record(payload) {
                    OpenRecordRoute::Archive => {
                        archive_state.ingest(chunk, payload, latest_global)?;
                    }
                    OpenRecordRoute::Memory => {
                        super::memory_frames::ingest(
                            &mut memory_state,
                            chunk,
                            payload,
                            latest_global,
                        )?;
                    }
                    OpenRecordRoute::Graph => {
                        graph_state.ingest(chunk, payload, latest_global)?;
                    }
                    OpenRecordRoute::ContainerVersion => {}
                    OpenRecordRoute::Publication => {
                        let envelope = crate::container::publication_envelope::decode(payload)?
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
                            .map_err(OwnerError::Repack)?;
                        import_epochs.ingest(payload)?;

                        external_index.ingest(payload);
                        archive_state.ingest(chunk, payload, latest_global)?;
                        super::memory_frames::ingest(
                            &mut memory_state,
                            chunk,
                            payload,
                            latest_global,
                        )?;
                        graph_state.ingest(chunk, payload, latest_global)?;
                        entity_state.ingest(chunk, payload, latest_global)?;
                        relationship_state.ingest(chunk, payload, latest_global)?;
                        community_state.ingest(payload)?;
                        super::insomnia_replay::ingest_frame(&mut insomnia_state, chunk, payload)?;
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
                            .map_err(OwnerError::RelMetadata)?;
                        if let Some(completion) =
                            crate::insomnia::completion::decode_completion(payload)
                                .map_err(|message| OwnerError::Freshness(message.into()))?
                            && completion.freshness_birth_turn.is_some()
                            && !completion.freshness_birth_memory_ids.is_empty()
                        {
                            freshness.freeze_derived_birth_policy();
                        }
                        if let Some(publication) =
                            crate::memory_external_publication::decode(payload)?
                            && publication.birth_turn.is_some()
                            && !publication.records.is_empty()
                        {
                            freshness.freeze_derived_birth_policy();
                        }
                        if payload.starts_with(b"CVAFRP01") {
                            freshness.ingest(payload).map_err(OwnerError::Freshness)?;
                        }
                        if payload.starts_with(b"CVDREAM") && !payload.starts_with(b"CVDREAM1") {
                            if !payload.starts_with(b"CVDREAM2") {
                                return Err(OwnerError::Freshness(
                                    "unsupported Dream journal version".into(),
                                ));
                            }
                            if lineage_owner.is_none() {
                                return Err(OwnerError::Freshness(
                                    "CVDREAM2 Begin requires a versioned V2 owner".into(),
                                ));
                            }
                            let record = crate::dream_begin_codec::decode(payload)
                                .map_err(OwnerError::Freshness)?;
                            if lineage_owner != Some(record.owner_uuid) {
                                return Err(OwnerError::Freshness(
                                    "Dream Freshness pass owner mismatch".into(),
                                ));
                            }
                            dream_freshness
                                .bind_owner(Some(record.owner_uuid))
                                .map_err(OwnerError::Freshness)?;
                            defer_dream_replay = true;
                        }
                        if !defer_dream_replay {
                            dream_freshness
                                .ingest(payload)
                                .map_err(OwnerError::Freshness)?;
                        }
                        processing_epochs.ingest(payload)?;

                        dream_pairs.ingest(payload)?;
                    }
                }
            }
            let clock_state = cursor.finish_open_scan()?;
            container.apply_scan_clock(clock_state);
        }
        let import_open = import_epochs.open().is_some();
        container.set_write_admission_closed(import_open);
        freshness
            .bind_owner(container.owner_uuid())
            .map_err(OwnerError::Freshness)?;
        if !freshness.rewrite_baselines.is_empty() {
            let genesis = rewrite_lineage.rewrite_genesis().ok_or_else(|| {
                OwnerError::Freshness(
                    "Freshness rewrite baselines require a RewriteGenesis record".into(),
                )
            })?;
            for baseline in freshness.rewrite_baselines.values() {
                if baseline.owner_uuid != genesis.destination_owner
                    || baseline.history_generation != genesis.generation
                    || baseline.policy_digest != genesis.policy_digest
                    || Some(baseline.owner_uuid) != container.owner_uuid()
                {
                    return Err(OwnerError::Freshness(
                        "Freshness rewrite baseline does not match RewriteGenesis".into(),
                    ));
                }
            }
        }
        dream_freshness
            .bind_owner(container.owner_uuid())
            .map_err(OwnerError::Freshness)?;
        Ok(Self {
            container,
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
            freshness,
            dream_freshness,
            defer_dream_replay,
            processing_epochs,
            dream_pairs,
            external_index,
        })
    }
}
