//! Replay finite source-only intervals into an unchanged left physical prefix.
use super::reconciliation::{CompletedEpoch, unsupported};
use crate::cva_reconcile::{CvaComparison, CvaReconcileResult};
use crate::{Cva, CvaReconcileError};
use std::{collections::BTreeMap, path::Path};

pub(super) fn replay(
    left: &mut Cva,
    right: &mut Cva,
    left_path: &Path,
    output_path: &Path,
    comparison: CvaComparison,
    known: BTreeMap<String, CompletedEpoch>,
    epochs: BTreeMap<String, CompletedEpoch>,
    boundary: usize,
) -> Result<CvaReconcileResult, CvaReconcileError> {
    crate::cva_reconcile::copy_and_validate(left_path, output_path)?;
    let mut output = Cva::open(output_path)?;
    let mut cursor = boundary;
    let mut coordinates = super::reconciliation_interval::Coordinates::shared(right, boundary)?;
    let before_archive = output.archive_version();
    let mut completions = 0;
    let mut changed = false;
    for (id, epoch) in &epochs {
        if let Some(retained) = known.get(id) {
            coordinates.retained_epoch(right, &mut output, epoch, retained)?;
        }
    }
    let mut ordered: Vec<_> = epochs.into_values().collect();
    ordered.sort_by_key(|epoch| epoch.first);
    for epoch in ordered {
        if epoch.end <= cursor {
            continue;
        }
        if epoch.first < cursor {
            if known.contains_key(&epoch.start.import_id) {
                cursor = epoch.end;
                continue;
            }
            return Err(unsupported("partial import replay interval"));
        }
        completions += replay_interval(right, &mut output, cursor, epoch.first, &mut coordinates)?;
        if known.contains_key(&epoch.start.import_id) {
            cursor = epoch.end;
            continue;
        }
        let mut start = epoch.start;
        start.pre = output.import_watermarks();
        output
            .import_epochs
            .append_start(&mut output.container, start.clone())
            .map_err(crate::CvaError::Import)?;
        completions += output.with_import_write_scope(|output| {
            replay_interval(
                right,
                output,
                epoch.first + 1,
                epoch.end - 1,
                &mut coordinates,
            )
        })?;
        // Native reconstruction freezes the destination input and derives its
        // start binding, byte-prefix digest and canonical pre-start base. Source
        // coordinates or source replacement bytes never authorize publication.
        output
            .reconstruct_historical_freshness(Default::default())
            .map_err(|error| crate::CvaError::Freshness(error.to_string()))?;
        match output.complete_native_import(&start.import_id, epoch.complete.completed_at_ns)? {
            super::ImportCompletionOutcome::Completed { .. } => {}
            super::ImportCompletionOutcome::Pending(_) => {
                return Err(unsupported("replayed import is not ready for completion"));
            }
        }
        changed = true;
        cursor = epoch.end;
    }
    completions += replay_interval(right, &mut output, cursor, usize::MAX, &mut coordinates)?;
    changed |= output.archive_version() != before_archive
        || completions != 0
        || coordinates.memory_revisions != 0
        || coordinates.entity_revisions != 0
        || coordinates.relationship_revisions != 0
        || coordinates.graph_transactions != 0;
    output.sync()?;
    let reopened = Cva::open(output_path)?;
    if !output.archive.activity_history_matches(&reopened.archive)
        || !reopened
            .archive
            .activity_merge_order_matches(&left.archive, &right.archive)
    {
        return Err(unsupported("reconciled import activity order changed"));
    }
    for id in known.keys() {
        if output.import_epoch_status(id) != left.import_epoch_status(id) {
            return Err(unsupported(
                "reconciliation changed a retained left import epoch",
            ));
        }
    }
    Ok(CvaReconcileResult {
        comparison,
        replayed_archive_records: output.archive_version().saturating_sub(before_archive) as usize,
        replayed_memory_revisions: coordinates.memory_revisions,
        duplicate_memory_revisions: coordinates.duplicate_memory_revisions,
        replayed_entity_revisions: coordinates.entity_revisions,
        duplicate_entity_revisions: coordinates.duplicate_entity_revisions,
        replayed_relationship_revisions: coordinates.relationship_revisions,
        duplicate_relationship_revisions: coordinates.duplicate_relationship_revisions,
        replayed_insomnia_completions: completions,
        replayed_graph_transactions: coordinates.graph_transactions,
        replayed_graph_mutations: coordinates.graph_mutations,
        duplicate_graph_mutations: coordinates.duplicate_graph_mutations,
        replayed_file_memory_links: coordinates.file_memory_links,
        canonical_change_required: changed,
        vector_rebuild_required: changed
            && (left.archive_vector_stats().objects > 0
                || left.packed_vector_stats().objects > 0
                || left.memory_vector_stats().objects > 0),
    })
}

fn replay_interval(
    source: &mut Cva,
    output: &mut Cva,
    first: usize,
    end: usize,
    coordinates: &mut super::reconciliation_interval::Coordinates,
) -> Result<usize, CvaReconcileError> {
    let chunks = source.container.chunks()?;
    let mut completions = 0;
    for (index, chunk) in chunks.into_iter().enumerate().take(end).skip(first) {
        let payload = source.container.read(chunk)?;
        let known_episodes: std::collections::HashSet<_> = output
            .archive
            .episodes
            .iter()
            .map(|episode| episode.id)
            .collect();
        completions += super::reconciliation_interval::replay_frame(
            source,
            output,
            index,
            &payload,
            coordinates,
        )?;
        // Admit newly accepted Episodes at their physical position. Later
        // Archive activity must not move a grouped publication's birth cut.
        let episodes: Vec<_> = output
            .archive
            .episodes
            .iter()
            .filter(|episode| !known_episodes.contains(&episode.id))
            .cloned()
            .collect();
        for episode in episodes {
            if output.insomnia_work(episode.id).is_none() {
                output.queue_insomnia_episode(
                    episode.id,
                    if episode.origin == crate::EpisodeOrigin::Import {
                        crate::InsomniaPriority::Import
                    } else {
                        crate::InsomniaPriority::Live
                    },
                    episode.finalized_at_ns,
                )?;
            }
        }
    }
    Ok(completions)
}
