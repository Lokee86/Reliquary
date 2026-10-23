//! Freshness source merge policy for owner reconciliation.

use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::freshness_storage::{FreshnessReplayEntry, FreshnessStore};
use crate::history::freshness_reconciliation::FreshnessReplaySource;
use crate::owner_lifecycle::{OwnerError, freshness_reconciliation};
use crate::{Archive, Container, CvaReconcileError, MemoryId};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct FreshnessReplayDestination<'a> {
    pub(crate) container: &'a mut Container,
    pub(crate) archive: &'a Archive,
    pub(crate) memories: &'a crate::memory_store::MemoryStore,
    pub(crate) owner_uuid: Option<[u8; 16]>,
    pub(crate) freshness: &'a mut FreshnessStore,
    pub(crate) dream_freshness: &'a mut DreamFreshnessJournal,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum ReplayKind {
    RewriteBaseline(MemoryId),
    Initialize(MemoryId),
    Admit(MemoryId),
    Event(String),
    AcceptedUse(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RebasedEntry {
    turn: u64,
    kind: ReplayKind,
    entry: FreshnessReplayEntry,
}

pub(crate) fn validate_freshness(
    left: &FreshnessReplaySource<'_>,
    right: &FreshnessReplaySource<'_>,
) -> Result<(), CvaReconcileError> {
    if !left.freshness.import_settlements().is_empty()
        || !right.freshness.import_settlements().is_empty()
    {
        if left.freshness.policy() != right.freshness.policy() {
            return Err(CvaReconcileError::UnsupportedSemanticOwner(
                "historical checkpoint owner policy conflict",
            ));
        }
        if !left.freshness.import_settlements().is_empty()
            && !right.freshness.import_settlements().is_empty()
            && left.freshness.import_settlements() != right.freshness.import_settlements()
        {
            return Err(CvaReconcileError::UnsupportedSemanticOwner(
                "ImportManifestConflict: historical checkpoint authority",
            ));
        }
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "historical checkpoint source rebasing without a verified reconstruction manifest",
        ));
    }
    merged_semantic_activation(left, right)?;
    if (left.freshness.has_any()
        || right.freshness.has_any()
        || left.dream_freshness.has_any()
        || right.dream_freshness.has_any())
        && left.freshness.policy() != right.freshness.policy()
    {
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "divergent Freshness owner policy",
        ));
    }
    if left.dream_freshness.has_pending() || right.dream_freshness.has_pending() {
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "pending Freshness Dream pass or publication intent",
        ));
    }
    let left_turns = activity_turns(left.archive);
    let right_turns = activity_turns(right.archive);
    let merged = merged_activity_turns(left, right);
    let mut identities = BTreeMap::<String, (u64, FreshnessReplayEntry)>::new();
    for (source, turns) in [(left, &left_turns), (right, &right_turns)] {
        for entry in source
            .freshness
            .replay_entries()
            .map_err(crate::owner_lifecycle::OwnerError::Freshness)?
        {
            crate::history::freshness_reconciliation::validate_semantic_source(source, &entry)?;
            let destination_turn = rebase_entry_turn(source, &entry, turns, &merged)?;
            if let FreshnessReplayEntry::Event {
                event_id, proof, ..
            } = &entry
            {
                crate::history::freshness_reconciliation::ensure_event_proof_preserved(
                    source, event_id, *proof,
                )?;
            }
            let identity = replay_identity(source, &entry)?;
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

pub(crate) fn right_changes_freshness(
    left: &FreshnessReplaySource<'_>,
    right: &FreshnessReplaySource<'_>,
) -> Result<bool, CvaReconcileError> {
    if right.freshness.semantic_activation_seen() && !left.freshness.semantic_activation_seen() {
        return Ok(true);
    }
    if right.freshness.has_any() && !left.freshness.has_any() {
        return Ok(true);
    }
    let identity = |entry: &FreshnessReplayEntry| replay_identity(left, entry);
    let known: BTreeSet<_> = left
        .freshness
        .replay_entries()
        .map_err(crate::owner_lifecycle::OwnerError::Freshness)?
        .iter()
        .map(&identity)
        .collect::<Result<_, _>>()?;
    if right
        .freshness
        .replay_entries()
        .map_err(crate::owner_lifecycle::OwnerError::Freshness)?
        .iter()
        .any(|entry| identity(entry).is_ok_and(|id| !known.contains(&id)))
    {
        return Ok(true);
    }
    let births: BTreeSet<_> = left
        .dream_freshness
        .publication_birth_ids()
        .into_iter()
        .collect();
    let settled: BTreeSet<_> = left
        .dream_freshness
        .settled_ids()
        .map(str::to_owned)
        .collect();
    Ok(right
        .dream_freshness
        .publication_birth_ids()
        .iter()
        .any(|id| !births.contains(id))
        || right
            .dream_freshness
            .settled_ids()
            .any(|id| !settled.contains(id)))
}

pub(crate) fn replay_freshness(
    left: &FreshnessReplaySource<'_>,
    right: &FreshnessReplaySource<'_>,
    mut output: FreshnessReplayDestination<'_>,
) -> Result<(), CvaReconcileError> {
    let mut activation = merged_semantic_activation(left, right)?;
    if left.freshness.semantic_activation_seen() || right.freshness.semantic_activation_seen() {
        set_policy_before_enrollment(&mut output, left.freshness.policy())?;
        if let Some(a) = activation.as_mut() {
            a.accepted_turn = match &a.boundary {
                None => 0,
                Some((c, n)) => output.archive.activity_position_for_turn(c, n).ok_or(
                    CvaReconcileError::UnsupportedSemanticOwner(
                        "missing semantic activation boundary",
                    ),
                )?,
            };
        }
        let owner = output
            .owner_uuid
            .ok_or(CvaReconcileError::UnsupportedSemanticOwner(
                "semantic activation lacks owner",
            ))?;
        output
            .freshness
            .persist_semantic_activation(output.container, owner, activation)
            .map_err(|_| {
                CvaReconcileError::UnsupportedSemanticOwner("conflicting semantic activation")
            })?;
    }
    let any = left.freshness.has_any()
        || right.freshness.has_any()
        || left.dream_freshness.has_any()
        || right.dream_freshness.has_any();
    if !any {
        return Ok(());
    }
    if left.freshness.policy() != right.freshness.policy() {
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "divergent Freshness owner policy",
        ));
    }
    if left.dream_freshness.has_pending() || right.dream_freshness.has_pending() {
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "pending Freshness Dream pass or publication intent",
        ));
    }
    let left_turns = activity_turns(left.archive);
    let right_turns = activity_turns(right.archive);
    let merged_turns: BTreeMap<_, _> = activity_turns(output.archive)
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
        for entry in source
            .freshness
            .replay_entries()
            .map_err(crate::owner_lifecycle::OwnerError::Freshness)?
        {
            crate::history::freshness_reconciliation::validate_semantic_source(source, &entry)?;
            let destination_turn = rebase_entry_turn(source, &entry, turns, &merged_turns)?;
            if let FreshnessReplayEntry::Event {
                event_id, proof, ..
            } = &entry
            {
                crate::history::freshness_reconciliation::ensure_event_proof_preserved(
                    source, event_id, *proof,
                )?;
            }
            let kind = replay_kind(&entry);
            let stable_key = replay_identity(source, &entry)?;
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
    set_policy_before_enrollment(&mut output, left.freshness.policy()).map_err(|_| {
        CvaReconcileError::UnsupportedSemanticOwner("divergent Freshness owner policy")
    })?;
    for rebased in entries {
        if let FreshnessReplayEntry::RewriteBaseline { baseline } = &rebased.entry {
            if baseline.destination_cut != rebased.turn {
                return Err(CvaReconcileError::UnsupportedSemanticOwner(
                    "rewrite baseline activity prefix changed",
                ));
            }
        }
        let outcome = freshness_reconciliation::replay_entry(
            output.container,
            output.archive,
            output.memories,
            output.owner_uuid,
            output.freshness,
            rebased.entry,
            rebased.turn,
        )?;
        let _ = outcome;
    }
    let publication_births: BTreeSet<_> = left
        .dream_freshness
        .publication_birth_ids()
        .into_iter()
        .chain(right.dream_freshness.publication_birth_ids())
        .collect();
    for id in publication_births {
        replay_publication_birth(&mut output, id)?;
    }
    let settled: BTreeSet<_> = left
        .dream_freshness
        .settled_ids()
        .chain(right.dream_freshness.settled_ids())
        .map(str::to_owned)
        .collect();
    for pass_id in settled {
        replay_settled(&mut output, &pass_id)?;
    }
    Ok(())
}

