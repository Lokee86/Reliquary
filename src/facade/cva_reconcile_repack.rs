use crate::cva_reconcile::{CvaComparison, CvaReconcileResult};
use crate::cva_reconcile_archive::{
    read_archive_tail, replay_archive_tail, replay_file_memory_links,
};
use crate::cva_reconcile_entity::{read_entity_tail, replay_entity_tail};
use crate::cva_reconcile_graph::{read_graph_tail, reconcile_right_graph_tail, replay_graph_tail};
use crate::cva_reconcile_interaction::{merge_interaction_streams, replay_interaction_streams};
use crate::cva_reconcile_memory::{read_memory_tail, replay_memory_tail};
use crate::cva_reconcile_relationship::{read_relationship_tail, replay_relationship_tail};
use crate::processing_epoch_model::{ProcessingEpochState, ProcessingLaneId};
use crate::processing_epoch_store::merge_processing_epoch_records;
use crate::{CompatibilityProfile, Cva, CvaReconcileError, MemoryId};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub(crate) fn reconcile_diverged(
    left_path: &Path,
    right_path: &Path,
    output_path: &Path,
    comparison: CvaComparison,
) -> Result<CvaReconcileResult, CvaReconcileError> {
    let mut left = Cva::open(left_path)?;
    let mut right = Cva::open(right_path)?;
    let legacy_scope = left.legacy_scope_kind();
    let left_rel_metadata = left.rel_metadata();
    let right_rel_metadata = right.rel_metadata();
    if left_rel_metadata != right_rel_metadata {
        return Err(CvaReconcileError::UnsupportedSemanticOwner("REL metadata"));
    }
    if left.ego_version() > 0 || right.ego_version() > 0 {
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "Ego state in divergent reconciliation",
        ));
    }
    let owner_uuid = left
        .owner_uuid()
        .ok_or(CvaReconcileError::MissingOwnerId("left"))?;
    let derived_vectors_present =
        has_derived_vector_state(&left) || has_derived_vector_state(&right);

    let left_archive = read_archive_tail(&mut left, 0)?;
    let right_archive = read_archive_tail(&mut right, comparison.common_chunk_count)?;
    let left_memory = read_memory_tail(&mut left, 0)?;
    let right_memory = read_memory_tail(&mut right, comparison.common_chunk_count)?;
    let left_entities = read_entity_tail(&mut left, 0)?;
    let right_entities = read_entity_tail(&mut right, comparison.common_chunk_count)?;
    let left_relationships = read_relationship_tail(&mut left, 0)?;
    let right_relationships = read_relationship_tail(&mut right, comparison.common_chunk_count)?;
    let left_graph = read_graph_tail(&mut left, 0)?;
    let left_graph_divergent = read_graph_tail(&mut left, comparison.common_chunk_count)?;
    let right_graph_divergent = read_graph_tail(&mut right, comparison.common_chunk_count)?;
    let left_profiles = left.compatibility_profiles();
    let right_profiles = right.compatibility_profiles();
    let left_echo = left.echo_records();
    let right_echo = right.echo_records();
    let left_project_history = left.project_revision_correlations();
    let right_project_history = right.project_revision_correlations();
    let left_processing_epochs = left.processing_epochs.records();
    let processing_epochs = merge_processing_epoch_records(
        left_processing_epochs
            .iter()
            .copied()
            .chain(right.processing_epochs.records()),
    )?;
    let processing_epoch_change_required = processing_epochs != left_processing_epochs;
    let left_dream_pairs = left.dream_pair_records();
    let dream_pairs = merge_dream_pairs(left_dream_pairs.clone(), right.dream_pair_records());
    let left_entity_resolutions = left.entity_resolution_records();
    let right_entity_resolutions = right.entity_resolution_records();
    if left_entity_resolutions != right_entity_resolutions {
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "divergent Entity resolution state",
        ));
    }
    let dream_pair_change_required = dream_pairs != left_dream_pairs;
    let latest_project_revision =
        compatible_project_revision(&left_project_history, &right_project_history).ok_or(
            CvaReconcileError::UnsupportedSemanticOwner(
                "divergent project-revision correlation history",
            ),
        )?;
    let right_project_history_change = right_project_history.len() > left_project_history.len();
    let (interaction_streams, right_stream_change) = merge_interaction_streams(
        left.interaction_stream_records(),
        right.interaction_stream_records(),
    )?;

    crate::cva_reconcile_freshness::validate_freshness(&left, &right)?;
    let freshness_change_required =
        crate::cva_reconcile_freshness::right_changes_freshness(&left, &right)?;

    let merge_result = (|| {
        let mut output = match legacy_scope {
            Some(scope) => Cva::create_legacy_scope_with_uuid(output_path, scope, owner_uuid)?,
            None => Cva::create_rel_with_uuid(output_path, owner_uuid, None)?,
        };
        output.set_rel_metadata(
            left_rel_metadata.type_label.clone(),
            left_rel_metadata.dependencies.clone(),
        )?;
        replay_profiles(&mut output, left_profiles)?;
        replay_interaction_streams(&mut output, interaction_streams)?;
        replay_archive_tail(&mut output, &left_archive)?;
        replay_memory_tail(&mut output, left_memory)?;
        replay_entity_tail(&mut output, left_entities)?;
        replay_relationship_tail(&mut output, left_relationships)?;
        for resolution in left_entity_resolutions.clone() {
            output.import_entity_resolution(resolution)?;
        }
        replay_graph_tail(&mut output, &left_graph)?;
        replay_file_memory_links(&mut output, &left_archive.file_memory_links)?;
        replay_echo(&mut output, left_echo)?;

        let before_right = output.container.chunks()?.len();
        replay_profiles(&mut output, right_profiles)?;
        let archive_records = replay_archive_tail(&mut output, &right_archive)?;
        let memory_result = replay_memory_tail(&mut output, right_memory)?;
        let entity_result = replay_entity_tail(&mut output, right_entities)?;
        let relationship_result = replay_relationship_tail(&mut output, right_relationships)?;
        let graph_result =
            reconcile_right_graph_tail(&mut output, &left_graph_divergent, &right_graph_divergent)?;
        let file_memory_links =
            replay_file_memory_links(&mut output, &right_archive.file_memory_links)?;
        replay_echo(&mut output, right_echo)?;
        let semantic_change_required = output.container.chunks()?.len() > before_right
            || right_stream_change
            || right_project_history_change;
        let canonical_change_required = semantic_change_required || freshness_change_required;
        let vector_rebuild_required = semantic_change_required && derived_vectors_present;
        crate::cva_reconcile_freshness::replay_freshness(&left, &right, &mut output)?;
        replay_processing_epochs(&mut output, processing_epochs)?;
        replay_dream_pairs(&mut output, dream_pairs)?;
        if canonical_change_required
            && let Some((project_revision, management)) = latest_project_revision.clone()
        {
            output.correlate_project_revision_with_management(project_revision, management)?;
        }

        output.sync()?;
        let reopened = Cva::open(output_path)?;
        if !output.archive.activity_history_matches(&reopened.archive)
            || !reopened
                .archive
                .activity_merge_order_matches(&left.archive, &right.archive)
        {
            return Err(CvaReconcileError::Archive(
                crate::ArchiveError::CorruptRecord(
                    "reconciled REL activity order or recovered history changed",
                ),
            ));
        }
        drop(reopened);
        drop(output);
        Ok((
            archive_records,
            memory_result,
            entity_result,
            relationship_result,
            graph_result,
            file_memory_links,
            canonical_change_required,
            vector_rebuild_required,
            processing_epoch_change_required,
            dream_pair_change_required,
        ))
    })();

    let (
        archive_records,
        memory_result,
        entity_result,
        relationship_result,
        graph_result,
        file_memory_links,
        canonical_change_required,
        vector_rebuild_required,
        processing_epoch_change_required,
        dream_pair_change_required,
    ) = match merge_result {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_file(output_path);
            return Err(error);
        }
    };

    if !canonical_change_required
        && !processing_epoch_change_required
        && !dream_pair_change_required
    {
        replace_with_left(left_path, output_path)?;
    }

    Ok(CvaReconcileResult {
        comparison,
        replayed_archive_records: archive_records,
        replayed_memory_revisions: memory_result.revisions,
        duplicate_memory_revisions: memory_result.duplicate_revisions,
        replayed_entity_revisions: entity_result.revisions,
        duplicate_entity_revisions: entity_result.duplicate_revisions,
        replayed_relationship_revisions: relationship_result.revisions,
        duplicate_relationship_revisions: relationship_result.duplicate_revisions,
        replayed_insomnia_completions: memory_result.completions,
        replayed_graph_transactions: graph_result.transactions,
        replayed_graph_mutations: graph_result.mutations,
        duplicate_graph_mutations: graph_result.duplicate_mutations,
        replayed_file_memory_links: file_memory_links,
        canonical_change_required,
        vector_rebuild_required,
    })
}

