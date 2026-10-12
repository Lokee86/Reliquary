//! Pure REL-local historical first-cycle Dream event production.
use super::historical_propagation::{HistoricalPropagationEvent, HistoricalPropagationKind};
use crate::{GraphRelationKind, GraphRelationOrigin, MemoryId};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalDreamMemory {
    pub id: MemoryId,
    pub cohort: String,
    /// Historical cohort admission; retained even for an archived duplicate source.
    pub birth_position: Option<u64>,
    pub credit_eligible: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoricalFirstCycleProof {
    Completed {
        roots: BTreeSet<MemoryId>,
        duplicate_origin: bool,
        evidence_digest: [u8; 32],
    },
    /// A settled marker without retained roots cannot prove no-link.
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalDreamLink {
    pub source: MemoryId,
    pub target: MemoryId,
    pub kind: GraphRelationKind,
    pub origin: GraphRelationOrigin,
    pub completed: bool,
    /// Explicit direct-publication evidence; false for duplicate-chain rewires.
    pub direct: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoricalDreamCoverage {
    ActualFirstCycle,
    VerifiedNoLink,
    CompletedGraphApproximation,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalDreamEvent {
    pub event_id: [u8; 32],
    pub source_id: MemoryId,
    pub coverage: HistoricalDreamCoverage,
    pub first_cycle_evidence_digest: Option<[u8; 32]>,
    pub propagation: HistoricalPropagationEvent,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalDreamPlan {
    pub events: Vec<HistoricalDreamEvent>,
    pub coverage: BTreeMap<MemoryId, HistoricalDreamCoverage>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoricalDreamError {
    DuplicateMemory(MemoryId),
    UnknownMemory(MemoryId),
    InvalidCohort(String),
    InvalidRepresentative(MemoryId),
}
impl std::fmt::Display for HistoricalDreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "historical Dream: {self:?}")
    }
}
impl std::error::Error for HistoricalDreamError {}

/// All inputs belong to one pinned, completed, authorized REL import. Representatives
/// are supplied by Dream's existing authority, never chosen from chain endpoints here.
/// Absent proof permits the explicit completed-direct-graph approximation; Unsupported
/// proof does not. A Completed empty root set is an authoritative no-link outcome.
pub fn plan_historical_dream_events(
    owner_uuid: [u8; 16],
    import_id: [u8; 32],
    memories: &[HistoricalDreamMemory],
    proofs: &BTreeMap<MemoryId, HistoricalFirstCycleProof>,
    links: &[HistoricalDreamLink],
    representatives: &BTreeMap<MemoryId, MemoryId>,
) -> Result<HistoricalDreamPlan, HistoricalDreamError> {
    let mut members = BTreeMap::new();
    let mut cohorts = BTreeMap::new();
    let mut cohort_members = BTreeMap::<String, BTreeSet<MemoryId>>::new();
    for memory in memories {
        cohort_members
            .entry(memory.cohort.clone())
            .or_default()
            .insert(memory.id);
        if members.insert(memory.id, memory).is_some() {
            return Err(HistoricalDreamError::DuplicateMemory(memory.id));
        }
        if memory.cohort.is_empty() || memory.birth_position == Some(0) {
            return Err(HistoricalDreamError::InvalidCohort(memory.cohort.clone()));
        }
        if let Some(old) = cohorts.insert(memory.cohort.clone(), memory.birth_position) {
            if old != memory.birth_position {
                return Err(HistoricalDreamError::InvalidCohort(memory.cohort.clone()));
            }
        }
    }
    for id in proofs
        .keys()
        .chain(links.iter().flat_map(|link| [&link.source, &link.target]))
    {
        if !members.contains_key(id) {
            return Err(HistoricalDreamError::UnknownMemory(*id));
        }
    }
    for proof in proofs.values() {
        if let HistoricalFirstCycleProof::Completed { roots, .. } = proof {
            for root in roots {
                if !members.contains_key(root) {
                    return Err(HistoricalDreamError::UnknownMemory(*root));
                }
            }
        }
    }
    for (&alias, &canonical) in representatives {
        if !members.contains_key(&alias) || !members.contains_key(&canonical) {
            return Err(HistoricalDreamError::UnknownMemory(alias));
        }
        if representatives
            .get(&canonical)
            .is_some_and(|id| *id != canonical)
        {
            return Err(HistoricalDreamError::InvalidRepresentative(alias));
        }
    }
    let mut direct_roots = BTreeMap::<MemoryId, BTreeSet<MemoryId>>::new();
    for link in links
        .iter()
        .filter(|link| link.completed && link.direct && link.origin == GraphRelationOrigin::Dream)
    {
        direct_roots
            .entry(link.source)
            .or_default()
            .insert(link.target);
        direct_roots
            .entry(link.target)
            .or_default()
            .insert(link.source);
    }
    let mut plan = HistoricalDreamPlan {
        events: Vec::new(),
        coverage: BTreeMap::new(),
    };
    for (&id, source) in &members {
        let Some(position) = source.birth_position else {
            plan.coverage
                .insert(id, HistoricalDreamCoverage::Unsupported);
            continue;
        };
        let (roots, coverage) = match proofs.get(&id) {
            Some(HistoricalFirstCycleProof::Unsupported) => {
                plan.coverage
                    .insert(id, HistoricalDreamCoverage::Unsupported);
                continue;
            }
            Some(HistoricalFirstCycleProof::Completed {
                roots,
                duplicate_origin,
                ..
            }) => {
                if !source.credit_eligible && !duplicate_origin {
                    plan.coverage
                        .insert(id, HistoricalDreamCoverage::Unsupported);
                    continue;
                }
                if roots.is_empty() {
                    plan.coverage
                        .insert(id, HistoricalDreamCoverage::VerifiedNoLink);
                    continue;
                }
                (roots.clone(), HistoricalDreamCoverage::ActualFirstCycle)
            }
            None if source.credit_eligible => {
                let roots = direct_roots.get(&id).cloned().unwrap_or_default();
                (roots, HistoricalDreamCoverage::CompletedGraphApproximation)
            }
            None => {
                plan.coverage
                    .insert(id, HistoricalDreamCoverage::Unsupported);
                continue;
            }
        };
        let excluded = cohort_members[&source.cohort].clone();
        let mut canonical_roots = BTreeMap::new();
        for root in roots {
            if !members.contains_key(&root) {
                return Err(HistoricalDreamError::UnknownMemory(root));
            }
            let representative = representatives.get(&root).copied().unwrap_or(root);
            let target = members[&representative];
            // Missing representative authority must not promote an archived alias.
            if !excluded.contains(&root)
                && !excluded.contains(&representative)
                && target.credit_eligible
                && target
                    .birth_position
                    .is_some_and(|birth| birth > 0 && birth < position)
            {
                canonical_roots.insert(
                    representative,
                    super::FreshnessPolicy::default_v1().linkage_principal,
                );
            }
        }
        plan.coverage.insert(id, coverage);
        if !canonical_roots.is_empty() {
            plan.events.push(HistoricalDreamEvent {
                event_id: historical_dream_event_id(owner_uuid, import_id, id),
                source_id: id,
                coverage,
                first_cycle_evidence_digest: match proofs.get(&id) {
                    Some(HistoricalFirstCycleProof::Completed {
                        evidence_digest, ..
                    }) => Some(*evidence_digest),
                    _ => None,
                },
                propagation: HistoricalPropagationEvent {
                    kind: HistoricalPropagationKind::FirstCycleDream,
                    position,
                    roots: canonical_roots,
                    credit_excluded: excluded,
                },
            });
        }
    }
    plan.events
        .sort_by_key(|event| (event.propagation.position, event.event_id));
    Ok(plan)
}

/// Frozen H1 typed-map encoding: identity excludes positions, roots and scores.
pub fn historical_dream_event_id(owner: [u8; 16], import: [u8; 32], source: MemoryId) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary:historical-dream:v1\0");
    hash.update([7]);
    hash.update(3u32.to_le_bytes());
    for (key, value) in [
        ("import_id", import.as_slice()),
        ("originating_memory_id", source.0.as_slice()),
        ("owner_uuid", owner.as_slice()),
    ] {
        hash.update([5]);
        hash.update((key.len() as u32).to_le_bytes());
        hash.update(key.as_bytes());
        hash.update([4]);
        hash.update((value.len() as u32).to_le_bytes());
        hash.update(value);
    }
    hash.finalize().into()
}

#[cfg(test)]
#[path = "historical_dream_tests.rs"]
mod tests;