fn replay_publication_birth(
    output: &mut FreshnessReplayDestination<'_>,
    id: MemoryId,
) -> Result<(), CvaReconcileError> {
    if !output.memories.contains_memory(id) {
        return Err(CvaReconcileError::Cva(OwnerError::Freshness(format!(
            "unknown owner-local Memory {:?}",
            id
        ))));
    }
    let payload = output
        .dream_freshness
        .replay_publication_birth_payload(id)
        .map_err(|error| CvaReconcileError::Cva(OwnerError::Freshness(error)))?;
    if !payload.is_empty() {
        output.container.append(&payload)?;
        output.container.sync()?;
        output
            .dream_freshness
            .ingest(&payload)
            .map_err(|error| CvaReconcileError::Cva(OwnerError::Freshness(error)))?;
    }
    Ok(())
}

fn replay_settled(
    output: &mut FreshnessReplayDestination<'_>,
    pass_id: &str,
) -> Result<(), CvaReconcileError> {
    let payload = output
        .dream_freshness
        .replay_settled_payload(pass_id)
        .map_err(|error| CvaReconcileError::Cva(OwnerError::Freshness(error)))?;
    if !payload.is_empty() {
        output.container.append(&payload)?;
        output.container.sync()?;
        output
            .dream_freshness
            .ingest(&payload)
            .map_err(|error| CvaReconcileError::Cva(OwnerError::Freshness(error)))?;
    }
    Ok(())
}

