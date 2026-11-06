//! Destination-native replay of completed import epochs.

use super::*;
use crate::records::{ImportComplete, ImportStart, ImportWatermarks};
use reliquary_archive::{ArchiveHistorySnapshot, EpisodeBoundary, EpisodeId, EpisodeOrigin};
use reliquary_freshness_store::FreshnessMemoryId;
use reliquary_memory_core::{decode_grouped_creation, encode_grouped_creation};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct ImportReplay {
    pub counts: ReplayCounts,
    pub rebased_freshness: BTreeSet<FreshnessMemoryId>,
}

#[derive(Clone)]
struct CompletedEpoch {
    start: ImportStart,
    complete: ImportComplete,
}

pub(super) fn validate(
    left: &RelSession,
    right: &RelSession,
) -> Result<(), RelReconciliationError> {
    let left_epochs = completed(left)?;
    let right_epochs = completed(right)?;
    for (id, right_epoch) in &right_epochs {
        if let Some(left_epoch) = left_epochs.get(id)
            && !same_authority(left_epoch, right_epoch, left, right)?
        {
            return Err(conflict(format!("conflicting completed import epoch {id}")));
        }
    }
    Ok(())
}

pub(super) fn replay(
    left: &RelSession,
    right: &RelSession,
    target_path: &Path,
) -> Result<ImportReplay, RelReconciliationError> {
    validate(left, right)?;
    let known = completed(left)?;
    let mut epochs = completed(right)?.into_values().collect::<Vec<_>>();
    epochs.sort_by(|left, right| {
        left.start
            .pre
            .archive_version
            .cmp(&right.start.pre.archive_version)
            .then_with(|| left.start.started_at_ns.cmp(&right.start.started_at_ns))
            .then_with(|| left.start.import_id.cmp(&right.start.import_id))
    });
    if epochs
        .iter()
        .all(|epoch| known.contains_key(&epoch.start.import_id))
    {
        return Ok(ImportReplay {
            counts: ReplayCounts::default(),
            rebased_freshness: BTreeSet::new(),
        });
    }

    let mut writer = RelWriter::open_path(target_path, merged_graph_admission(left, right), [], [])
        .map_err(verification)?;
    let mut counts = ReplayCounts::default();
    let mut rebased_freshness = BTreeSet::new();
    let mut source_archive_cursor = 0;
    for epoch in epochs {
        let pre =
            epoch
                .start
                .pre
                .archive_version
                .ok_or(RelReconciliationError::UnsupportedState(
                    "completed import lacks an Archive pre-watermark",
                ))?;
        let post =
            epoch
                .complete
                .post
                .archive_version
                .ok_or(RelReconciliationError::UnsupportedState(
                    "completed import lacks an Archive post-watermark",
                ))?;
        if pre < source_archive_cursor || post < pre {
            return Err(RelReconciliationError::UnsupportedState(
                "completed import Archive watermarks are not ordered",
            ));
        }
        replay_archive_range(
            &mut writer,
            &right.archive,
            source_archive_cursor,
            pre,
            None,
            false,
            &mut counts,
        )?;
        if known.contains_key(&epoch.start.import_id) {
            source_archive_cursor = post;
            continue;
        }
        let status = writer
            .start_native_import(
                &epoch.start.import_key,
                &epoch.start.source_namespace,
                &epoch.start.source_format,
                epoch.start.started_at_ns,
            )
            .map_err(verification)?;
        let ImportEpochStatus::Open(destination_start) = status else {
            return Err(conflict(
                "destination import identity was unexpectedly completed",
            ));
        };
        let episodes = replay_archive_range(
            &mut writer,
            &right.archive,
            pre,
            post,
            Some(&destination_start.import_id),
            true,
            &mut counts,
        )?;
        replay_grouped_completions(
            &mut writer,
            right,
            &destination_start.import_id,
            &episodes,
            &mut counts,
            &mut rebased_freshness,
        )?;
        writer
            .complete_native_import(&destination_start.import_id, epoch.complete.completed_at_ns)
            .map_err(verification)?;
        source_archive_cursor = post;
    }
    replay_archive_range(
        &mut writer,
        &right.archive,
        source_archive_cursor,
        right.archive.archive_version(),
        None,
        false,
        &mut counts,
    )?;
    drop(writer);
    counts.semantic_change = counts.archive
        + counts.memory
        + counts.entities
        + counts.relationships
        + counts.graph
        + counts.freshness
        > 0;
    Ok(ImportReplay {
        counts,
        rebased_freshness,
    })
}

