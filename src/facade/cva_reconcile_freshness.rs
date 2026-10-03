//! Rebase owner-local Freshness journal entries when divergent REL histories are merged.
//!
//! Source turns are tied to accepted archive activity identity. Reconciliation preserves
//! event results as pinned deltas, but places them at the corresponding destination cut.
use crate::freshness_storage::FreshnessReplayEntry;
use crate::{Cva, CvaReconcileError};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum ReplayKind {
    Initialize(crate::MemoryId),
    Admit(crate::MemoryId),
    Event(String),
    AcceptedUse(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RebasedEntry {
    turn: u64,
    kind: ReplayKind,
    entry: FreshnessReplayEntry,
}

pub(crate) fn replay_freshness(
    left: &Cva,
    right: &Cva,
    output: &mut Cva,
) -> Result<(), CvaReconcileError> {
    let any = left.freshness.has_any()
        || right.freshness.has_any()
        || left.dream_freshness.has_any()
        || right.dream_freshness.has_any();
    if !any {
        return Ok(());
    }

    if left.freshness_policy() != right.freshness_policy() {
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "divergent Freshness owner policy",
        ));
    }
    if left.freshness_has_pending_dream_passes() || right.freshness_has_pending_dream_passes() {
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "pending Freshness Dream pass or publication intent",
        ));
    }

    let left_turns = activity_turns(left);
    let right_turns = activity_turns(right);
    let merged_turns: BTreeMap<_, _> = activity_turns(output)
        .into_iter()
        .map(|(turn, identity)| (identity, turn))
        .collect();
    if merged_turns != merged_activity_turns(left, right) {
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "destination Archive activity index differs from the planned Freshness mapping",
        ));
    }
    let mut by_identity = BTreeMap::<String, RebasedEntry>::new();
    for (source, turns) in [(left, &left_turns), (right, &right_turns)] {
        for entry in source.freshness_replay_entries()? {
            let source_turn = entry.source_turn();
            let destination_turn = rebase_turn(source_turn, turns, &merged_turns)?;
            let kind = match &entry {
                FreshnessReplayEntry::Initialize { memory_id, .. } => {
                    ReplayKind::Initialize(*memory_id)
                }
                FreshnessReplayEntry::Admit { memory_id, .. } => ReplayKind::Admit(*memory_id),
                FreshnessReplayEntry::AcceptedUseReceipt { use_id, .. } => {
                    ReplayKind::AcceptedUse(use_id.clone())
                }
                FreshnessReplayEntry::Event {
                    event_id, proof, ..
                } => {
                    ensure_event_proof_preserved(source, event_id, *proof)?;
                    ReplayKind::Event(event_id.clone())
                }
            };
            let stable_key = match &kind {
                ReplayKind::Initialize(id) => format!("I:{}", hex_id(*id)),
                ReplayKind::Admit(id) => format!("A:{}", hex_id(*id)),
                ReplayKind::Event(id) => format!("E:{id}"),
                ReplayKind::AcceptedUse(id) => format!("U:{id}"),
            };
            let rebased = RebasedEntry {
                turn: destination_turn,
                kind,
                entry,
            };
            if let Some(previous) = by_identity.get(&stable_key) {
                if previous.turn != rebased.turn
                    || !same_rebased_identity(&previous.entry, &rebased.entry)
                {
                    return Err(CvaReconcileError::UnsupportedSemanticOwner(
                        "conflicting Freshness event or admission identity",
                    ));
                }
                continue;
            }
            by_identity.insert(stable_key, rebased);
        }
    }

    let mut entries: Vec<_> = by_identity.into_values().collect();
    entries.sort_by(|a, b| (a.turn, &a.kind).cmp(&(b.turn, &b.kind)));

    output
        .set_freshness_policy_before_enrollment(left.freshness_policy())
        .map_err(|_| {
            CvaReconcileError::UnsupportedSemanticOwner("divergent Freshness owner policy")
        })?;
    for rebased in entries {
        match rebased.entry {
            FreshnessReplayEntry::Initialize {
                memory_id,
                admitted_at_birth,
                ..
            } => output.freshness_replay_initialize(memory_id, rebased.turn, admitted_at_birth)?,
            FreshnessReplayEntry::Admit { memory_id, .. } => {
                output.freshness_replay_admit(memory_id, rebased.turn)?
            }
            FreshnessReplayEntry::Event {
                event_id,
                effects,
                proof,
                provenance,
                ..
            } => {
                output.freshness_replay_event(event_id, rebased.turn, proof, effects, provenance)?
            }
            FreshnessReplayEntry::AcceptedUseReceipt {
                use_id,
                sources,
                policy_version,
                ..
            } => output.freshness_replay_accepted_use_receipt(
                use_id,
                rebased.turn,
                sources,
                policy_version,
            )?,
        }
    }

    let publication_births: BTreeSet<_> = left
        .freshness_publication_birth_ids()
        .into_iter()
        .chain(right.freshness_publication_birth_ids())
        .collect();
    for id in publication_births {
        output.freshness_replay_publication_birth(id)?;
    }

    let settled: BTreeSet<_> = left
        .freshness_settled_dream_pass_ids()
        .into_iter()
        .chain(right.freshness_settled_dream_pass_ids())
        .collect();
    for pass_id in settled {
        output.freshness_replay_settled_dream_pass(&pass_id)?;
    }
    Ok(())
}

