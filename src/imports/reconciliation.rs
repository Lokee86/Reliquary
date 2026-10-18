//! Ordered reconciliation of completed native import epochs through their owners.
use super::{ImportComplete, ImportEpochStatus, ImportStart};
use crate::cva_reconcile::{CvaComparison, CvaReconcileResult};
use crate::freshness_storage::HistoricalImportFreshnessSettlement;
use crate::{Cva, CvaReconcileError};
use std::{collections::BTreeMap, path::Path};

pub(super) struct CompletedEpoch {
    pub start: ImportStart,
    pub complete: ImportComplete,
    pub settlement: HistoricalImportFreshnessSettlement,
    pub first: usize,
    pub end: usize,
    completions: Vec<Vec<u8>>,
    archive_records: Vec<Vec<u8>>,
    events: Vec<crate::freshness_storage::FreshnessReplayEntry>,
    dream: Vec<Vec<u8>>,
    owner_evidence: Vec<Vec<u8>>,
}

pub(crate) fn reconcile_import_epochs(
    left_path: &Path,
    right_path: &Path,
    output_path: &Path,
    comparison: CvaComparison,
) -> Result<Option<CvaReconcileResult>, CvaReconcileError> {
    let mut left = Cva::open(left_path)?;
    let mut right = Cva::open(right_path)?;
    if left.import_epoch_statuses().is_empty() && right.import_epoch_statuses().is_empty() {
        return Ok(None);
    }
    if left.open_import_epoch().is_some() || right.open_import_epoch().is_some() {
        return Err(unsupported("open import epoch in divergent reconciliation"));
    }
    // A conflict in source revision authority is independent of replay support.
    // Detect it before copying a destination or comparing coordinate-bound data.
    for receipt in right.native_import_receipts() {
        left.archive.validate_import_revisions(&receipt)?;
    }
    let left_epochs = inventory(&mut left)?;
    let right_epochs = inventory(&mut right)?;
    for (id, epoch) in &right_epochs {
        if let Some(known) = left_epochs.get(id) {
            if !same_authority(known, epoch)
                || left.archive.import_receipts.get(&known.start.import_key)
                    != right.archive.import_receipts.get(&epoch.start.import_key)
            {
                return Err(CvaReconcileError::Conflict(
                    crate::CvaReconcileConflict::ImportEpoch {
                        import_id: id.clone(),
                    },
                ));
            }
        }
    }
    let boundary = physical_boundary(&mut right, comparison.common_chunk_count)?;
    validate_replay_interval(&mut right, boundary)?;
    let left_boundary = physical_boundary(&mut left, comparison.common_chunk_count)?;
    let left_graph = crate::cva_reconcile_graph::read_graph_tail(&mut left, left_boundary)?;
    let right_graph = crate::cva_reconcile_graph::read_graph_tail(&mut right, boundary)?;
    crate::cva_reconcile_graph::validate_graph_tails(&left_graph, &right_graph)?;
    if left.rel_metadata() != right.rel_metadata()
        || left.freshness_policy() != right.freshness_policy()
    {
        return Err(unsupported(
            "import replay owner metadata or policy conflict",
        ));
    }
    // Shared physical records cannot end halfway through an epoch: that would
    // require resolving competing accepted continuations of the same start.
    if right_epochs.values().any(|epoch| {
        epoch.first < boundary
            && epoch.end > boundary
            && !left_epochs.contains_key(&epoch.start.import_id)
    }) {
        return Err(unsupported(
            "divergent continuation of a shared import start",
        ));
    }
    let result = super::reconciliation_replay::replay(
        &mut left,
        &mut right,
        left_path,
        output_path,
        comparison,
        left_epochs,
        right_epochs,
        boundary,
    );
    if result.is_err() {
        let _ = std::fs::remove_file(output_path);
    }
    result.map(Some)
}