pub(super) fn rebased_freshness(
    left: &RelSession,
    right: &RelSession,
) -> Result<BTreeSet<FreshnessMemoryId>, RelReconciliationError> {
    let known = completed(left)?;
    let pending = completed(right)?;
    let mut ids = BTreeSet::new();
    for epoch in pending.values() {
        if known.contains_key(&epoch.start.import_id) {
            continue;
        }
        for episode in imported_episodes(&right.archive, epoch)? {
            for completion in right
                .grouped_completions
                .values()
                .filter(|completion| completion.episode_id == episode.0)
            {
                if completion.freshness_birth_turn.is_some() {
                    ids.extend(
                        completion
                            .freshness_birth_memory_ids
                            .iter()
                            .copied()
                            .map(FreshnessMemoryId),
                    );
                }
            }
        }
    }
    Ok(ids)
}

pub(super) fn verify(
    left: &RelSession,
    right: &RelSession,
    output: &RelSession,
) -> Result<(), RelReconciliationError> {
    let output_epochs = completed(output)?;
    for epoch in completed(left)?.values() {
        let retained = output_epochs
            .get(&epoch.start.import_id)
            .ok_or_else(|| conflict("reconciliation lost a retained import epoch"))?;
        if retained.start != epoch.start || retained.complete != epoch.complete {
            return Err(conflict(
                "reconciliation changed a retained left import epoch",
            ));
        }
    }
    for epoch in completed(right)?.values() {
        let retained = output_epochs
            .get(&epoch.start.import_id)
            .ok_or_else(|| conflict("reconciliation lost a replayed import epoch"))?;
        if !same_authority(retained, epoch, output, right)? {
            return Err(conflict("replayed import authority changed"));
        }
    }
    Ok(())
}

fn completed(
    session: &RelSession,
) -> Result<BTreeMap<String, CompletedEpoch>, RelReconciliationError> {
    let mut result = BTreeMap::new();
    for status in session.imports.statuses() {
        let ImportEpochStatus::Completed { start, complete } = status else {
            return Err(RelReconciliationError::UnsupportedState(
                "open import epoch in divergent reconciliation",
            ));
        };
        result.insert(start.import_id.clone(), CompletedEpoch { start, complete });
    }
    Ok(result)
}