/// Perform source-history checks before the destination file is created.
pub(crate) fn validate_freshness(left: &Cva, right: &Cva) -> Result<(), CvaReconcileError> {
    if (left.freshness.has_any()
        || right.freshness.has_any()
        || left.dream_freshness.has_any()
        || right.dream_freshness.has_any())
        && left.freshness_policy() != right.freshness_policy()
    {
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "divergent Freshness owner policy",
        ));
    }
    if left.freshness_has_pending_dream_passes() || right.freshness_has_pending_dream_passes() {
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "pending Freshness Dream pass or publication intent",
        ));
    }
    let left_turns = activity_turns(left);
    let right_turns = activity_turns(right);
    let merged = merged_activity_turns(left, right);
    let mut identities = BTreeMap::<String, (u64, FreshnessReplayEntry)>::new();
    for (source, turns) in [(left, &left_turns), (right, &right_turns)] {
        for entry in source.freshness_replay_entries()? {
            let destination_turn = rebase_turn(entry.source_turn(), turns, &merged)?;
            let identity = match &entry {
                FreshnessReplayEntry::Initialize { memory_id, .. } => {
                    format!("I:{}", hex_id(*memory_id))
                }
                FreshnessReplayEntry::Admit { memory_id, .. } => {
                    format!("A:{}", hex_id(*memory_id))
                }
                FreshnessReplayEntry::AcceptedUseReceipt { use_id, .. } => format!("U:{use_id}"),
                FreshnessReplayEntry::Event {
                    event_id, proof, ..
                } => {
                    ensure_event_proof_preserved(source, event_id, *proof)?;
                    format!("E:{event_id}")
                }
            };
            if let Some((previous_turn, previous)) = identities.get(&identity) {
                if *previous_turn != destination_turn || !same_rebased_identity(previous, &entry) {
                    return Err(CvaReconcileError::UnsupportedSemanticOwner(
                        "conflicting Freshness replay identity",
                    ));
                }
            } else {
                identities.insert(identity, (destination_turn, entry));
            }
        }
    }
    Ok(())
}

// Accepted producer receipts preserve the pinned proof even when an optional
// derived Community snapshot is absent from a transformed owner.
fn ensure_event_proof_preserved(
    source: &Cva,
    event_id: &str,
    proof: crate::FreshnessEventProof,
) -> Result<(), CvaReconcileError> {
    let accepted_use = source
        .freshness
        .event_has_accepted_provenance(event_id)
        .map_err(|_| {
            CvaReconcileError::UnsupportedSemanticOwner(
                "invalid accepted Freshness source provenance",
            )
        })?;
    if accepted_use
        || source.initial_dream_pass_settled(event_id)
        || source.freshness_event_proof_is_available(proof)
    {
        return Ok(());
    }
    Err(CvaReconcileError::UnsupportedSemanticOwner(
        "Freshness event source graph/community proof is unavailable",
    ))
}

fn same_rebased_provenance(
    a: &Option<crate::freshness_storage::FreshnessEventProvenance>,
    b: &Option<crate::freshness_storage::FreshnessEventProvenance>,
) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => {
            a.producer_kind == b.producer_kind
                && a.source_id == b.source_id
                && a.origin_owner == b.origin_owner
                && a.policy_version == b.policy_version
        }
        (None, None) => true,
        _ => false,
    }
}

