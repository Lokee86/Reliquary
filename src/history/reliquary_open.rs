//! Ordered immutable REL reconstruction and interleaved History proofs.
use crate::imports::epoch_model::ImportEpochStatus;
use crate::owner_lifecycle::{OwnerError, ReliquaryReconstruction, ReliquaryReplay};

pub(crate) fn reconstruct_reliquary(
    replay: &mut ReliquaryReplay,
) -> Result<ReliquaryReconstruction, OwnerError> {
    let archive = replay.finish_archive()?;
    super::archive_retention::validate(
        &archive,
        &replay.history_source().map_err(|error| error.0)?,
        &replay.owner_maintenance,
    )?;
    let memories = replay.finish_memory()?;
    super::memory_retention::validate(&memories, &replay.owner_maintenance)?;
    let relations = replay.finish_relations(&memories, &archive)?;
    let graph = replay.finish_graph(&memories, &relations.entities)?;
    super::validate_source_history(
        &replay.owner_maintenance,
        &replay
            .history_source()
            .map_err(|error| OwnerError::Repack(error.to_string()))?,
        &graph,
    )
    .map_err(OwnerError::Repack)?;
    graph.validate_rewrite_baseline(
        replay.owner_uuid(),
        replay.rewrite_lineage.rewrite_genesis(),
    )?;
    let accepted = replay.finish_accepted(&graph, &archive, &memories)?;
    let due = replay.rebuild_due(&archive)?;
    let vectors = replay.finish_vectors(&memories, &archive)?;
    crate::owner_lifecycle::global_versions::validate_semantic_global_versions(
        &archive,
        &memories,
        &relations.entities,
        &relations.relationships,
        &graph,
        &vectors.generations,
    )?;
    let compactions = replay.finish_compactions()?;
    replay.import_epochs.validate_owner(replay.owner_uuid())?;
    crate::owner_lifecycle::validate_import_receipts(
        &replay.import_epochs,
        &archive.import_receipts,
    )?;
    super::reliquary_vector_retention::validate_packed_vector_retention(
        replay.history_source(),
        &vectors.packed,
        &vectors.memory,
        &vectors.archive,
        &vectors.profiles,
        &vectors.generations,
        &replay.owner_maintenance,
    )?;
    super::reliquary_description_retention::validate_description_retention(
        replay.history_source(),
        &relations.entities,
        &relations.relationships,
        &replay.owner_maintenance,
    )?;
    for settlement in replay.freshness.import_settlements().to_vec() {
        super::validate_historical_settlement(
            &replay.history_read_access(),
            &archive,
            &memories,
            &graph,
            &replay.freshness,
            &replay.dream_freshness,
            &replay.import_epochs,
            &settlement,
        )?;
    }
    if replay
        .import_epochs
        .statuses()
        .iter()
        .any(|status| matches!(status, ImportEpochStatus::Completed { .. }))
    {
        let source = replay
            .history_source()
            .map_err(|error| OwnerError::Container(error.0))?;
        super::completion_epochs(&source, &archive.import_receipts)?;
    }
    replay.entity_resolutions.validate_local()?;
    crate::owner_lifecycle::validate_entity_resolution_references(
        &memories,
        &relations.entities,
        &replay.entity_resolutions.records(),
    )?;
    Ok(ReliquaryReconstruction {
        archive,
        memories,
        graph,
        accepted,
        relations,
        vectors,
        conversation_compactions: compactions,
        freshness_due: due,
    })
}
