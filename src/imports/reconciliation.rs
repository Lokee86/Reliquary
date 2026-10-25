//! Ordered reconciliation of completed native import epochs through their owners.
#[cfg(test)]
#[path = "reconciliation_archive_envelope_tests.rs"]
mod archive_envelope_tests;
#[cfg(test)]
#[path = "reconciliation_entity_compact_tests.rs"]
mod entity_compact_tests;
#[cfg(test)]
#[path = "reconciliation_freshness_authority_tests.rs"]
mod freshness_authority_tests;
#[cfg(test)]
#[path = "reconciliation_graph_compact_tests.rs"]
mod graph_compact_tests;
#[cfg(test)]
#[path = "reconciliation_graph_envelope_tests.rs"]
mod graph_envelope_tests;
#[cfg(test)]
#[path = "reconciliation_memory_envelope_tests.rs"]
mod memory_envelope_tests;
#[cfg(test)]
#[path = "reconciliation_resolution_delta_tests.rs"]
mod resolution_delta_tests;
use super::{ImportComplete, ImportEpochStatus, ImportStart};
use crate::CvaReconcileError;
use crate::freshness_storage::HistoricalImportFreshnessSettlement;
use crate::owner_lifecycle::ReliquaryOwner;
use crate::reconciliation_values::{CvaComparison, CvaReconcileResult};
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
    accepted_events: BTreeMap<String, crate::freshness_storage::FreshnessReplayEntry>,
    dream: Vec<Vec<u8>>,
    owner_evidence: Vec<Vec<u8>>,
}

pub(crate) fn reconcile_import_epochs(
    left_path: &Path,
    right_path: &Path,
    output_path: &Path,
    comparison: CvaComparison,
) -> Result<
    Option<(
        CvaReconcileResult,
        crate::owner_lifecycle::ReconstructedReliquary,
    )>,
    CvaReconcileError,