fn same_rebased_identity(a: &FreshnessReplayEntry, b: &FreshnessReplayEntry) -> bool {
    match (a, b) {
        (
            FreshnessReplayEntry::Initialize {
                memory_id: a,
                admitted_at_birth: ab,
                ..
            },
            FreshnessReplayEntry::Initialize {
                memory_id: b,
                admitted_at_birth: bb,
                ..
            },
        ) => a == b && ab == bb,
        (
            FreshnessReplayEntry::Admit { memory_id: a, .. },
            FreshnessReplayEntry::Admit { memory_id: b, .. },
        ) => a == b,
        (
            FreshnessReplayEntry::Event {
                event_id: ai,
                effects: ae,
                proof: ap,
                provenance: av,
                ..
            },
            FreshnessReplayEntry::Event {
                event_id: bi,
                effects: be,
                proof: bp,
                provenance: bv,
                ..
            },
        ) => ai == bi && ae == be && ap == bp && same_rebased_provenance(av, bv),
        (
            FreshnessReplayEntry::AcceptedUseReceipt {
                use_id: a,
                sources: sa,
                policy_version: pa,
                ..
            },
            FreshnessReplayEntry::AcceptedUseReceipt {
                use_id: b,
                sources: sb,
                policy_version: pb,
                ..
            },
        ) => a == b && sa == sb && pa == pb,
        _ => false,
    }
}

fn activity_turns(cva: &Cva) -> BTreeMap<u64, (String, String)> {
    cva.archive
        .nodes
        .iter()
        .filter_map(|node| {
            cva.archive
                .activity_position_for_turn(&node.conversation_id, &node.id)
                .map(|turn| (turn, (node.conversation_id.clone(), node.id.clone())))
        })
        .collect()
}

fn merged_activity_turns(left: &Cva, right: &Cva) -> BTreeMap<(String, String), u64> {
    let mut ordered: Vec<(String, String)> = activity_turns(left).into_values().collect();
    let mut known: BTreeSet<_> = ordered.iter().cloned().collect();
    for identity in activity_turns(right).into_values() {
        if known.insert(identity.clone()) {
            ordered.push(identity);
        }
    }
    ordered
        .into_iter()
        .enumerate()
        .map(|(i, identity)| {
            (
                identity,
                u64::try_from(i).unwrap_or(u64::MAX).saturating_add(1),
            )
        })
        .collect()
}

fn rebase_turn(
    source_turn: u64,
    source: &BTreeMap<u64, (String, String)>,
    merged: &BTreeMap<(String, String), u64>,
) -> Result<u64, CvaReconcileError> {
    if source_turn == 0 {
        return Ok(0);
    }
    let identity = source
        .get(&source_turn)
        .ok_or(CvaReconcileError::UnsupportedSemanticOwner(
            "Freshness source cut has no accepted Archive activity identity",
        ))?;
    merged
        .get(identity)
        .copied()
        .ok_or(CvaReconcileError::UnsupportedSemanticOwner(
            "Freshness source activity is absent from merged Archive order",
        ))
}

fn hex_id(id: crate::MemoryId) -> String {
    id.0.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Whether the right input contributes new accepted Freshness authority.
/// Re-emitting the shared left projection into a temporary output is not a change.
pub(crate) fn right_changes_freshness(left: &Cva, right: &Cva) -> Result<bool, CvaReconcileError> {
    if right.freshness.has_any() && !left.freshness.has_any() {
        return Ok(true);
    }
    let identity = |entry: &FreshnessReplayEntry| match entry {
        FreshnessReplayEntry::Initialize { memory_id, .. } => format!("I:{}", hex_id(*memory_id)),
        FreshnessReplayEntry::Admit { memory_id, .. } => format!("A:{}", hex_id(*memory_id)),
        FreshnessReplayEntry::Event { event_id, .. } => format!("E:{event_id}"),
        FreshnessReplayEntry::AcceptedUseReceipt { use_id, .. } => format!("U:{use_id}"),
    };
    let known: BTreeSet<_> = left
        .freshness_replay_entries()?
        .iter()
        .map(&identity)
        .collect();
    if right
        .freshness_replay_entries()?
        .iter()
        .any(|entry| !known.contains(&identity(entry)))
    {
        return Ok(true);
    }
    let births: BTreeSet<_> = left.freshness_publication_birth_ids().into_iter().collect();
    let settled: BTreeSet<_> = left
        .freshness_settled_dream_pass_ids()
        .into_iter()
        .collect();
    Ok(right
        .freshness_publication_birth_ids()
        .iter()
        .any(|id| !births.contains(id))
        || right
            .freshness_settled_dream_pass_ids()
            .iter()
            .any(|id| !settled.contains(id)))
}