fn inventory(cva: &mut Cva) -> Result<BTreeMap<String, CompletedEpoch>, CvaReconcileError> {
    let mut positions = BTreeMap::new();
    let mut starts = BTreeMap::new();
    let chunks = cva.container.chunks()?;
    for (index, chunk) in chunks.into_iter().enumerate() {
        let payload = cva.container.read(chunk)?;
        match super::epoch_codec::decode(&payload).map_err(crate::CvaError::Import)? {
            Some(super::epoch_codec::ImportEpochRecord::Start(start)) => {
                if starts.insert(start.import_id, index).is_some() {
                    return Err(unsupported(
                        "repeated physical import start in replay source",
                    ));
                }
            }
            Some(super::epoch_codec::ImportEpochRecord::Complete(complete)) => {
                if positions.insert(complete.import_id, index + 1).is_some() {
                    return Err(unsupported(
                        "repeated physical import completion in replay source",
                    ));
                }
            }
            None => {}
        }
    }
    let mut result = BTreeMap::new();
    for status in cva.import_epoch_statuses() {
        let ImportEpochStatus::Completed { start, complete } = status else {
            return Err(unsupported("open import epoch in divergent reconciliation"));
        };
        let settlement = cva
            .freshness
            .import_settlements()
            .iter()
            .find(|settlement| hex(&settlement.import_id) == start.import_id)
            .cloned()
            .ok_or_else(|| unsupported("completed import lacks a scoped settlement"))?;
        let first = *starts
            .get(&start.import_id)
            .ok_or_else(|| unsupported("import start position unavailable"))?;
        let end = *positions
            .get(&start.import_id)
            .ok_or_else(|| unsupported("import completion position unavailable"))?;
        let mut completions = Vec::new();
        let mut archive_records = Vec::new();
        let mut events = Vec::new();
        let mut dream = Vec::new();
        let mut owner_evidence = Vec::new();
        let mut normalized_dream =
            crate::freshness_dream_journal::DreamFreshnessJournal::new(cva.owner_uuid());
        for chunk in cva.container.chunks()?.into_iter().take(end).skip(first) {
            let payload = cva.container.read(chunk)?;
            if let Some(version) = crate::archive_history_codec::decode_record_version(&payload)? {
                // Retain every accepted source-domain record in interval order,
                // including unrelated records not named by the import receipt.
                archive_records.push(cva.container.read(version.record)?);
            }
            if let Some(version) = crate::memory_codec::decode_version(&payload)? {
                let bytes = cva.container.read(version.record)?;
                if let Some(mut record) = crate::memory_codec::decode_record(&bytes)? {
                    record.global_version = 0;
                    record.memory_version = 0;
                    owner_evidence.push(crate::memory_codec::encode_record(&record)?);
                } else {
                    owner_evidence.push(bytes);
                }
            }
            if let Some(version) = crate::entity_codec::decode_version(&payload)? {
                let bytes = cva.container.read(version.record)?;
                if let Some(mut record) = crate::entity_codec::decode_record(&bytes)? {
                    record.global_version = 0;
                    record.entity_version = 0;
                    owner_evidence.push(crate::entity_codec::encode_record(&record)?);
                } else {
                    owner_evidence.push(bytes);
                }
            }
            if let Some(version) = crate::relationship_codec::decode_version(&payload)? {
                let bytes = cva.container.read(version.record)?;
                if let Some(mut record) = crate::relationship_codec::decode_record(&bytes)? {
                    record.global_version = 0;
                    record.relationship_version = 0;
                    owner_evidence.push(crate::relationship_codec::encode_record(&record)?);
                } else {
                    owner_evidence.push(bytes);
                }
            }
            if let Some(version) = crate::graph_codec::decode_version(&payload)? {
                owner_evidence.push(cva.container.read(version.mutation)?);
            }
            if let Some(mut snapshot) =
                crate::community_codec::decode_snapshot(&payload).map_err(crate::CvaError::from)?
            {
                snapshot.generation = 0;
                snapshot.derived_graph_version = 0;
                owner_evidence.push(
                    crate::community_codec::encode_snapshot(&snapshot)
                        .map_err(crate::CvaError::from)?,
                );
            }
            if crate::entity_resolution_codec::decode_resolution(&payload)?.is_some()
                || crate::memory_routing_codec::decode(&payload)?.is_some()
                || payload.starts_with(b"CVAPEP01")
            {
                owner_evidence.push(payload.clone());
            }
            for mut event in crate::freshness_storage::FreshnessReplayEntry::from_payload(&payload)
                .map_err(crate::CvaError::Freshness)?
            {
                use crate::freshness_storage::FreshnessReplayEntry as E;
                match &mut event {
                    E::Initialize { source_turn, .. }
                    | E::Admit { source_turn, .. }
                    | E::AcceptedUseReceipt { source_turn, .. } => *source_turn = 0,
                    E::Event {
                        source_turn,
                        proof,
                        provenance,
                        semantic,
                        ..
                    } => {
                        *source_turn = 0;
                        proof.graph_version = 0;
                        proof.community_generation = proof.community_generation.map(|_| 0);
                        proof.community_graph_version = proof.community_graph_version.map(|_| 0);
                        if let Some(p) = provenance {
                            p.accepted_turn = 0;
                        }
                        if let Some(e) = semantic {
                            e.accepted_turn = 0;
                        }
                    }
                }
                events.push(event);
            }
            if payload.starts_with(b"CVDREAM1") {
                let normalized = normalized_dream
                    .replay_payload(
                        &payload,
                        |_| Ok(0),
                        |_| Ok(0),
                        |mut p| {
                            p.graph_version = 0;
                            p.community_generation = p.community_generation.map(|_| 0);
                            p.community_graph_version = p.community_graph_version.map(|_| 0);
                            Ok(p)
                        },
                    )
                    .map_err(crate::CvaError::Freshness)?;
                normalized_dream
                    .ingest(&normalized)
                    .map_err(crate::CvaError::Freshness)?;
                if !normalized.is_empty() {
                    dream.push(normalized);
                }
            }
            if let Some(mut completion) = crate::insomnia::completion::decode_completion(&payload)
                .map_err(CvaReconcileError::InvalidInsomniaCompletion)?
            {
                // Global coordinates and transaction serialization time belong
                // to the source file, not to the accepted extraction identity.
                cva.hydrate_grouped_import_completion_bodies(&mut completion)?;
                completion.bodies.sort_by_key(|body| body.id.0);
                for (index, record) in completion.records.iter_mut().enumerate() {
                    let coordinate = u64::try_from(index)
                        .ok()
                        .and_then(|offset| offset.checked_add(1))
                        .ok_or_else(|| unsupported("too many grouped completion records"))?;
                    record.global_version = coordinate;
                    record.memory_version = coordinate;
                }
                completion.global_version_start = 1;
                completion.transaction_time_ns = 1;
                completion.freshness_birth_turn = completion.freshness_birth_turn.map(|_| 0);
                completions.push(
                    crate::insomnia::completion::encode_completion(&completion)
                        .map_err(CvaReconcileError::InvalidInsomniaCompletion)?,
                );
            }
        }
        result.insert(
            start.import_id.clone(),
            CompletedEpoch {
                start,
                complete,
                settlement,
                first,
                end,
                completions,
                archive_records,
                events,
                dream,
                owner_evidence,
            },
        );
    }
    Ok(result)
}