fn replay_processing_epochs(
    output: &mut Cva,
    records: Vec<(MemoryId, ProcessingLaneId, ProcessingEpochState)>,
) -> Result<(), CvaReconcileError> {
    for (memory_id, lane_id, state) in records {
        output
            .processing_epochs
            .put(&mut output.container, memory_id, lane_id, state)?;
    }
    Ok(())
}

fn merge_dream_pairs(
    left: Vec<(crate::MemoryId, crate::MemoryId)>,
    right: Vec<(crate::MemoryId, crate::MemoryId)>,
) -> Vec<(crate::MemoryId, crate::MemoryId)> {
    let mut merged = BTreeMap::new();
    for (left, right) in left.into_iter().chain(right) {
        let pair = if left.0 <= right.0 {
            (left, right)
        } else {
            (right, left)
        };
        merged.insert((pair.0.0, pair.1.0), pair);
    }
    merged.into_values().collect()
}

fn replay_dream_pairs(
    output: &mut Cva,
    records: Vec<(crate::MemoryId, crate::MemoryId)>,
) -> Result<(), CvaReconcileError> {
    for (left, right) in records {
        output.mark_dream_pair_evaluated(left, right)?;
    }
    Ok(())
}

fn replay_echo(output: &mut Cva, events: Vec<crate::EchoEvent>) -> Result<(), CvaReconcileError> {
    for event in events {
        output.put_echo_event(event)?;
    }
    Ok(())
}