fn replay_kind(entry: &FreshnessReplayEntry) -> ReplayKind {
    match entry {
        FreshnessReplayEntry::RewriteBaseline { baseline } => {
            ReplayKind::RewriteBaseline(baseline.memory_id)
        }
        FreshnessReplayEntry::Initialize { memory_id, .. } => ReplayKind::Initialize(*memory_id),
        FreshnessReplayEntry::Admit { memory_id, .. } => ReplayKind::Admit(*memory_id),
        FreshnessReplayEntry::Event { event_id, .. } => ReplayKind::Event(event_id.clone()),
        FreshnessReplayEntry::AcceptedUseReceipt { use_id, .. } => {
            ReplayKind::AcceptedUse(use_id.clone())
        }
    }
}

fn replay_identity(
    source: &FreshnessReplaySource<'_>,
    entry: &FreshnessReplayEntry,
) -> Result<String, CvaReconcileError> {
    let _ = source;
    Ok(match replay_kind(entry) {
        ReplayKind::RewriteBaseline(id) => format!("B:{}", hex_id(id)),
        ReplayKind::Initialize(id) => format!("I:{}", hex_id(id)),
        ReplayKind::Admit(id) => format!("A:{}", hex_id(id)),
        ReplayKind::Event(id) => format!("E:{id}"),
        ReplayKind::AcceptedUse(id) => format!("U:{id}"),
    })
}

fn merged_semantic_activation(
    left: &FreshnessReplaySource<'_>,
    right: &FreshnessReplaySource<'_>,
) -> Result<Option<crate::freshness_storage::SemanticActivation>, CvaReconcileError> {
    let a = left.freshness.semantic_activation();
    let b = right.freshness.semantic_activation();
    if left.freshness.semantic_activation_seen()
        && right.freshness.semantic_activation_seen()
        && a.is_some() != b.is_some()
    {
        return Err(CvaReconcileError::UnsupportedSemanticOwner(
            "divergent semantic activation/cancellation authority",
        ));
    }
    for (source, activation) in [(left, a), (right, b)] {
        if let Some(activation) = activation {
            let at = activation.boundary.as_ref().map_or(Some(0), |(c, n)| {
                source.archive.activity_position_for_turn(c, n)
            });
            if source.owner_uuid != Some(activation.owner_uuid)
                || at != Some(activation.accepted_turn)
            {
                return Err(CvaReconcileError::UnsupportedSemanticOwner(
                    "unavailable semantic activation authority",
                ));
            }
        }
    }
    if let (Some(a), Some(b)) = (a, b) {
        if a.owner_uuid != b.owner_uuid
            || a.boundary != b.boundary
            || a.policy_version != b.policy_version
            || a.policy_digest != b.policy_digest
        {
            return Err(CvaReconcileError::UnsupportedSemanticOwner(
                "divergent semantic activation boundaries or policies",
            ));
        }
    }
    Ok(a.or(b).cloned())
}

