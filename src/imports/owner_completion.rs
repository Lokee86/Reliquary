//! Native owner entry point for Import's one canonical completion sequence.
use crate::Archive;
use crate::dream_readiness::DreamReadiness;
use crate::imports::{ImportCompletionOutcome, ImportEpochStore, ImportWatermarks};
use crate::insomnia::import_readiness::ImportReadiness as InsomniaReadiness;
use crate::memory_store::MemoryStore;
use crate::owner_lifecycle::{ImportCompletionAccess, OwnerError, ReliquaryOwner};
use crate::perception_import_readiness::ImportReadiness as PerceptionReadiness;

/// Immutable owner evidence captured for this completion request.
pub(crate) struct CompletionReadViews<'a> {
    pub(crate) archive: &'a Archive,
    pub(crate) memories: &'a MemoryStore,
    pub(crate) settlement: crate::history::SettlementCompletionEvidence<'a>,
    pub(crate) insomnia: InsomniaReadiness<'a>,
    pub(crate) dream: DreamReadiness<'a>,
    pub(crate) perception: PerceptionReadiness<'a>,
    pub(crate) watermarks: ImportWatermarks,
    pub(crate) turn: u64,
}

/// Complete an open import using accepted facts from a concrete native owner.
/// This wrapper gathers owner views; `complete_with_views` owns the sequence.
pub(crate) fn complete(
    owner: &mut ReliquaryOwner,
    import_id: &str,
    now_ns: i64,
) -> Result<ImportCompletionOutcome, OwnerError> {
    let views = CompletionReadViews {
        archive: &owner.archive,
        memories: &owner.memories,
        settlement: crate::history::SettlementCompletionEvidence::new(
            &owner.archive,
            &owner.memories,
            &owner.graph,
            &owner.freshness,
            &owner.dream_freshness,
        ),
        insomnia: InsomniaReadiness::new(&owner.insomnia),
        dream: DreamReadiness::new(&owner.dream_freshness, &owner.processing_epochs),
        perception: PerceptionReadiness::new(&owner.memories, &owner.entity_resolutions),
        watermarks: watermarks(owner),
        turn: owner.archive.rel_turn_count(),
    };
    let mut access = ImportCompletionAccess::new(&mut owner.container);
    complete_with_views(
        &mut access,
        &mut owner.import_epochs,
        import_id,
        now_ns,
        views,
    )
}

/// The facade supplies its concrete constituent views and this lifecycle
/// access; all recovery, refusal and publication ordering remains here.
pub(crate) fn complete_with_views(
    access: &mut ImportCompletionAccess<'_>,
    epochs: &mut ImportEpochStore,
    import_id: &str,
    now_ns: i64,
    views: CompletionReadViews<'_>,
) -> Result<ImportCompletionOutcome, OwnerError> {
    validate_retained_settlements(access, epochs, &views)?;

    let mut session = access.session()?;
    let (mut recovered_epochs, recovered) = crate::imports::completion_coordinator::recover_import(
        &mut session,
        views.archive,
        import_id,
    )?;
    let start = match recovered {
        crate::imports::completion_coordinator::RecoveredImport::Completed(outcome) => {
            *epochs = recovered_epochs;
            return Ok(outcome);
        }
        crate::imports::completion_coordinator::RecoveredImport::Open(start) => start,
    };

    // Keep the accepted recovered prefix even if a later time or receipt gate
    // refuses completion, matching the facade's former state transition.
    *epochs = recovered_epochs.clone();
    drop(session);

    crate::imports::completion_coordinator::validate_started_at(&start, now_ns)?;
    let Some(receipt) = views
        .archive
        .import_receipts
        .get(&start.import_key)
        .cloned()
    else {
        return Ok(crate::imports::completion_coordinator::missing_receipt());
    };
    crate::imports::completion_coordinator::validate_receipt(&start, &receipt)?;

    let settlement = views.settlement.settlement(&start.import_id);
    let scope = crate::history::read_import_epoch_scope(
        &access.history_access(),
        views.archive,
        views.memories,
        start.clone(),
    )?;
    let mut session = access.session()?;
    let outcome = crate::imports::completion_coordinator::complete_import(
        &mut session,
        &mut recovered_epochs,
        &start,
        &receipt,
        settlement.as_ref(),
        &scope,
        views.archive,
        views.memories,
        &views.insomnia,
        &views.dream,
        &views.perception,
        views.turn,
        views.watermarks,
        now_ns,
    )?;
    *epochs = recovered_epochs;
    Ok(outcome)
}

fn validate_retained_settlements(
    access: &ImportCompletionAccess<'_>,
    epochs: &ImportEpochStore,
    views: &CompletionReadViews<'_>,
) -> Result<(), OwnerError> {
    let history = access.history_access();
    views.settlement.validate_retained(&history, epochs)
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