fn compatible_project_revision(
    left: &[crate::ProjectRevisionCorrelation],
    right: &[crate::ProjectRevisionCorrelation],
) -> Option<
    Option<(
        crate::ProjectRevisionRef,
        crate::ProjectRepositoryManagement,
    )>,
> {
    let common = left
        .iter()
        .zip(right)
        .take_while(|(left, right)| {
            left.project_revision == right.project_revision
                && left.repository_management == right.repository_management
        })
        .count();
    if common != left.len().min(right.len()) {
        return None;
    }
    let history = if right.len() > left.len() {
        right
    } else {
        left
    };
    Some(history.last().map(|record| {
        (
            record.project_revision.clone(),
            record.repository_management,
        )
    }))
}

fn replay_profiles(
    output: &mut Cva,
    profiles: Vec<CompatibilityProfile>,
) -> Result<(), CvaReconcileError> {
    for profile in profiles {
        output
            .compatibility_profiles
            .put(&mut output.container, profile)?;
    }
    Ok(())
}

fn replace_with_left(left_path: &Path, output_path: &Path) -> Result<(), CvaReconcileError> {
    fs::remove_file(output_path)?;
    crate::cva_reconcile::copy_and_validate(left_path, output_path)
}

fn has_derived_vector_state(cva: &Cva) -> bool {
    cva.packed_vector_stats().objects > 0
        || cva.memory_vector_stats().objects > 0
        || cva.archive_vector_stats().objects > 0
        || cva.vector_generation_stats().generations > 0
}