fn activity_turns(archive: &Archive) -> BTreeMap<u64, (String, String)> {
    archive
        .nodes
        .iter()
        .filter_map(|node| {
            archive
                .activity_position_for_turn(&node.conversation_id, &node.id)
                .map(|turn| (turn, (node.conversation_id.clone(), node.id.clone())))
        })
        .collect()
}

fn merged_activity_turns(
    left: &FreshnessReplaySource<'_>,
    right: &FreshnessReplaySource<'_>,
) -> BTreeMap<(String, String), u64> {
    let mut ordered: Vec<_> = activity_turns(left.archive).into_values().collect();
    let mut known: BTreeSet<_> = ordered.iter().cloned().collect();
    for identity in activity_turns(right.archive).into_values() {
        if known.insert(identity.clone()) {
            ordered.push(identity);
        }
    }
    ordered
        .into_iter()
        .enumerate()
        .map(|(index, identity)| {
            (
                identity,
                u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1),
            )
        })
        .collect()
}

fn rebase_entry_turn(
    source: &FreshnessReplaySource<'_>,
    entry: &FreshnessReplayEntry,
    turns: &BTreeMap<u64, (String, String)>,
    merged: &BTreeMap<(String, String), u64>,
) -> Result<u64, CvaReconcileError> {
    if let FreshnessReplayEntry::Event {
        semantic: Some(e), ..
    } = entry
    {
        if let Some(fragment) = &e.fragment {
            return fragment
                .sources
                .iter()
                .filter(|(c, n)| source.archive.activity_position_for_turn(c, n).is_some())
                .map(|key| {
                    merged
                        .get(key)
                        .copied()
                        .ok_or(CvaReconcileError::UnsupportedSemanticOwner(
                            "missing fragment source mapping",
                        ))
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .max()
                .ok_or(CvaReconcileError::UnsupportedSemanticOwner(
                    "empty fragment source mapping",
                ));
        }
    }
    let source_turn = entry.source_turn();
    if source_turn == 0 {
        return Ok(0);
    }
    let identity = turns
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

fn same_rebased_identity(a: &FreshnessReplayEntry, b: &FreshnessReplayEntry) -> bool {
    match (a, b) {
        (
            FreshnessReplayEntry::RewriteBaseline { baseline: a },
            FreshnessReplayEntry::RewriteBaseline { baseline: b },
        ) => a == b,
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
                semantic: ase,
                ..
            },
            FreshnessReplayEntry::Event {
                event_id: bi,
                effects: be,
                proof: bp,
                provenance: bv,
                semantic: bse,
                ..
            },
        ) => {
            let normalize = |e: &Option<crate::freshness_storage::SemanticEventEvidence>| {
                e.clone().map(|mut evidence| {
                    evidence.accepted_turn = 0;
                    evidence
                })
            };
            ai == bi
                && ae == be
                && ap == bp
                && same_provenance(av, bv)
                && normalize(ase) == normalize(bse)
        }
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

fn same_provenance(
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

fn hex_id(id: MemoryId) -> String {
    id.0.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn set_policy_before_enrollment(
    output: &mut FreshnessReplayDestination<'_>,
    policy: crate::freshness::FreshnessPolicy,
) -> Result<(), OwnerError> {
    output.container.require_write_admission()?;
    if output.dream_freshness.has_any() || !output.freshness.records().is_empty() {
        return Err(OwnerError::Freshness(
            "Freshness policy is immutable after first enrollment".into(),
        ));
    }
    let owner = output.owner_uuid.ok_or_else(|| {
        OwnerError::Freshness(
            "legacy owner requires explicit migration before Freshness policy".into(),
        )
    })?;
    output
        .freshness
        .set_policy_before_enrollment(policy)
        .map_err(OwnerError::Freshness)?;
    output
        .freshness
        .persist_policy_if_needed(output.container, owner)
        .map_err(OwnerError::Freshness)
}
