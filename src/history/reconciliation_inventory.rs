//! Narrow immutable source reads used by divergent Import reconciliation.
//!
//! This view owns Container-dependent materialization. Import receives only
//! source facts and keeps the reconciliation policy and normalization rules.

use crate::owner_lifecycle::HistoryObject;
use crate::owner_lifecycle::{HistoryReadAccess, HistoryReadFailure, HistorySource, OwnerError};
use crate::{Archive, MemoryError, memory_store::MemoryStore};

pub(crate) struct ReconciliationRead<'a> {
    access: HistoryReadAccess<'a>,
    archive: &'a Archive,
    memories: &'a MemoryStore,
}

pub(crate) struct ReconciliationEpochNormalizer {
    dream: crate::freshness_dream_journal::DreamFreshnessJournal,
}

pub(crate) struct ReconciliationFrameEvidence {
    pub(crate) archive_records: Vec<Vec<u8>>,
    pub(crate) owner_evidence: Vec<Vec<u8>>,
    pub(crate) events: Vec<crate::freshness_storage::FreshnessReplayEntry>,
    pub(crate) dream: Option<Vec<u8>>,
    pub(crate) completion: Option<Vec<u8>>,
}

impl<'a> ReconciliationRead<'a> {
    pub(crate) fn new(
        access: HistoryReadAccess<'a>,
        archive: &'a Archive,
        memories: &'a MemoryStore,
    ) -> Self {
        Self {
            access,
            archive,
            memories,
        }
    }

    /// Preserve the existing first full-source physical scan checkpoint.
    pub(crate) fn materialized_resolution_changes(
        &self,
    ) -> Result<
        std::collections::BTreeMap<
            HistoryObject,
            crate::entity_resolution_codec::DecodedEntityResolution,
        >,
        MemoryError,
    > {
        self.access.materialized_resolution_changes()
    }

