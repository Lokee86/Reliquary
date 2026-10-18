//! Ordered reconciliation of completed source-only native import epochs.
//! General derived-owner replay remains refused until its grouped publication
//! and accepted event evidence can be rebased through their native owners.
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
    validate_source_only(&mut right, boundary)?;
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
        for chunk in cva.container.chunks()?.into_iter().take(end).skip(first) {
            let payload = cva.container.read(chunk)?;
            if let Some(version) = crate::archive_history_codec::decode_record_version(&payload)? {
                // Retain every accepted source-domain record in interval order,
                // including unrelated records not named by the import receipt.
                archive_records.push(cva.container.read(version.record)?);
            }
            if let Some(mut completion) = crate::insomnia::completion::decode_completion(&payload)
                .map_err(CvaReconcileError::InvalidInsomniaCompletion)?
            {
                // Global coordinates and transaction serialization time belong
                // to the source file, not to the accepted extraction identity.
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
            },
        );
    }
    Ok(result)
}

fn validate_source_only(cva: &mut Cva, from: usize) -> Result<(), CvaReconcileError> {
    // Inspect the actual divergent payloads, not just current owner counts:
    // terminal/deleted derived state is still retained authority.
    let chunks = cva.container.chunks()?;
    for chunk in chunks.into_iter().skip(from) {
        let payload = cva.container.read(chunk)?;
        if let Some(completion) = crate::insomnia::completion::decode_completion(&payload)
            .map_err(CvaReconcileError::InvalidInsomniaCompletion)?
        {
            if !completion.memory_ids.is_empty()
                || !completion.records.is_empty()
                || !completion.bodies.is_empty()
                || !completion.routing_metadata.is_empty()
                || !completion.external_memory_refs.is_empty()
                || !completion.freshness_birth_memory_ids.is_empty()
            {
                return Err(unsupported(
                    "import interval with derived Memory publication",
                ));
            }
            continue;
        }
        if payload.starts_with(b"CVAFRS05") {
            let settlement = HistoricalImportFreshnessSettlement::decode(&payload)
                .map_err(crate::CvaError::Freshness)?;
            if !settlement.records.is_empty()
                || !settlement.cohort.is_empty()
                || !settlement.superseded.is_empty()
                || settlement
                    .epoch
                    .as_ref()
                    .is_none_or(|scope| !scope.born_memories.is_empty())
            {
                return Err(unsupported(
                    "import interval with accepted derived Freshness effects",
                ));
            }
            continue;
        }
        if super::epoch_codec::decode(&payload)
            .map_err(crate::CvaError::Import)?
            .is_some()
        {
            continue;
        }
        match crate::archive_codec::decode_record(&payload)? {
            crate::archive_codec::ArchiveRecord::FileMemoryLink(_)
            | crate::archive_codec::ArchiveRecord::File(_) => {
                return Err(unsupported("import interval with file publication"));
            }
            crate::archive_codec::ArchiveRecord::Other => {}
            _ => continue,
        }
        let magic = payload.get(..8).unwrap_or(&[]);
        if ![
            b"CVAAREC1".as_slice(),
            b"CVAAFMT2",
            b"CVAVERS1",
            b"CVAVERS2",
            b"CVAINSF1",
            b"CVAFRP01",
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
    {
        return false;
    }
    let empty = |s: &HistoricalImportFreshnessSettlement| {
        s.records.is_empty()
            && s.cohort.is_empty()
            && s.superseded.is_empty()
            && s.epoch.as_ref().is_some_and(|e| e.born_memories.is_empty())
    };
    if !empty(&a.settlement) || !empty(&b.settlement) {
        return a.start == b.start && a.complete == b.complete && a.settlement == b.settlement;
    }
    a.complete.owner_uuid == b.complete.owner_uuid
        && a.complete.import_id == b.complete.import_id
        && a.complete.receipt_digest == b.complete.receipt_digest
        && a.complete.completed_at_ns == b.complete.completed_at_ns
        && a.settlement.epoch.as_ref().unwrap().source_nodes
            == b.settlement.epoch.as_ref().unwrap().source_nodes
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
