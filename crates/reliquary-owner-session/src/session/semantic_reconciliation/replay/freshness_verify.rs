use super::*;
use reliquary_freshness_store::FreshnessReplayEntry;

pub(super) fn replay_freshness(
    target: &mut Container,
    source: &FreshnessStore,
    destination: &mut FreshnessStore,
    rebased: &BTreeSet<FreshnessMemoryId>,
    counts: &mut ReplayCounts,
) -> Result<(), RelReconciliationError> {
    for entry in source.replay_entries() {
        if let FreshnessReplayEntry::Birth {
            memory_id,
            admitted_at_birth,
            ..
        } = &entry
            && rebased.contains(memory_id)
        {
            let retained = destination.replay_entries().into_iter().any(|candidate| {
                matches!(candidate, FreshnessReplayEntry::Birth {
                    memory_id: candidate_id,
                    admitted_at_birth: candidate_admitted,
                    ..
                } if candidate_id == *memory_id && candidate_admitted == *admitted_at_birth)
            });
            if !retained {
                return Err(conflict("rebased import Freshness birth changed"));
            }
            continue;
        }
        let inserted = match entry {
            FreshnessReplayEntry::Birth {
                memory_id,
                accepted_turn,
                admitted_at_birth,
            } => destination.create_birth(target, memory_id, accepted_turn, admitted_at_birth),
            FreshnessReplayEntry::Admission {
                memory_id,
                accepted_turn,
            } => destination.admit(target, memory_id, accepted_turn),
            FreshnessReplayEntry::Event(event) => destination.append_event(target, event),
            FreshnessReplayEntry::AcceptedUse(receipt, proof) => destination.accept_use(
                target,
                receipt.use_id,
                receipt.accepted_turn,
                receipt.sources,
                proof,
            ),
        }
        .map_err(conflict)?;
        counts.freshness += usize::from(inserted);
    }
    Ok(())
}

pub(super) fn verify_merged(
    left: &RelSession,
    right: &RelSession,
    output: &RelSession,
    rebased: &BTreeSet<FreshnessMemoryId>,
) -> Result<(), RelReconciliationError> {
    for source in [left, right] {
        for accepted in source.archive.accepted_turn_sources() {
            if output
                .archive
                .node(&accepted.conversation_id, &accepted.node_id)
                != source
                    .archive
                    .node(&accepted.conversation_id, &accepted.node_id)
            {
                return Err(conflict("reopened Archive lost accepted history"));
            }
        }
        for id in source.common.memories.memory_ids() {
            let expected = source.common.memories.history(id).map_err(conflict)?;
            let actual = output.common.memories.history(id).map_err(conflict)?;
            for revision in expected.revisions {
                if !actual.revisions.iter().any(|candidate| {
                    candidate.record.id == revision.record.id
                        && candidate.record.revision == revision.record.revision
                        && candidate.record.mutation_id == revision.record.mutation_id
                        && candidate.record.body_id == revision.record.body_id
                }) {
                    return Err(conflict("reopened Memory history lost a revision"));
                }
            }
            if expected.tombstone.is_some() && actual.tombstone.is_none() {
                return Err(conflict("reopened Memory history lost a tombstone"));
            }
        }
        let output_freshness = output.freshness.replay_entries();
        for entry in source.freshness.replay_entries() {
            if let FreshnessReplayEntry::Birth {
                memory_id,
                admitted_at_birth,
                ..
            } = &entry
                && rebased.contains(memory_id)
                && output_freshness.iter().any(|candidate| {
                    matches!(candidate, FreshnessReplayEntry::Birth {
                        memory_id: candidate_id,
                        admitted_at_birth: candidate_admitted,
                        ..
                    } if candidate_id == memory_id && candidate_admitted == admitted_at_birth)
                })
            {
                continue;
            }
            if !output_freshness.contains(&entry) {
                return Err(conflict(
                    "reopened Freshness history lost accepted evidence",
                ));
            }
        }
    }
    Ok(())
}
