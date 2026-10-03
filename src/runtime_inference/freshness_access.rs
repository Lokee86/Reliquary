use crate::{Cva, FreshnessState, MemoryId, PropagationEvent};
use std::collections::{BTreeMap, BTreeSet};

/// Explicit proof that Memory content was accepted into a real context/use.
/// Search results and low-level reads never construct this receipt themselves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedMemoryUseReceipt {
    pub owner_uuid: [u8; 16],
    pub accepted_turn: u64,
    pub use_id: String,
    pub memories: Vec<MemoryId>,
}

/// Graded REL-local signal for Ego context selection. Consumers combine it with
/// coverage and hard-keep lanes; it does not filter the Memory Web.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EgoMemoryFreshness {
    pub memory_id: MemoryId,
    pub score: i16,
    pub state: FreshnessState,
}

/// An owner-local routine-context candidate and its already-computed token cost.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EgoFreshnessCandidate {
    pub memory_id: MemoryId,
    pub context_cost: usize,
}

impl Cva {
    /// Apply one accepted-use event per distinct Memory in a delivery. The
    /// caller invokes this only after content has actually been consumed.
    pub fn record_accepted_memory_use(
        &mut self,
        receipt: AcceptedMemoryUseReceipt,
        workers: usize,
    ) -> Result<usize, crate::CvaError> {
        if receipt.use_id.trim().is_empty() {
            return Err(crate::CvaError::Freshness(
                "accepted-use receipt has an empty identity".into(),
            ));
        }
        if self.owner_uuid() != Some(receipt.owner_uuid) {
            return Err(crate::CvaError::Freshness(
                "accepted-use receipt belongs to a different REL owner".into(),
            ));
        }
        if receipt.accepted_turn > self.rel_turn_count() {
            return Err(crate::CvaError::Freshness(
                "accepted-use receipt refers to a future REL turn".into(),
            ));
        }

        let memories: BTreeSet<_> = receipt.memories.into_iter().collect();
        let memories = memories.into_iter().collect::<Vec<_>>();
        if self
            .freshness
            .resolve_accepted_use(&receipt.use_id, receipt.accepted_turn, &memories)
            .map_err(crate::CvaError::Freshness)?
            .is_some()
        {
            return Ok(0);
        }
        if memories.is_empty() {
            return Ok(0);
        }
        let mut adjacency = BTreeMap::<MemoryId, BTreeSet<MemoryId>>::new();
        for relation in self
            .graph_relations()
            .into_iter()
            .filter(|relation| relation.active)
        {
            adjacency
                .entry(relation.source)
                .or_default()
                .insert(relation.target);
            adjacency
                .entry(relation.target)
                .or_default()
                .insert(relation.source);
        }
        let graph_version = self.memory_graph_version();
        let community_snapshot = self
            .community_snapshot()
            .filter(|snapshot| snapshot.derived_graph_version == graph_version);
        let graph_proof = crate::FreshnessEventProof {
            graph_version,
            community_generation: community_snapshot
                .as_ref()
                .map(|snapshot| snapshot.generation),
            community_graph_version: community_snapshot
                .as_ref()
                .map(|snapshot| snapshot.derived_graph_version),
        };
        let policy = self.freshness_policy();
        let memberships = community_snapshot
            .map(|snapshot| {
                snapshot
                    .communities
                    .into_iter()
                    .flat_map(|community| {
                        community
                            .members
                            .into_iter()
                            .map(move |memory| (memory, community.id))
                    })
                    .collect::<BTreeMap<MemoryId, crate::CommunityId>>()
            })
            .unwrap_or_default();

        for memory_id in &memories {
            if self.freshness_record(*memory_id).is_none() {
                return Err(crate::CvaError::Freshness(format!(
                    "Memory {:?} has no Freshness record",
                    memory_id
                )));
            }
        }
        let events = memories
            .iter()
            .map(|memory_id| PropagationEvent {
                roots: vec![*memory_id],
                principal: policy.access_principal,
                excluded: BTreeSet::new(),
            })
            .collect::<Vec<_>>();
        let outcomes = crate::freshness::propagate_events_with_policy(
            &events,
            &adjacency,
            &memberships,
            workers.max(1),
            &policy,
        )
        .map_err(|error| crate::CvaError::Freshness(error.to_string()))?;
        if !self.freshness_event_proof_is_available(graph_proof) {
            return Err(crate::CvaError::Freshness(
                "accepted-use snapshot proof is unavailable".into(),
            ));
        }
        let affected = outcomes
            .iter()
            .flat_map(|outcome| outcome.candidates.keys().copied())
            .collect::<BTreeSet<_>>();
        let applied = memories.len();
        let payload = self
            .freshness
            .prepare_accepted_use_batch(
                receipt.owner_uuid,
                receipt.use_id,
                receipt.accepted_turn,
                memories,
                graph_proof,
                outcomes
                    .into_iter()
                    .map(|outcome| outcome.candidates)
                    .collect(),
            )
            .map_err(crate::CvaError::Freshness)?;
        self.freshness
            .persist_policy_if_needed(&mut self.container, receipt.owner_uuid)
            .map_err(crate::CvaError::Freshness)?;
        self.container.append(&payload)?;
        self.container.sync()?;
        self.freshness
            .ingest_appended(&payload)
            .map_err(crate::CvaError::Freshness)?;
        self.freshness_refresh_due(affected, receipt.accepted_turn)?;
        Ok(applied)
    }

