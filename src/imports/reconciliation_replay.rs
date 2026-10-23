//! Replay finite source-only intervals into an unchanged left physical prefix.
use super::reconciliation::{CompletedEpoch, unsupported};
use super::reconciliation_copy::copy_import_source;
use super::reconciliation_epoch_replay::EpochReplay;
use crate::CvaReconcileError;
use crate::owner_lifecycle::ReliquaryOwner;
use crate::reconciliation_values::{CvaComparison, CvaReconcileResult};
use std::{collections::BTreeMap, path::Path};

fn import_watermarks(owner: &ReliquaryOwner) -> crate::imports::ImportWatermarks {
    crate::imports::ImportWatermarks {
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

pub(super) fn replay(
    left: &mut ReliquaryOwner,
    right: &mut ReliquaryOwner,
    left_path: &Path,
    output_path: &Path,
    comparison: CvaComparison,
    known: BTreeMap<String, CompletedEpoch>,
    epochs: BTreeMap<String, CompletedEpoch>,
    boundary: usize,
) -> Result<
    (
        CvaReconcileResult,
        crate::owner_lifecycle::ReconstructedReliquary,
    ),
    CvaReconcileError,
> {
    let mut output = copy_import_source(left_path, output_path)?;
    let created_output =
        crate::owner_lifecycle::import_epoch_transport::hold_created_output_identity(
            &output.container,
        )?;
    let replay_result = (|| {
        let mut cursor = boundary;
        let mut coordinates = super::reconciliation_interval::Coordinates::shared(right, boundary)?;
        let before_archive = output.archive.archive_version();
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
            completions +=
                replay_interval(right, &mut output, cursor, epoch.first, &mut coordinates)?;
            if known.contains_key(&epoch.start.import_id) {
                cursor = epoch.end;
                continue;
            }
            let mut start = epoch.start;
            start.pre = import_watermarks(&output);
            crate::owner_lifecycle::import_epoch_transport::append_import_start(
                &mut output.container,
                &mut output.import_epochs,
                start.clone(),
            )
            .map_err(crate::CvaError::Import)?;
            completions += EpochReplay::new(&mut output).replay(
                right,
                epoch.first + 1,
                epoch.end - 1,
                &mut coordinates,
            )?;
            // Native reconstruction freezes the destination input and derives its
            // start binding, byte-prefix digest and canonical pre-start base. Source
            // coordinates or source replacement bytes never authorize publication.
            crate::imports::owner_historical::reconstruct(&mut output)
                .map_err(|error| crate::CvaError::Freshness(error.to_string()))?;
            match crate::imports::owner_completion::complete(
                &mut output,
                &start.import_id,
                epoch.complete.completed_at_ns,
            )? {
                super::ImportCompletionOutcome::Completed { .. } => {}
                super::ImportCompletionOutcome::Pending(_) => {
                    return Err(unsupported("replayed import is not ready for completion"));
                }
            }
            changed = true;
            cursor = epoch.end;
        }
        completions += replay_interval(right, &mut output, cursor, usize::MAX, &mut coordinates)?;
        changed |= output.archive.archive_version() != before_archive
            || completions != 0
            || coordinates.memory_revisions != 0
            || coordinates.entity_revisions != 0
            || coordinates.relationship_revisions != 0
            || coordinates.graph_transactions != 0;
        crate::owner_lifecycle::import_epoch_transport::sync(&output.container)?;
        let expected_archive = output.archive;
        let (reopened, due) = super::reconciliation_copy::finish_replay_with_due(
            crate::owner_lifecycle::ReliquaryReplay::read(output.container)?,
        )?;
        if !expected_archive.activity_history_matches(&reopened.archive)
            || !reopened
                .archive
                .activity_merge_order_matches(&left.archive, &right.archive)
        {
            return Err(unsupported("reconciled import activity order changed"));
        }
        for id in known.keys() {
            if reopened.import_epochs.status(id) != left.import_epochs.status(id) {
                return Err(unsupported(
                    "reconciliation changed a retained left import epoch",
                ));
            }
        }
        let result = CvaReconcileResult {
            comparison,
            replayed_archive_records: reopened
                .archive
                .archive_version()
                .saturating_sub(before_archive) as usize,
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
                && (left.archive_vectors.stats().objects > 0
                    || left.packed_vectors.stats().objects > 0
                    || left.memory_vectors.stats().objects > 0),
        };
        Ok((result, (reopened, due)))
    })();
    if replay_result.is_err() {
        crate::owner_lifecycle::import_epoch_transport::cleanup_created_output_after_failure(
            created_output,
            output_path,
        );
    }
    replay_result
}

pub(super) fn replay_interval(
    source: &mut ReliquaryOwner,
    output: &mut ReliquaryOwner,
    first: usize,
    end: usize,
    coordinates: &mut super::reconciliation_interval::Coordinates,
) -> Result<usize, CvaReconcileError> {
    let chunks = super::reconciliation_interval::physical_objects(source)?;
    let mut completions = 0;
    for (index, chunk) in chunks.into_iter().enumerate().take(end).skip(first) {
        let payload = crate::owner_lifecycle::HistoryReadAccess::new(&source.container)
            .snapshot()
            .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?
            .read_object(chunk)
            .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?;
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
            if output.insomnia.work(episode.id).is_none() {
                let source = crate::owner_lifecycle::insomnia_replay::episode_source(
                    &episode,
                    if episode.origin == crate::EpisodeOrigin::Import {
                        crate::InsomniaPriority::Import
                    } else {
                        crate::InsomniaPriority::Live
                    },
                );
                output
                    .insomnia
                    .queue(&mut output.container, &source, episode.finalized_at_ns)?;
            }
        }
    }
    Ok(completions)
}
