//! Native owner completion entry point for Import's completion policy.
use crate::imports::{ImportCompletionOutcome, ImportWatermarks};
use crate::owner_lifecycle::{ImportSession, OwnerError, ReliquaryOwner};

/// Complete an open import using accepted facts from the concrete native owner.
/// The ordering mirrors the former facade coordinator: validate retained
/// settlements, recover durable epoch state, then apply start/receipt gates,
/// derive the exact scope and readiness inputs, and finally publish completion.
pub(crate) fn complete(
    owner: &mut ReliquaryOwner,
    import_id: &str,
    now_ns: i64,
) -> Result<ImportCompletionOutcome, OwnerError> {
    validate_retained_settlements(owner)?;

    let mut session = ImportSession::offline_rel(&mut owner.container)?;
    let (mut epochs, recovered) = crate::imports::completion_coordinator::recover_import(
        &mut session,
        &owner.archive,
        import_id,
    )?;
    let start = match recovered {
        crate::imports::completion_coordinator::RecoveredImport::Completed(outcome) => {
            owner.import_epochs = epochs;
            return Ok(outcome);
        }
        crate::imports::completion_coordinator::RecoveredImport::Open(start) => start,
    };

    // Preserve the accepted recovered prefix even when a later start/receipt
    // validation refuses completion.
    owner.import_epochs = epochs.clone();
    drop(session);

    crate::imports::completion_coordinator::validate_started_at(&start, now_ns)?;
    let Some(receipt) = owner
        .archive
        .import_receipts
        .get(&start.import_key)
        .cloned()
    else {
        return Ok(crate::imports::completion_coordinator::missing_receipt());
    };
    crate::imports::completion_coordinator::validate_receipt(&start, &receipt)?;

    let settlement = owner
        .freshness
        .import_settlements()
        .iter()
        .find(|settlement| {
            settlement
                .import_id
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
                == start.import_id
        })
        .cloned();
    let history = owner.history_read_access();
    let settlement_owners = crate::history::HistoricalSettlementReadOwners::new(
        &owner.archive,
        &owner.memories,
        &owner.graph,
        &owner.dream_freshness,
        &owner.import_epochs,
        &owner.freshness,
    );
    let scope = settlement_owners.epoch_scope(&history, start.clone())?;
    let current_watermarks = watermarks(owner);
    let current_turn = owner.archive.rel_turn_count();
    let insomnia = crate::insomnia::import_readiness::ImportReadiness::new(&owner.insomnia);
    let dream = crate::dream_readiness::DreamReadiness::new(
        &owner.dream_freshness,
        &owner.processing_epochs,
    );
    let perception = crate::perception_import_readiness::ImportReadiness::new(
        &owner.memories,
        &owner.entity_resolutions,
    );

    let mut session = ImportSession::offline_rel(&mut owner.container)?;
    let outcome = crate::imports::completion_coordinator::complete_import(
        &mut session,
        &mut epochs,
        &start,
        &receipt,
        settlement.as_ref(),
        &scope,
        &owner.archive,
        &owner.memories,
        &insomnia,
        &dream,
        &perception,
        current_turn,
        current_watermarks,
        now_ns,
    )?;
    owner.import_epochs = epochs;
    Ok(outcome)
}

fn validate_retained_settlements(owner: &ReliquaryOwner) -> Result<(), OwnerError> {
    let access = owner.history_read_access();
    let owners = crate::history::HistoricalSettlementReadOwners::new(
        &owner.archive,
        &owner.memories,
        &owner.graph,
        &owner.dream_freshness,
        &owner.import_epochs,
        &owner.freshness,
    );
    for settlement in owner.freshness.import_settlements().to_vec() {
        owners.validate(&access, &settlement)?;
    }
    Ok(())
}

fn watermarks(owner: &ReliquaryOwner) -> ImportWatermarks {
    ImportWatermarks {
        global_version: owner.latest_global_version(),
        archive_version: owner.archive.archive_version(),
        memory_version: owner.memories.memory_version(),
        entity_version: owner.entities.entity_version(),
        relationship_version: owner.relationships.relationship_version(),
        graph_version: owner.graph.graph_version(),
        memory_graph_version: owner.graph.memory_graph_version(),
        vector_version: owner.vector_generations.vector_version(),
    }
}
