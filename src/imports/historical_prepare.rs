//! Import-owned historical freeze over bounded read capabilities.
use super::historical_freeze::{self, HistoricalOwnerCut};
use super::historical_manifest::{self, HistoricalManifestReadViews};
use super::historical_settlement_plan::FrozenHistoricalImport;
use crate::owner_lifecycle::OwnerError;
fn fail(message: &str) -> OwnerError {
    OwnerError::Freshness(message.into())
}

pub(crate) fn prepare(
    views: HistoricalManifestReadViews<'_>,
    open_epoch: Option<super::ImportStart>,
    requested_complete: bool,
    #[cfg(feature = "freshness-performance-trace")] traces: &mut Vec<
        crate::history::dream_evidence::DreamEvidenceTrace,
    >,
) -> Result<FrozenHistoricalImport, OwnerError> {
    let history = views.history;
    let archive = views.archive;
    let memories = views.memories;
    let freshness = views.freshness;
    let dream = views.dream;
    let vectors = views.vectors;
    // The read operation cannot mutate these accepted in-memory owner clocks.
    let mut cut = HistoricalOwnerCut {
        owner: [0; 16],
        activity: views.cut.activity_cut,
        global: views.cut.global_version,
        archive: views.cut.archive_version,
        graph: views.cut.graph_version,
        memory_graph: views.cut.memory_graph_version,
    };
    let epoch_scope = || -> Result<Option<crate::history::ImportEpochScope>, OwnerError> {
        open_epoch
            .clone()
            .map(|start| crate::history::read_import_epoch_scope(history, archive, memories, start))
            .transpose()
    };
    // Resolve accepted identity before any new manifest inspection or snapshots.
    if let Some(settlement) =
        freshness.accepted_settlement(open_epoch.as_ref().map(|start| start.import_id.as_str()))
    {
        if open_epoch.is_some()
            && epoch_scope()?.unwrap().start_digest != settlement.start_evidence_digest
        {
            return Err(fail("import settlement start evidence mismatch"));
        }
        return historical_freeze::accepted_retry(settlement);
    }
    let scope = epoch_scope()?;
    let manifest = historical_manifest::build(
        views,
        scope,
        requested_complete,
        #[cfg(feature = "freshness-performance-trace")]
        traces,
    )?;
    let owner = history
        .owner_uuid()
        .ok_or_else(|| fail("historical import requires a durable REL owner"))?;
    let event_owner = history
        .owner_uuid()
        .ok_or_else(|| fail("missing historical event owner"))?;
    let owner_id = history
        .owner_id()
        .ok_or_else(|| fail("missing historical event owner identity"))?;
    let event_scope = epoch_scope()?;
    let outcome = super::historical_events::build(
        &super::historical_events::HistoricalEventReadAccess {
            owner_uuid: event_owner,
            owner_id: &owner_id,
            epoch_scope: event_scope,
            history,
            archive,
            memories,
            vectors,
            dream,
        },
        &manifest,
    );
    #[cfg(feature = "freshness-performance-trace")]
    traces.extend(outcome.traces);
    let events = outcome.result?;
    freshness.synchronize_source()?;
    cut.owner = owner;
    let mut frozen = historical_freeze::from_manifest(manifest, events, cut);
    let replay = if frozen.epoch {
        historical_freeze::bind_result_cohort(&mut frozen)?;
        let scope = epoch_scope()?
            .ok_or_else(|| fail("historical reconstruction requires an open import epoch"))?;
        historical_freeze::framed_scope(&frozen, scope)
    } else {
        let source_nodes = crate::history::source_proof::required_nodes(archive, frozen.cut())?
            .into_keys()
            .collect();
        historical_freeze::unframed_scope(&frozen, source_nodes)
    };
    historical_freeze::attach_semantic_evidence(&mut frozen, freshness, &replay)?;
    frozen.settlement.input_bytes = 0;
    Ok(frozen)
}