fn validate_replay_interval(cva: &mut Cva, from: usize) -> Result<(), CvaReconcileError> {
    // Inspect the actual divergent payloads, not just current owner counts:
    // terminal/deleted derived state is still retained authority.
    let chunks = cva.container.chunks()?;
    for chunk in chunks.into_iter().skip(from) {
        let payload = cva.container.read(chunk)?;
        if let Some(completion) = crate::insomnia::completion::decode_completion(&payload)
            .map_err(CvaReconcileError::InvalidInsomniaCompletion)?
        {
            if !completion.external_memory_refs.is_empty() {
                return Err(unsupported("import interval with external PHY publication"));
            }
            continue;
        }
        if payload.starts_with(b"CVAFRS05") {
            HistoricalImportFreshnessSettlement::decode(&payload)
                .map_err(crate::CvaError::Freshness)?;
            continue;
        }
        if super::epoch_codec::decode(&payload)
            .map_err(crate::CvaError::Import)?
            .is_some()
        {
            continue;
        }
        if !matches!(
            crate::archive_codec::decode_record(&payload)?,
            crate::archive_codec::ArchiveRecord::Other
        ) {
            continue;
        }
        // Each accepted owner is replayed through its concrete codec/API;
        // physical format and payload frames are consumed by their version record.
        if crate::insomnia::codec::decode_record(&payload)?.is_some()
            || crate::memory_codec::decode_body(&payload)?.is_some()
            || crate::memory_codec::decode_record(&payload)?.is_some()
            || crate::memory_codec::decode_version(&payload)?.is_some()
            || crate::memory_routing_codec::decode(&payload)?.is_some()
            || crate::entity_codec::decode_record(&payload)?.is_some()
            || crate::entity_codec::decode_version(&payload)?.is_some()
            || crate::relationship_codec::decode_record(&payload)?.is_some()
            || crate::relationship_codec::decode_version(&payload)?.is_some()
            || crate::entity_resolution_codec::decode_resolution(&payload)?.is_some()
            || crate::graph_codec::decode_node(&payload)?.is_some()
            || crate::graph_codec::decode_version(&payload)?.is_some()
            || crate::graph_codec::decode_mutation(&payload)?.is_some()
            || crate::graph_codec::decode_batch(&payload)?.is_some()
            || crate::community_codec::decode_snapshot(&payload)
                .map_err(crate::CvaError::from)?
                .is_some()
            || !crate::freshness_storage::FreshnessReplayEntry::from_payload(&payload)
                .map_err(crate::CvaError::Freshness)?
                .is_empty()
            || payload.starts_with(b"CVDREAM1")
            || payload.starts_with(b"CVAPEP01")
        {
            continue;
        }
        let magic = payload.get(..8).unwrap_or(&[]);
        if ![
            b"CVAAREC1".as_slice(),
            b"CVAAFMT2",
            b"CVAVERS1",
            b"CVAVERS2",
            b"CVAINSF1",
            b"CVAFRP01",
            b"CVAMEMF2",
            b"CVAENTF1",
            b"CVARELF1",
            b"CVAGFMT1",
        ]
        .contains(&magic)
        {
            return Err(unsupported("unhandled owner payload in import interval"));
        }
    }
    // Dropping an outstanding queue/lease/failure would lose owner authority.
    for episode in cva.archive.episodes.iter() {
        if cva
            .insomnia_work(episode.id)
            .is_some_and(|work| work.state != crate::InsomniaWorkState::Complete)
        {
            return Err(unsupported(
                "outstanding Insomnia work in import reconciliation source",
            ));
        }
    }
    Ok(())
}