> {
    let mut left = super::reconciliation_copy::open_read_only(left_path)?;
    let mut right = super::reconciliation_copy::open_read_only(right_path)?;
    if left.import_epochs.statuses().is_empty() && right.import_epochs.statuses().is_empty() {
        return Ok(None);
    }
    if left.import_epochs.open().is_some() || right.import_epochs.open().is_some() {
        return Err(unsupported("open import epoch in divergent reconciliation"));
    }
    // A conflict in source revision authority is independent of replay support.
    // Detect it before copying a destination or comparing coordinate-bound data.
    for receipt in right.archive.import_receipts.values().cloned() {
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
    let boundary = if comparison.common_chunk_count == 0 {
        0
    } else {
        let source = crate::owner_lifecycle::HistoryReadAccess::new(&right.container)
            .snapshot()
            .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?;
        physical_boundary(&source, comparison.common_chunk_count)?
    };
    let replay_source = crate::owner_lifecycle::HistoryReadAccess::new(&right.container)
        .snapshot()
        .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?;
    validate_replay_interval(
        &replay_source,
        &right.archive,
        &crate::insomnia::import_readiness::ImportReadiness::new(&right.insomnia),
        boundary,
    )?;
    let left_boundary = if comparison.common_chunk_count == 0 {
        0
    } else {
        let source = crate::owner_lifecycle::HistoryReadAccess::new(&left.container)
            .snapshot()
            .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?;
        physical_boundary(&source, comparison.common_chunk_count)?
    };
    let left_graph =
        super::reconciliation_interval::read_graph_range(&left, left_boundary, usize::MAX)?;
    let right_graph =
        super::reconciliation_interval::read_graph_range(&right, boundary, usize::MAX)?;
    crate::history::reconciliation_inventory::validate_graph_overlap(&left_graph, &right_graph)?;
    if left.rel_metadata.effective(&left.container)
        != right.rel_metadata.effective(&right.container)
        || !crate::owner_lifecycle::import_reconciliation_restore::freshness_policies_match(
            &left, &right,
        )
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
    result.map(Some)
}

fn inventory(cva: &ReliquaryOwner) -> Result<BTreeMap<String, CompletedEpoch>, CvaReconcileError> {
    let history = crate::history::reconciliation_inventory::ReconciliationRead::new(
        crate::owner_lifecycle::HistoryReadAccess::new(&cva.container),
        &cva.archive,
        &cva.memories,
        &cva.freshness,
        &cva.entities,
        &cva.relationships,
        &cva.graph,
    );
    let statuses = cva.import_epochs.statuses();
    inventory_from_history(&history, &statuses)
}

/// Import policy over narrow, immutable History and owner read views. All
/// physical source reads and Container-dependent materialization stay below
/// this boundary in History.
fn inventory_from_history(
    history: &crate::history::reconciliation_inventory::ReconciliationRead<'_>,
    statuses: &[ImportEpochStatus],
) -> Result<BTreeMap<String, CompletedEpoch>, CvaReconcileError> {
    let resolution_changes = history.materialized_resolution_changes()?;
    let mut positions = BTreeMap::new();
    let mut starts = BTreeMap::new();
    let source = history.snapshot().map_err(|error| error.0)?;
    let chunks = history.physical_objects(&source)?;
    for (index, chunk) in chunks.into_iter().enumerate() {
        let payload = history.read_object(&source, chunk)?;
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
    for status in statuses.iter().cloned() {
        let ImportEpochStatus::Completed { start, complete } = status else {
            return Err(unsupported("open import epoch in divergent reconciliation"));
        };
        let settlement = history
            .settlement(&start.import_id)
            .ok_or_else(|| unsupported("completed import lacks a scoped settlement"))?;
        let semantic_scope = settlement.semantic.clone().ok_or_else(|| {
            crate::CvaError::Freshness("accepted settlement lacks semantic authority".into())
        })?;
        let mut accepted_events = history
            .event_entries(&semantic_scope.accepted_events.keys().cloned().collect())
            .map_err(crate::CvaError::Freshness)?;
        for event in accepted_events.values_mut() {
            normalize_event_coordinates(event);
        }
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
        let mut normalizer = history.epoch_normalizer();
        for chunk in history
            .physical_objects(&source)?
            .into_iter()
            .take(end)
            .skip(first)
        {
            let payload = history.read_object(&source, chunk)?;
            let frame = history.frame_evidence(
                &source,
                chunk,
                &payload,
                resolution_changes.get(&chunk),
                &mut normalizer,
            )?;
            // Retain every accepted source-domain Archive record in interval
            // order, including records not named by the import receipt.
            archive_records.extend(frame.archive_records);
            owner_evidence.extend(frame.owner_evidence);
            for mut event in frame.events {
                normalize_event_coordinates(&mut event);
                events.push(event);
            }
            if let Some(bytes) = frame.dream {
                dream.push(bytes);
            }
            if let Some(completion) = frame.completion {
                completions.push(completion);
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
                accepted_events,
                dream,
                owner_evidence,
            },
        );
    }
    Ok(result)
}

fn normalize_event_coordinates(event: &mut crate::freshness_storage::FreshnessReplayEntry) {
    use crate::freshness_storage::FreshnessReplayEntry as E;
    match event {
        E::RewriteBaseline { .. } => {}
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
}

fn validate_replay_interval(
    source: &crate::owner_lifecycle::HistorySource<'_>,
    archive: &crate::Archive,
    insomnia: &crate::insomnia::import_readiness::ImportReadiness<'_>,
    from: usize,
) -> Result<(), CvaReconcileError> {
    // Inspect the actual divergent payloads, not just current owner counts:
    // terminal/deleted derived state is still retained authority.
    // Preserve Container::chunks()' full-source framing preflight before any
    // payload-specific refusal: first collect every physical object, then read
    // and classify only the requested interval.
    let chunks = {
        let mut cursor = source
            .cursor()
            .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?;
        let mut chunks = Vec::new();
        while let Some(frame) = cursor
            .next_frame()
            .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?
        {
            chunks.push(frame.object);
        }
        chunks
    };
    for chunk in chunks.into_iter().skip(from) {
        let payload = source
            .read_object(chunk)
            .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?;
        if payload.starts_with(b"CVAECR")
            || payload.starts_with(b"CVAEDL")
            || (payload.starts_with(b"CVAETM") && !payload.starts_with(b"CVAETM02"))
            || payload.starts_with(b"CVAEIDA")
        {
            return Err(unsupported("unhandled owner payload in import interval"));
        }
        if let Some(completion) =
            crate::owner_lifecycle::insomnia_import_replay::decode_completion(&payload)
                .map_err(CvaReconcileError::InvalidInsomniaCompletion)?
        {
            if !completion.external_memory_refs.is_empty() {
                return Err(unsupported("import interval with external PHY publication"));
            }
            continue;
        }
        if payload.starts_with(b"CVAFRS07") {
            crate::freshness::historical_scorer::validate_encoded_settlement_structure(&payload)
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
        if crate::archive_codec::publication::version_at(chunk, &payload)?.is_some() {
            continue;
        }
        // Accepted publications are replayed through current owner envelopes.
        // Unpublished bare native bodies carry no semantic clock.
        if crate::history::import_replay_read::has_owner_record(chunk, &payload)? {
            continue;
        }
        let magic = payload.get(..8).unwrap_or(&[]);
        if ![
            b"CVAAFMT2".as_slice(),
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
    for episode in archive.episodes.iter() {
        if insomnia
            .state(episode.id)
            .is_some_and(|state| state != crate::InsomniaWorkState::Complete)
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

#[cfg(test)]
#[path = "reconciliation_compact_memory_tests.rs"]
mod compact_memory_tests;
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
        || a.accepted_events != b.accepted_events
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
        && a.settlement.scope_identity().unwrap().affected_recipients
            == b.settlement.scope_identity().unwrap().affected_recipients
        && a.complete.owner_uuid == b.complete.owner_uuid
        && a.complete.import_id == b.complete.import_id
        && a.complete.receipt_digest == b.complete.receipt_digest
        && a.complete.completed_at_ns == b.complete.completed_at_ns
        && a.settlement.scope_identity().unwrap().source_nodes
            == b.settlement.scope_identity().unwrap().source_nodes
        && a.settlement.scope_identity().unwrap().born_memories
            == b.settlement.scope_identity().unwrap().born_memories
}

fn physical_boundary(
    source: &crate::owner_lifecycle::HistorySource<'_>,
    common: usize,
) -> Result<usize, CvaReconcileError> {
    if common == 0 {
        return Ok(0);
    }
    let mut semantic = 0;
    // Match the old chunks()+read() checkpoint: validate physical framing for
    // the complete source before classifying an earlier common boundary.
    let chunks = {
        let mut cursor = source
            .cursor()
            .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?;
        let mut chunks = Vec::new();
        while let Some(frame) = cursor
            .next_frame()
            .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?
        {
            chunks.push(frame.object);
        }
        chunks
    };
    for (index, chunk) in chunks.into_iter().enumerate() {
        let payload = source
            .read_object(chunk)
            .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?;
        if !crate::conversation_compaction_codec::is_compaction_payload(&payload) {
            semantic += 1;
            if semantic == common {
                return Ok(index + 1);
            }
        }
    }
    Err(unsupported("common import boundary unavailable"))
}

#[cfg(test)]
#[path = "reconciliation_ego_compact_tests.rs"]
mod ego_compact_tests;
#[cfg(test)]
#[path = "reconciliation_relationship_compact_tests.rs"]
mod relationship_compact_tests;