    pub(crate) fn snapshot(&self) -> Result<HistorySource<'a>, HistoryReadFailure> {
        self.access.snapshot()
    }

    pub(crate) fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.access.owner_uuid()
    }

    pub(crate) fn epoch_normalizer(&self) -> ReconciliationEpochNormalizer {
        ReconciliationEpochNormalizer {
            dream: crate::freshness_dream_journal::DreamFreshnessJournal::new(self.owner_uuid()),
        }
    }

    /// Read and normalize one physical record's accepted owner evidence. All
    /// codec and cross-owner materialization stays behind this History view;
    /// Import retains epoch policy and aggregation only.
    pub(crate) fn frame_evidence(
        &self,
        source: &HistorySource<'_>,
        object: HistoryObject,
        payload: &[u8],
        resolution: Option<&crate::entity_resolution_codec::DecodedEntityResolution>,
        entities: &crate::entity_store::EntityStore,
        relationships: &crate::relationship_store::RelationshipStore,
        graph: &crate::graph_store::GraphStore,
        normalizer: &mut ReconciliationEpochNormalizer,
    ) -> Result<ReconciliationFrameEvidence, crate::CvaReconcileError> {
        let mut result = ReconciliationFrameEvidence {
            archive_records: Vec::new(),
            owner_evidence: Vec::new(),
            events: Vec::new(),
            dream: None,
            completion: None,
        };
        if let Some(version) = crate::archive_codec::publication::version_at(object, payload)? {
            let record = self.archive_publication(source, version)?;
            result
                .archive_records
                .push(crate::archive_codec::publication::import_evidence(record)?);
        }
        if let Some((global, local)) =
            crate::memory_compact_codec::publication::coordinates(payload)?
        {
            result.owner_evidence.push(
                self.memories
                    .reconciliation_publication_evidence(global, local, payload)?
                    .ok_or(crate::CvaReconcileError::InvalidMemoryVersionRecord)?,
            );
        }
        if let Some((global, local)) =
            crate::entity_compact_codec::publication::coordinates(payload)?
        {
            let publication = entities
                .retained_publication_at(global, local)?
                .ok_or(crate::EntityError::InvalidVersion)?;
            result
                .owner_evidence
                .push(publication.normalized_import_evidence()?);
        }
        if let Some((global, local)) =
            crate::relationship_compact_codec::publication::coordinates(payload)?
        {
            let publication = relationships
                .retained_publication_at(global, local)?
                .ok_or(crate::RelationshipError::InvalidVersion)?;
            result
                .owner_evidence
                .push(publication.normalized_import_evidence()?);
        }
        if let Some(version) = crate::graph_compact_codec::publication::version_at(object, payload)?
        {
            let bytes = source
                .read_object(version.mutation)
                .map_err(|error| crate::CvaReconcileError::Container(error.0))?;
            result
                .owner_evidence
                .push(graph.normalized_import_evidence(&bytes)?);
        }
        if let Some(mut snapshot) =
            crate::community_codec::decode_snapshot(payload).map_err(crate::CvaError::from)?
        {
            snapshot.generation = 0;
            snapshot.derived_graph_version = 0;
            result.owner_evidence.push(
                crate::community_codec::encode_snapshot(&snapshot)
                    .map_err(crate::CvaError::from)?,
            );
        }
        if let Some(change) = resolution {
            result
                .owner_evidence
                .push(crate::entity_resolution_codec::comparison_carrier(change)?);
        }
        if crate::memory_routing_codec::decode(payload)?.is_some()
            || payload.starts_with(b"CVAPEP01")
        {
            result.owner_evidence.push(payload.to_vec());
        }
        result.events = crate::freshness_storage::FreshnessReplayEntry::from_payload(payload)
            .map_err(crate::CvaError::Freshness)?;

        if payload.starts_with(b"CVDREAM1") || payload.starts_with(b"CVDREAM2") {
            let normalized =
                if let Some(mut pass) = self.materialized_dream_begin(source, object, payload)? {
                    pass.turn = 0;
                    pass.graph_version_before = 0;
                    let owner = self
                        .access
                        .required_owner_uuid()
                        .map_err(|error| crate::CvaReconcileError::Container(error.0))?;
                    normalizer
                        .dream
                        .ingest_materialized_begin(owner, pass.clone())
                        .map_err(crate::CvaError::Freshness)?;
                    crate::freshness_dream_journal::begin_comparison_carrier(owner, &pass)
                } else {
                    let bytes = normalizer
                        .dream
                        .rebase_import_payload(payload)
                        .map_err(crate::CvaError::Freshness)?;
                    normalizer
                        .dream
                        .ingest(&bytes)
                        .map_err(crate::CvaError::Freshness)?;
                    bytes
                };
            if !normalized.is_empty() {
                result.dream = Some(normalized);
            }
        }

        if let Some(mut completion) = crate::insomnia::completion::decode_completion(payload)
            .map_err(crate::CvaReconcileError::InvalidInsomniaCompletion)?
        {
            self.hydrate_grouped_completion(source, &mut completion)?;
            completion.bodies.sort_by_key(|body| body.id.0);
            for (index, record) in completion.records.iter_mut().enumerate() {
                let coordinate = u64::try_from(index)
                    .ok()
                    .and_then(|offset| offset.checked_add(1))
                    .ok_or(crate::CvaReconcileError::UnsupportedSemanticOwner(
                        "too many grouped completion records",
                    ))?;
                record.global_version = coordinate;
                record.memory_version = coordinate;
            }
            completion.global_version_start = 1;
            completion.transaction_time_ns = 1;
            completion.freshness_birth_turn = completion.freshness_birth_turn.map(|_| 0);
            result.completion = Some(
                crate::insomnia::completion::encode_completion(&completion)
                    .map_err(crate::CvaReconcileError::InvalidInsomniaCompletion)?,
            );
        }
        Ok(result)
    }

    pub(crate) fn physical_objects(
        &self,
        source: &HistorySource<'_>,
    ) -> Result<Vec<HistoryObject>, crate::CvaReconcileError> {
        let mut cursor = source
            .cursor()
            .map_err(|error| crate::CvaReconcileError::Container(error.0))?;
        let mut objects = Vec::new();
        while let Some(frame) = cursor
            .next_frame()
            .map_err(|error| crate::CvaReconcileError::Container(error.0))?
        {
            objects.push(frame.object);
        }
        Ok(objects)
    }

    pub(crate) fn read_object(
        &self,
        source: &HistorySource<'_>,
        object: HistoryObject,
    ) -> Result<Vec<u8>, crate::CvaReconcileError> {
        source
            .read_object(object)
            .map_err(|error| crate::CvaReconcileError::Container(error.0))
    }

    pub(crate) fn archive_publication(
        &self,
        source: &HistorySource<'_>,
        version: crate::ArchiveRecordVersion,
    ) -> Result<crate::archive_codec::ArchiveRecord, crate::ArchiveError> {
        source.archive_publication(self.archive, version)
    }

    pub(crate) fn materialized_dream_begin(
        &self,
        source: &HistorySource<'_>,
        object: HistoryObject,
        payload: &[u8],
    ) -> Result<Option<crate::InitialDreamPass>, OwnerError> {
        source.materialized_dream_begin(self.archive, self.memories, object, payload)
    }

    pub(crate) fn hydrate_grouped_completion(
        &self,
        source: &HistorySource<'_>,
        completion: &mut crate::insomnia::completion::InsomniaCompletion,
    ) -> Result<(), OwnerError> {
        source
            .hydrate_grouped_completion(self.memories, completion)
            .map_err(OwnerError::from)
    }
}

/// Validate a shared graph prefix before divergent replay. The Graph owner
/// computes semantic skips and translates conflicts to the public reconciliation
/// error type.
pub(crate) fn validate_graph_overlap(
    left: &crate::graph_ordered_replay::GraphTail,
    right: &crate::graph_ordered_replay::GraphTail,
) -> Result<(), crate::CvaReconcileError> {
    crate::graph_ordered_replay::replay_skips(left, right)
        .map(|_| ())
        .map_err(crate::owner_lifecycle::graph_ordered_replay::conflict_error)
}
