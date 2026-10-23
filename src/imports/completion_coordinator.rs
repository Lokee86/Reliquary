//! Import-owned completion policy over accepted owner facts.
use crate::Archive;
use crate::dream_readiness::DreamReadiness;
use crate::freshness_storage::HistoricalImportFreshnessSettlement;
use crate::imports::readiness;
use crate::imports::{
    ImportComplete, ImportCompletionOutcome, ImportCompletionPending, ImportEpochScope,
    ImportEpochStatus, ImportEpochStore, ImportReceipt, ImportStart, ImportWatermarks, prepare,
};
use crate::insomnia::import_readiness::ImportReadiness as InsomniaReadiness;
use crate::memory_store::MemoryStore;
use crate::owner_lifecycle::HistoryReadAccess;
use crate::owner_lifecycle::{ImportSession, OwnerError};
use crate::perception_import_readiness::ImportReadiness as PerceptionReadiness;
use sha2::{Digest, Sha256};

fn epoch_error(message: &'static str) -> OwnerError {
    OwnerError::Import(crate::imports::ImportError::Epoch(message))
}

fn settlement_digest(settlement: &HistoricalImportFreshnessSettlement) -> String {
    format!("{:x}", Sha256::digest(settlement.encode()))
}

pub(crate) fn recover_epochs(
    access: &HistoryReadAccess<'_>,
    archive: &Archive,
) -> Result<ImportEpochStore, OwnerError> {
    let source = access
        .snapshot()
        .map_err(|error| OwnerError::Container(error.0))?;
    crate::history::completion_epochs(&source, &archive.import_receipts)
}

pub(crate) enum RecoveredImport {
    Open(ImportStart),
    Completed(ImportCompletionOutcome),
}

/// Recover the accepted epoch prefix and apply the durable status transition
/// for this request. Keeping the bounded scan beside the status/fence decision
/// makes the Import coordinator the sole owner of recovery policy.
pub(crate) fn recover_import(
    session: &mut ImportSession<'_>,
    archive: &Archive,
    import_id: &str,
) -> Result<(ImportEpochStore, RecoveredImport), OwnerError> {
    let access = session.history_access();
    let epochs = recover_epochs(&access, archive)?;
    drop(access);
    let recovered = recover_status(session, &epochs, import_id)?;
    Ok((epochs, recovered))
}

pub(crate) fn recover_status(
    session: &mut ImportSession<'_>,
    epochs: &ImportEpochStore,
    import_id: &str,
) -> Result<RecoveredImport, OwnerError> {
    let status = epochs
        .status(import_id)
        .ok_or_else(|| epoch_error("unknown import completion identity"))?;
    match status {
        ImportEpochStatus::Completed { complete, .. } => {
            session.sync_completed_retry(epochs)?;
            Ok(RecoveredImport::Completed(
                ImportCompletionOutcome::Completed {
                    complete,
                    committed: false,
                },
            ))
        }
        ImportEpochStatus::Open(start) => {
            session.hold_open_import_fence();
            Ok(RecoveredImport::Open(start))
        }
    }
}

pub(crate) fn validate_started_at(start: &ImportStart, now_ns: i64) -> Result<(), OwnerError> {
    if now_ns < start.started_at_ns {
        return Err(epoch_error("import completion precedes its start"));
    }
    Ok(())
}

pub(crate) fn validate_receipt(
    start: &ImportStart,
    receipt: &ImportReceipt,
) -> Result<(), OwnerError> {
    if receipt.source_namespace != start.source_namespace
        || receipt.source_format != start.source_format
    {
        return Err(epoch_error("completion receipt/start identity mismatch"));
    }
    Ok(())
}

pub(crate) fn missing_receipt() -> ImportCompletionOutcome {
    ImportCompletionOutcome::Pending(ImportCompletionPending::MissingReceipt)
}

/// Complete an already identified open epoch. The facade performs physical
/// epoch recovery and the earlier start/receipt gates; all readiness, settlement
/// acceptance and completion publication policy lives here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn complete_import(
    session: &mut ImportSession<'_>,
    epochs: &mut ImportEpochStore,
    start: &ImportStart,
    receipt: &ImportReceipt,
    settlement: Option<&HistoricalImportFreshnessSettlement>,
    scope: &ImportEpochScope,
    archive: &Archive,
    memories: &MemoryStore,
    insomnia: &InsomniaReadiness<'_>,
    dream: &DreamReadiness<'_>,
    perception: &PerceptionReadiness<'_>,
    current_turn: u64,
    current_watermarks: ImportWatermarks,
    now_ns: i64,
) -> Result<ImportCompletionOutcome, OwnerError> {
    let read_access = session.history_access();
    if let Some(blocker) = readiness::derive_import_readiness(
        start,
        receipt,
        settlement,
        scope,
        archive,
        memories,
        insomnia,
        dream,
        perception,
        &read_access,
        current_turn,
        current_watermarks,
    )? {
        return Ok(ImportCompletionOutcome::Pending(
            ImportCompletionPending::Readiness(blocker),
        ));
    }
    let Some(settlement) = settlement else {
        return Ok(ImportCompletionOutcome::Pending(
            ImportCompletionPending::MissingFreshnessSettlement,
        ));
    };
    crate::history::validate_import_binding(start, settlement)?;
    if !readiness::settlement_is_current(settlement, current_turn, current_watermarks) {
        return Err(OwnerError::Import(crate::imports::ImportError::Epoch(
            "completion settlement cut is stale",
        )));
    }
    let complete = ImportComplete {
        owner_uuid: start.owner_uuid,
        import_id: start.import_id.clone(),
        start_digest: prepare::digest(start)?,
        receipt_digest: prepare::digest(receipt)?,
        completed_at_ns: now_ns,
        post: current_watermarks,
        historical_freshness_settlement_digest: settlement_digest(settlement),
    };
    let publication =
        crate::imports::epoch_store::ImportCompletionPublication::new(complete.clone());
    session.publish_completion(epochs, publication)?;
    Ok(ImportCompletionOutcome::Completed {
        complete,
        committed: true,
    })
}

#[cfg(test)]
#[path = "completion_tests.rs"]
mod tests;