fn same_authority(
    left: &CompletedEpoch,
    right: &CompletedEpoch,
    left_session: &RelSession,
    right_session: &RelSession,
) -> Result<bool, RelReconciliationError> {
    let mut left_start = left.start.clone();
    let mut right_start = right.start.clone();
    left_start.pre = ImportWatermarks::default();
    right_start.pre = ImportWatermarks::default();
    Ok(left_start == right_start
        && left.complete.format_version == right.complete.format_version
        && left.complete.owner_uuid == right.complete.owner_uuid
        && left.complete.import_id == right.complete.import_id
        && left.complete.receipt_digest == right.complete.receipt_digest
        && left.complete.completed_at_ns == right.complete.completed_at_ns
        && output_receipt(left, left_session)? == output_receipt(right, right_session)?
        && archive_evidence(left_session, left)? == archive_evidence(right_session, right)?
        && grouped_evidence(left_session, left)? == grouped_evidence(right_session, right)?)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArchiveEvidence {
    turns: Vec<reliquary_archive::HistoricalArchiveTurn>,
    branches: Vec<reliquary_archive::Branch>,
    episodes: Vec<reliquary_archive::Episode>,
    receipts: Vec<crate::model::ImportReceipt>,
}

fn archive_evidence(
    session: &RelSession,
    epoch: &CompletedEpoch,
) -> Result<ArchiveEvidence, RelReconciliationError> {
    let (before, after) = epoch_archive_snapshots(&session.archive, epoch)?;
    let before_turns = before
        .turns
        .iter()
        .map(|turn| (turn.node.conversation_id.clone(), turn.node.id.clone()))
        .collect::<BTreeSet<_>>();
    let mut turns = after
        .turns
        .into_iter()
        .filter(|turn| {
            !before_turns.contains(&(turn.node.conversation_id.clone(), turn.node.id.clone()))
        })
        .collect::<Vec<_>>();
    for turn in &mut turns {
        turn.activity_position = None;
    }
    let before_branches = before
        .branches
        .iter()
        .map(|branch| ((branch.conversation_id.clone(), branch.id.clone()), branch))
        .collect::<BTreeMap<_, _>>();
    let branches = after
        .branches
        .into_iter()
        .filter(|branch| {
            before_branches
                .get(&(branch.conversation_id.clone(), branch.id.clone()))
                .copied()
                != Some(branch)
        })
        .collect();
    let before_episodes = before
        .episodes
        .iter()
        .map(|episode| episode.id)
        .collect::<BTreeSet<_>>();
    let episodes = after
        .episodes
        .into_iter()
        .filter(|episode| !before_episodes.contains(&episode.id))
        .collect();
    let before_receipts = before
        .import_receipts
        .iter()
        .map(|receipt| receipt.import_key)
        .collect::<BTreeSet<_>>();
    let receipts = after
        .import_receipts
        .into_iter()
        .filter(|receipt| !before_receipts.contains(&receipt.import_key))
        .collect();
    Ok(ArchiveEvidence {
        turns,
        branches,
        episodes,
        receipts,
    })
}

fn grouped_evidence(
    session: &RelSession,
    epoch: &CompletedEpoch,
) -> Result<Vec<GroupedInsomniaCompletion>, RelReconciliationError> {
    let episodes = imported_episodes(&session.archive, epoch)?
        .into_iter()
        .map(|id| id.0)
        .collect::<BTreeSet<_>>();
    let mut completions = session
        .grouped_completions
        .values()
        .filter(|completion| episodes.contains(&completion.episode_id))
        .cloned()
        .collect::<Vec<_>>();
    for completion in &mut completions {
        completion.global_version_start = 0;
        completion.transaction_time_ns = 0;
        completion.freshness_birth_turn = completion.freshness_birth_turn.map(|_| 0);
        completion.memory_records = completion
            .memory_records
            .iter()
            .map(|bytes| {
                let mut record = decode_grouped_creation(bytes).map_err(conflict)?;
                record.memory_version = 0;
                encode_grouped_creation(&record).map_err(conflict)
            })
            .collect::<Result<Vec<_>, _>>()?;
    }
    completions.sort_by_key(|completion| (completion.episode_id, completion.attempt));
    Ok(completions)
}

fn output_receipt<'a>(
    epoch: &CompletedEpoch,
    session: &'a RelSession,
) -> Result<&'a crate::model::ImportReceipt, RelReconciliationError> {
    let key = decode_key(&epoch.start.import_key)
        .ok_or_else(|| conflict("completed import has a noncanonical source key"))?;
    session
        .archive
        .import_receipt(&key)
        .ok_or_else(|| conflict("completed import lacks its Archive receipt"))
}