    /// Return current signed Freshness grades for Ego without changing the
    /// owner or excluding low-scoring, archived, or hard-kept Memories.
    pub fn ego_memory_freshness(
        &self,
        memory_ids: &[MemoryId],
        as_of_turn: u64,
    ) -> Result<Vec<EgoMemoryFreshness>, crate::CvaError> {
        if as_of_turn > self.rel_turn_count() {
            return Err(crate::CvaError::Freshness(
                "Ego Freshness cut is beyond the REL activity head".into(),
            ));
        }
        let policy = self.freshness_policy();
        let mut output = Vec::new();
        let mut seen = BTreeSet::new();
        for memory_id in memory_ids {
            if !seen.insert(*memory_id) {
                continue;
            }
            let record = if as_of_turn == self.rel_turn_count() {
                self.freshness_record(*memory_id)
            } else {
                self.freshness_record_at(*memory_id, as_of_turn)?
            };
            let Some(record) = record else {
                continue;
            };
            output.push(EgoMemoryFreshness {
                memory_id: *memory_id,
                score: record
                    .score_at_with_policy(as_of_turn, &policy)
                    .map_err(|error| crate::CvaError::Freshness(error.to_string()))?,
                state: record
                    .state_at_with_policy(as_of_turn, &policy)
                    .map_err(|error| crate::CvaError::Freshness(error.to_string()))?,
            });
        }
        Ok(output)
    }

    /// Choose routine context under a token budget. Hard-keeps survive even if
    /// they exceed budget; all candidates are retained when the full Web fits.
    pub fn select_ego_routine_context(
        &self,
        candidates: &[EgoFreshnessCandidate],
        budget: usize,
        hard_keep: &BTreeSet<MemoryId>,
        as_of_turn: u64,
    ) -> Result<Vec<MemoryId>, crate::CvaError> {
        if as_of_turn > self.rel_turn_count() {
            return Err(crate::CvaError::Freshness(
                "Ego Freshness cut is beyond the REL activity head".into(),
            ));
        }
        let mut unique = BTreeMap::<MemoryId, usize>::new();
        for candidate in candidates {
            unique
                .entry(candidate.memory_id)
                .and_modify(|cost| *cost = (*cost).min(candidate.context_cost))
                .or_insert(candidate.context_cost);
        }
        let total = unique
            .values()
            .try_fold(0usize, |sum, cost| sum.checked_add(*cost));
        if total.is_some_and(|total| total <= budget) {
            let mut all = hard_keep.clone();
            all.extend(unique.keys().copied());
            return Ok(all.into_iter().collect());
        }

        let mut selected = hard_keep.clone();
        let mut ordered = hard_keep.iter().copied().collect::<Vec<_>>();
        let spent = selected
            .iter()
            .filter_map(|id| unique.get(id))
            .try_fold(0usize, |sum, cost| sum.checked_add(*cost))
            .unwrap_or(usize::MAX);
        let graded =
            self.ego_memory_freshness(&unique.keys().copied().collect::<Vec<_>>(), as_of_turn)?;
        let grades = graded
            .into_iter()
            .map(|grade| (grade.memory_id, grade.score))
            .collect::<BTreeMap<_, _>>();
        let mut ranked = unique.keys().copied().collect::<Vec<_>>();
        // A legacy row without a durable grade remains eligible at neutral score;
        // this selection seam never enrolls it or infers a lifecycle state.
        ranked.sort_by(|a, b| {
            grades
                .get(b)
                .copied()
                .unwrap_or(0)
                .cmp(&grades.get(a).copied().unwrap_or(0))
                .then_with(|| a.cmp(b))
        });
        let mut remaining = budget.saturating_sub(spent);
        for memory_id in ranked {
            if selected.contains(&memory_id) {
                continue;
            }
            let cost = unique[&memory_id];
            if cost <= remaining {
                selected.insert(memory_id);
                ordered.push(memory_id);
                remaining -= cost;
            }
        }
        Ok(ordered)
    }
}
