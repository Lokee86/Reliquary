use crate::history::freshness_reconciliation::FreshnessReplaySource;
use crate::maintenance::reconciliation_owner_access::{
    create_reconciliation_owner, has_derived_vector_state, import_entity_resolution, legacy_scope,
    open_reconciliation_container, open_reconciliation_owner,
};
use crate::maintenance::reconciliation_replay::{
    merge_interaction_streams, read_archive_tail, read_entity_tail, read_graph_tail,
    read_memory_tail, read_relationship_tail, replay_archive_tail, replay_entity_tail,
    replay_file_memory_links, replay_graph_tail, replay_graph_with_skips,
    replay_interaction_streams, replay_memory_tail, replay_relationship_tail, validate_graph_tails,
};
use crate::maintenance::rewrite::owner_access::ReliquaryRewriteAccess;
use crate::owner_lifecycle::ReliquaryOwner;
use crate::processing_epoch_model::{ProcessingEpochState, ProcessingLaneId};
use crate::processing_epoch_store::merge_processing_epoch_records;
use crate::reconciliation_values::{CvaComparison, CvaReconcileResult};
use crate::{CompatibilityProfile, CvaReconcileError, MemoryId};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub(crate) fn reconcile_diverged(
    left_path: &Path,
    right_path: &Path,
    output_path: &Path,
    comparison: CvaComparison,
) -> Result<(CvaReconcileResult, ReliquaryOwner), CvaReconcileError> {
    let mut left = open_reconciliation_owner(left_path)?;
    let mut right = open_reconciliation_owner(right_path)?;
    if !left.external_index.sources.is_empty()
        || left.external_index.damaged
        || !right.external_index.sources.is_empty()
        || right.external_index.damaged
        || !left.memories.external_windows.is_empty()
        || !right.memories.external_windows.is_empty()
    {
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "external source state in divergent reconciliation",
        ));
    }
    let legacy_scope_kind = legacy_scope(&left);
    let left_rel_metadata = left.rel_metadata();
    let right_rel_metadata = right.rel_metadata();
    if left_rel_metadata != right_rel_metadata {
        return Err(CvaReconcileError::UnsupportedSemanticOwner("REL metadata"));
    }
    if left.ego.version() > 0 || right.ego.version() > 0 {
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "Ego state in divergent reconciliation",
        ));
    }
    let owner_uuid = left
        .container
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
    let left_profiles = left
        .compatibility_profiles
        .profiles()
        .into_iter()
        .map(|profile| {
            let retired = left.compatibility_profiles.is_retired(profile.id);
            (profile, retired)
        })
        .collect();
    let right_profiles = right
        .compatibility_profiles
        .profiles()
        .into_iter()
        .map(|profile| {
            let retired = right.compatibility_profiles.is_retired(profile.id);
            (profile, retired)
        })
        .collect();
    let left_echo = left.echo.all_records();
    let right_echo = right.echo.all_records();
    let left_project_history = left.project_history.records();
    let right_project_history = right.project_history.records();
    let left_processing_epochs = left.processing_epochs.records();
    let processing_epochs = merge_processing_epoch_records(
        left_processing_epochs
            .iter()
            .copied()
            .chain(right.processing_epochs.records()),
    )?;
    let processing_epoch_change_required = processing_epochs != left_processing_epochs;
    let left_dream_pairs = left.dream_pairs.records();
    let dream_pairs = merge_dream_pairs(left_dream_pairs.clone(), right.dream_pairs.records());
    let left_entity_resolutions = left.entity_resolutions.records();
    let right_entity_resolutions = right.entity_resolutions.records();
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
        left.interaction_streams.all_records(),
        right.interaction_streams.all_records(),
    )?;

    let left_freshness = freshness_source(&left);
    let right_freshness = freshness_source(&right);
    crate::maintenance::freshness_reconciliation::validate_freshness(
        &left_freshness,
        &right_freshness,
    )?;
    let freshness_change_required =
        crate::maintenance::freshness_reconciliation::right_changes_freshness(
            &left_freshness,
            &right_freshness,
        )?;

    if left.graph.rewrite_baseline_memories() != right.graph.rewrite_baseline_memories() {
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "conflicting rewrite Graph baseline",
        ));
    }
    let mut created_output = None;
    let merge_result = (|| {
        let mut output = create_reconciliation_owner(output_path, owner_uuid, legacy_scope_kind)?;
        created_output = Some(output.container.read_source()?);
        if let Some(genesis) = left.rewrite_lineage.rewrite_genesis().cloned() {
            let payload = genesis.encode();
            output.container.append(&payload)?;
            output
                .rewrite_lineage
                .ingest(&payload, owner_uuid)
                .map_err(crate::owner_lifecycle::OwnerError::Repack)?;
        }
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
            import_entity_resolution(&mut output, resolution)?;
        }
        if let Some(memories) = left.graph.rewrite_baseline_memories() {
            let genesis = left.rewrite_lineage.rewrite_genesis().ok_or(
                CvaReconcileError::UnsupportedSemanticOwner("Graph baseline lacks lineage"),
            )?;
            output.graph.publish_rewrite_baseline(
                &mut output.container,
                owner_uuid,
                genesis.generation,
                genesis.policy_digest,
                memories,
            )?;
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
        let graph_skips = validate_graph_tails(&left_graph_divergent, &right_graph_divergent)?;
        let graph_result =
            replay_graph_with_skips(&mut output, &right_graph_divergent, &graph_skips, false)?;
        let file_memory_links =
            replay_file_memory_links(&mut output, &right_archive.file_memory_links)?;
        replay_echo(&mut output, right_echo)?;
        let semantic_change_required = output.container.chunks()?.len() > before_right
            || right_stream_change
            || right_project_history_change;
        let canonical_change_required = semantic_change_required || freshness_change_required;
        let vector_rebuild_required = semantic_change_required && derived_vectors_present;
        let output_owner_uuid = output.container.owner_uuid();
        crate::maintenance::freshness_reconciliation::replay_freshness(
            &left_freshness,
            &right_freshness,
            crate::maintenance::freshness_reconciliation::FreshnessReplayDestination {
                container: &mut output.container,
                archive: &output.archive,
                memories: &output.memories,
                owner_uuid: output_owner_uuid,
                freshness: &mut output.freshness,
                dream_freshness: &mut output.dream_freshness,
            },
        )?;
        replay_processing_epochs(&mut output, processing_epochs)?;
        replay_dream_pairs(&mut output, dream_pairs)?;
        if canonical_change_required
            && let Some((project_revision, management)) = latest_project_revision.clone()
        {
            let rel_cut = crate::RelSemanticCut {
                global_version: output.container.latest_version(),
                archive_version: output.archive.archive_version(),
                memory_version: output.memories.memory_version(),
            };
            output
                .project_history
                .put(&mut output.container, rel_cut, project_revision, management)
                .map_err(crate::owner_lifecycle::OwnerError::from)?;
        }

        output.container.sync()?;
        let expected_archive = output.archive;
        let output_container = output.container;
        let reopened = open_reconciliation_container(output_container)?;
        if !expected_archive.activity_history_matches(&reopened.archive)
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
            reopened,
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
        validated_output,
    ) = match merge_result {
        Ok(value) => value,
        Err(error) => {
            if created_output
                .as_ref()
                .is_some_and(|source| source.validate_path_identity().is_ok())
            {
                let _ = fs::remove_file(output_path);
            }
            return Err(error);
        }
    };

    let validated_output = if !canonical_change_required
        && !processing_epoch_change_required
        && !dream_pair_change_required
    {
        replace_with_left(left_path, output_path, validated_output)?
    } else {
        validated_output
    };

    let result = CvaReconcileResult {
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
    };
    Ok((result, validated_output))
}