fn replay_archive_range(
    writer: &mut RelWriter,
    source: &Archive,
    from: u64,
    through: u64,
    import_id: Option<&str>,
    require_exact_interval: bool,
    counts: &mut ReplayCounts,
) -> Result<Vec<EpisodeId>, RelReconciliationError> {
    if through < from || through > source.archive_version() {
        return Err(RelReconciliationError::UnsupportedState(
            "import Archive replay range is invalid",
        ));
    }
    if from == through {
        return Ok(Vec::new());
    }
    let before = source.history_at(from).map_err(conflict)?;
    let after = source.history_at(through).map_err(conflict)?;
    reject_unhandled_archive_changes(&before, &after)?;

    let before_turns = before
        .turns
        .iter()
        .map(|turn| (turn.node.conversation_id.clone(), turn.node.id.clone()))
        .collect::<BTreeSet<_>>();
    let new_turns = after
        .turns
        .iter()
        .filter(|turn| {
            !before_turns.contains(&(turn.node.conversation_id.clone(), turn.node.id.clone()))
        })
        .collect::<Vec<_>>();
    for turn in &new_turns {
        let mut attachments = Vec::new();
        let mut project_attachments = Vec::new();
        for attachment in &turn.attachments {
            if let Some(reference) = attachment.project_reference.clone() {
                project_attachments.push(reliquary_archive::ProjectAttachmentInput {
                    filename: attachment.filename.clone(),
                    mime_type: attachment.mime_type.clone(),
                    content_id: attachment.content_id,
                    byte_length: attachment.byte_length,
                    reference,
                });
            } else {
                attachments.push(reliquary_archive::AttachmentInput {
                    filename: attachment.filename.clone(),
                    mime_type: attachment.mime_type.clone(),
                    bytes: attachment.bytes.clone(),
                });
            }
        }
        let accepted = writer
            .session
            .archive
            .accept_turn(
                &mut writer.container,
                reliquary_archive::TurnInput {
                    id: turn.node.id.clone(),
                    conversation_id: turn.node.conversation_id.clone(),
                    parent_id: turn.node.parent_id.clone(),
                    role: turn.node.role.clone(),
                    principal_id: turn.node.principal_id.clone(),
                    timestamp_ns: turn.node.timestamp_ns,
                    content: turn.content.clone().ok_or(
                        RelReconciliationError::UnsupportedState(
                            "pruned Archive content in import replay",
                        ),
                    )?,
                    attachments,
                    project_attachments,
                },
            )
            .map_err(conflict)?;
        counts.archive += usize::from(accepted.inserted);
        writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    }

    let before_branches = before
        .branches
        .iter()
        .map(|branch| ((branch.conversation_id.clone(), branch.id.clone()), branch))
        .collect::<BTreeMap<_, _>>();
    let changed_branches = after
        .branches
        .iter()
        .filter(|branch| {
            before_branches.get(&(branch.conversation_id.clone(), branch.id.clone()))
                != Some(branch)
        })
        .cloned()
        .collect::<Vec<_>>();
    for branch in &changed_branches {
        counts.archive += usize::from(
            writer
                .session
                .archive
                .put_branch(&mut writer.container, branch.clone())
                .map_err(conflict)?,
        );
        writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    }

    let before_episodes = before
        .episodes
        .iter()
        .map(|episode| episode.id)
        .collect::<BTreeSet<_>>();
    let new_episodes = after
        .episodes
        .iter()
        .filter(|episode| !before_episodes.contains(&episode.id))
        .cloned()
        .collect::<Vec<_>>();
    for episode in &new_episodes {
        let existed = writer.session.archive.episode(episode.id).is_some();
        let replayed = if let Some(import_id) = import_id {
            if episode.origin != EpisodeOrigin::Import
                || episode.boundary != EpisodeBoundary::ImportEnd
            {
                return Err(RelReconciliationError::UnsupportedState(
                    "import interval contains a non-import Episode",
                ));
            }
            writer
                .finalize_native_import_episode(
                    import_id,
                    &episode.conversation_id,
                    &episode.start_node_id,
                    &episode.end_node_id,
                    episode.finalized_at_ns,
                )
                .map_err(verification)?
        } else {
            writer
                .finalize_episode(
                    &episode.conversation_id,
                    &episode.start_node_id,
                    &episode.end_node_id,
                    episode.origin,
                    episode.boundary,
                    episode.finalized_at_ns,
                )
                .map_err(verification)?
        };
        if replayed != episode.id {
            return Err(conflict("Archive Episode stable identity changed"));
        }
        counts.archive += usize::from(!existed);
    }

    let before_receipts = before
        .import_receipts
        .iter()
        .map(|receipt| receipt.import_key)
        .collect::<BTreeSet<_>>();
    let new_receipts = after
        .import_receipts
        .iter()
        .filter(|receipt| !before_receipts.contains(&receipt.import_key))
        .cloned()
        .collect::<Vec<_>>();
    for receipt in &new_receipts {
        let Some(import_id) = import_id else {
            if writer
                .session
                .archive
                .import_receipt(&receipt.import_key)
                .is_none()
            {
                return Err(RelReconciliationError::UnsupportedState(
                    "Archive import receipt is outside its replayed epoch",
                ));
            }
            continue;
        };
        let replayed = writer
            .commit_native_import_receipt(import_id, receipt.clone())
            .map_err(verification)?;
        if replayed != *receipt {
            return Err(conflict("Archive import receipt changed during replay"));
        }
        counts.archive += 1;
    }

    if require_exact_interval {
        let represented =
            new_turns.len() + changed_branches.len() + new_episodes.len() + new_receipts.len();
        if u64::try_from(represented).ok() != Some(through - from) {
            return Err(RelReconciliationError::UnsupportedState(
                "import Archive interval has unrepresented revision history",
            ));
        }
    }
    Ok(new_episodes.into_iter().map(|episode| episode.id).collect())
}

fn reject_unhandled_archive_changes(
    before: &ArchiveHistorySnapshot,
    after: &ArchiveHistorySnapshot,
) -> Result<(), RelReconciliationError> {
    if before.fragment_definitions != after.fragment_definitions
        || before.conversation_tombstones != after.conversation_tombstones
    {
        return Err(RelReconciliationError::UnsupportedState(
            "import interval contains unsupported Archive maintenance state",
        ));
    }
    Ok(())
}