pub(super) fn unsupported(reason: &'static str) -> CvaReconcileError {
    CvaReconcileError::UnsupportedSemanticOwner(reason)
}
fn hex(id: &[u8; 32]) -> String {
    id.iter().map(|byte| format!("{byte:02x}")).collect()
}

// Exact source-only authority can occur at different physical coordinates.
// Do not compare prefix/manifest digests as semantic import identities. This
// normalization is deliberately confined to empty derived effect sets.
fn same_authority(a: &CompletedEpoch, b: &CompletedEpoch) -> bool {
    let mut a_start = a.start.clone();
    let mut b_start = b.start.clone();
    a_start.pre = Default::default();
    b_start.pre = Default::default();
    if a_start != b_start
        || a.completions != b.completions
        || a.archive_records != b.archive_records
        || a.events != b.events
        || a.dream != b.dream
        || a.owner_evidence != b.owner_evidence
    {
        return false;
    }
    let records = |settlement: &HistoricalImportFreshnessSettlement| {
        settlement
            .records
            .iter()
            .map(|(id, record)| {
                (
                    *id,
                    record.score,
                    record.admitted_at_turn.is_some(),
                    record.accounted_turn.is_some(),
                )
            })
            .collect::<Vec<_>>()
    };
    a.settlement.cohort == b.settlement.cohort
        && a.settlement.superseded == b.settlement.superseded
        && records(&a.settlement) == records(&b.settlement)
        && a.settlement.epoch.as_ref().unwrap().affected_recipients
            == b.settlement.epoch.as_ref().unwrap().affected_recipients
        && a.complete.owner_uuid == b.complete.owner_uuid
        && a.complete.import_id == b.complete.import_id
        && a.complete.receipt_digest == b.complete.receipt_digest
        && a.complete.completed_at_ns == b.complete.completed_at_ns
        && a.settlement.epoch.as_ref().unwrap().source_nodes
            == b.settlement.epoch.as_ref().unwrap().source_nodes
        && a.settlement.epoch.as_ref().unwrap().born_memories
            == b.settlement.epoch.as_ref().unwrap().born_memories
}

fn physical_boundary(cva: &mut Cva, common: usize) -> Result<usize, CvaReconcileError> {
    if common == 0 {
        return Ok(0);
    }
    let mut semantic = 0;
    for (index, chunk) in cva.container.chunks()?.into_iter().enumerate() {
        let payload = cva.container.read(chunk)?;
        if !crate::conversation_compaction_codec::is_compaction_payload(&payload) {
            semantic += 1;
            if semantic == common {
                return Ok(index + 1);
            }
        }
    }
    Err(unsupported("common import boundary unavailable"))
}