fn replay_processing_epochs(
    output: &mut ReliquaryOwner,
    records: Vec<(MemoryId, ProcessingLaneId, ProcessingEpochState)>,
) -> Result<(), CvaReconcileError> {
    for (memory_id, lane_id, state) in records {
        output
            .processing_epochs
            .put(&mut output.container, memory_id, lane_id, state)?;
    }
    Ok(())
}

fn freshness_source(owner: &ReliquaryOwner) -> FreshnessReplaySource<'_> {
    FreshnessReplaySource {
        owner_uuid: owner.container.owner_uuid(),
        archive: &owner.archive,
        freshness: &owner.freshness,
        dream_freshness: &owner.dream_freshness,
        graph: &owner.graph,
        communities: &owner.communities,
    }
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
    output: &mut ReliquaryOwner,
    records: Vec<(crate::MemoryId, crate::MemoryId)>,
) -> Result<(), CvaReconcileError> {
    for (left, right) in records {
        if !output.memories.contains_memory(left) || !output.memories.contains_memory(right) {
            return Err(crate::MemoryError::MissingMemory.into());
        }
        output.dream_pairs.put(&mut output.container, left, right)?;
    }
    Ok(())
}

fn replay_echo(
    output: &mut ReliquaryOwner,
    events: Vec<crate::EchoEvent>,
) -> Result<(), CvaReconcileError> {
    for event in events {
        output
            .echo
            .put(&mut output.container, event)
            .map_err(crate::owner_lifecycle::OwnerError::from)?;
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
    output: &mut ReliquaryOwner,
    profiles: Vec<(CompatibilityProfile, bool)>,
) -> Result<(), CvaReconcileError> {
    for (profile, retired) in profiles {
        let id = profile.id;
        output
            .compatibility_profiles
            .put(&mut output.container, profile)?;
        if retired {
            output
                .compatibility_profiles
                .retire(&mut output.container, id)?;
        }
    }
    Ok(())
}

fn replace_with_left(
    left_path: &Path,
    output_path: &Path,
    held_output: ReliquaryOwner,
) -> Result<ReliquaryOwner, CvaReconcileError> {
    let source = crate::Container::open_read_only(left_path)?;
    let staging =
        output_path.with_file_name(format!(".reconcile-noop-{}.tmp", uuid::Uuid::new_v4()));
    let mut candidate = source.create_empty_like(&staging)?;
    let candidate_identity = candidate
        .backing_file_identity()?
        .ok_or(crate::ContainerError::LockingUnsupported)?;
    let result = (|| -> Result<(), CvaReconcileError> {
        let mut frames = source.physical_cursor()?;
        while let Some(frame) = frames.next_frame()? {
            candidate.append(&frame.payload)?;
        }
        drop(frames);
        candidate.sync()?;
        source.validate_path_identity()?;
        held_output.container.validate_path_identity()?;
        candidate.validate_path_identity()?;
        crate::atomic_publication::atomic_replace(&staging, output_path)?;
        candidate.relocate_published_path(output_path)?;
        candidate.sync()?;
        crate::atomic_publication::sync_parent(
            output_path.parent().unwrap_or_else(|| Path::new(".")),
        )?;
        Ok(())
    })();
    if result.is_err() && candidate.validate_path_identity().is_ok() && staging.exists() {
        let _ = fs::remove_file(&staging);
    }
    result?;
    match open_reconciliation_container(candidate) {
        Ok(output) => Ok(output),
        Err(error) => {
            remove_if_identity(output_path, candidate_identity);
            Err(error.into())
        }
    }
}

fn remove_if_identity(path: &Path, expected: (u64, u64)) {
    let Ok(container) = crate::Container::open_read_only(path) else {
        return;
    };
    let same = container.backing_file_identity().ok().flatten() == Some(expected);
    drop(container);
    if same {
        let _ = fs::remove_file(path);
    }
}