fn replay_grouped_completions(
    writer: &mut RelWriter,
    source: &RelSession,
    import_id: &str,
    episodes: &[EpisodeId],
    counts: &mut ReplayCounts,
    rebased_freshness: &mut BTreeSet<FreshnessMemoryId>,
) -> Result<(), RelReconciliationError> {
    for episode in episodes {
        let mut completions = source
            .grouped_completions
            .values()
            .filter(|completion| completion.episode_id == episode.0)
            .cloned()
            .collect::<Vec<_>>();
        completions.sort_by_key(|completion| completion.attempt);
        if completions.len() != 1 || completions[0].attempt != 1 {
            return Err(RelReconciliationError::UnsupportedState(
                "import Episode journal lacks one replayable grouped completion",
            ));
        }
        let source_completion = completions.pop().unwrap();
        if !source_completion.external_memory_refs.is_empty()
            || !source_completion.routing_metadata.is_empty()
        {
            return Err(RelReconciliationError::UnsupportedState(
                "import grouped completion crosses an unsupported owner boundary",
            ));
        }
        let duration = source_completion
            .completed_at_ns
            .saturating_sub(source_completion.started_at_ns)
            .saturating_add(1)
            .max(1);
        let claim = writer
            .claim_native_import_episode(
                import_id,
                *episode,
                "semantic-reconciliation",
                source_completion.started_at_ns,
                duration,
            )
            .map_err(verification)?;
        let mut completion = source_completion;
        completion.attempt = claim.work.attempt_count;
        completion.global_version_start = writer.next_global_version().map_err(verification)?;
        let base_memory_version = writer.session.common.memories.memory_version();
        completion.memory_records = completion
            .memory_records
            .iter()
            .enumerate()
            .map(|(index, bytes)| {
                let mut record = decode_grouped_creation(bytes).map_err(conflict)?;
                record.memory_version = base_memory_version
                    .checked_add(index as u64 + 1)
                    .ok_or_else(|| conflict("Memory version exhausted during import replay"))?;
                encode_grouped_creation(&record).map_err(conflict)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if completion.freshness_birth_turn.is_some() {
            completion.freshness_birth_turn = Some(writer.session.archive.rel_turn_count());
            rebased_freshness.extend(
                completion
                    .freshness_birth_memory_ids
                    .iter()
                    .copied()
                    .map(FreshnessMemoryId),
            );
        }
        let token = claim
            .work
            .lease_token
            .ok_or_else(|| conflict("replayed import claim lacks a lease"))?;
        writer
            .commit_native_import_completion_with_routed_receipts(
                import_id,
                token,
                completion.clone(),
                completion.completed_at_ns,
                &[],
            )
            .map_err(verification)?;
        counts.memory += completion.memory_records.len();
        counts.freshness += completion.freshness_birth_memory_ids.len();
    }
    Ok(())
}

fn imported_episodes(
    archive: &Archive,
    epoch: &CompletedEpoch,
) -> Result<Vec<EpisodeId>, RelReconciliationError> {
    let (before, after) = epoch_archive_snapshots(archive, epoch)?;
    let known = before
        .episodes
        .into_iter()
        .map(|episode| episode.id)
        .collect::<BTreeSet<_>>();
    Ok(after
        .episodes
        .into_iter()
        .filter(|episode| !known.contains(&episode.id))
        .map(|episode| episode.id)
        .collect())
}

fn epoch_archive_snapshots(
    archive: &Archive,
    epoch: &CompletedEpoch,
) -> Result<(ArchiveHistorySnapshot, ArchiveHistorySnapshot), RelReconciliationError> {
    let pre = epoch
        .start
        .pre
        .archive_version
        .ok_or(RelReconciliationError::UnsupportedState(
            "completed import lacks an Archive pre-watermark",
        ))?;
    let post =
        epoch
            .complete
            .post
            .archive_version
            .ok_or(RelReconciliationError::UnsupportedState(
                "completed import lacks an Archive post-watermark",
            ))?;
    let before = archive.history_at(pre).map_err(conflict)?;
    let after = archive.history_at(post).map_err(conflict)?;
    Ok((before, after))
}

fn decode_key(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut key = [0; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        key[index] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Some(key)
}

fn nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}
