//! Bounded sparse dispatcher for owner-local Freshness lifecycle crossings.
//!
//! The dispatcher is derived state. Callers rebuild it from durable records when
//! opening an owner, and refresh an entry after each accepted event that changes
//! its score or admission anchor. Draining returns the exact crossed boundary;
//! the caller settles the durable record at that boundary and refreshes it there
//! to enqueue a later crossing, even when several boundaries were missed offline.

use crate::{
    FreshnessBoundaryIndex, FreshnessError, FreshnessPolicy, FreshnessRecord, FreshnessState,
};
use reliquary_memory_core::MemoryId;

/// One lifecycle boundary removed from the due index for durable settlement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreshnessDue {
    pub boundary_turn: u64,
    pub memory: MemoryId,
    pub from_state: FreshnessState,
    pub to_state: FreshnessState,
}

/// State reversal observed while refreshing after a newly accepted event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreshnessReversal {
    pub memory: MemoryId,
    pub at_turn: u64,
    pub from_state: FreshnessState,
    pub to_state: FreshnessState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreshnessRefresh {
    pub next_boundary: Option<u64>,
    pub reversal: Option<FreshnessReversal>,
}

/// Rebuildable due index wrapper with an explicit bounded drain contract.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FreshnessDueDispatcher {
    index: FreshnessBoundaryIndex,
    initial_decay_carries: std::collections::BTreeMap<MemoryId, u64>,
    records: std::collections::BTreeMap<MemoryId, FreshnessRecord>,
    policy: FreshnessPolicy,
}

impl FreshnessDueDispatcher {
    pub fn configure_initial_decay_carries(
        &mut self,
        carries: impl IntoIterator<Item = (MemoryId, u64)>,
    ) -> Result<(), FreshnessError> {
        self.initial_decay_carries = carries
            .into_iter()
            .filter(|(_, carry)| *carry != 0)
            .collect();
        if self
            .initial_decay_carries
            .values()
            .any(|carry| *carry >= self.policy.decay_turns_per_point)
        {
            return Err(FreshnessError::InvalidScore);
        }
        for (id, record) in &self.records {
            let boundary = self.boundary_for(*id, record, record.accounted_turn.unwrap_or(0))?;
            self.index.replace_boundary(*id, boundary);
        }
        Ok(())
    }
    fn carry_for(&self, id: MemoryId, record: &FreshnessRecord) -> u64 {
        if record.accounted_turn == Some(0) {
            self.initial_decay_carries.get(&id).copied().unwrap_or(0)
        } else {
            0
        }
    }
    fn boundary_for(
        &self,
        id: MemoryId,
        record: &FreshnessRecord,
        at: u64,
    ) -> Result<Option<u64>, FreshnessError> {
        let carry = self.carry_for(id, record);
        crate::next_state_boundary_with_policy(
            record,
            at.checked_add(carry)
                .ok_or(FreshnessError::ArithmeticOverflow)?,
            &self.policy,
        )?
        .map(|value| {
            value
                .checked_sub(carry)
                .ok_or(FreshnessError::ArithmeticOverflow)
        })
        .transpose()
    }
    fn state_for(
        &self,
        id: MemoryId,
        record: &FreshnessRecord,
        at: u64,
    ) -> Result<FreshnessState, FreshnessError> {
        let mut record = *record;
        let carry = self.carry_for(id, &record);
        crate::settle_with_initial_decay_carry(&mut record, at, carry, &self.policy)?;
        record.state_at_with_policy(at, &self.policy)
    }
    /// Rebuilds all scheduled boundaries from the owner's current durable records.
    pub fn rebuild<I>(
        &mut self,
        records: I,
        as_of_turn: u64,
        policy: &FreshnessPolicy,
    ) -> Result<(), FreshnessError>
    where
        I: IntoIterator<Item = (MemoryId, FreshnessRecord)>,
    {
        let mut rebuilt = FreshnessBoundaryIndex::default();
        let mut snapshots = std::collections::BTreeMap::new();
        for (memory, record) in records {
            rebuilt.insert_with_policy(memory, &record, as_of_turn, policy)?;
            snapshots.insert(memory, record);
        }
        self.initial_decay_carries.clear();
        self.index = rebuilt;
        self.records = snapshots;
        self.policy = policy.clone();
        Ok(())
    }

    /// Replaces a Memory's scheduled crossing after admission, reinforcement,
    /// or exact-boundary settlement.
    pub fn refresh(
        &mut self,
        memory: MemoryId,
        record: &FreshnessRecord,
        as_of_turn: u64,
        policy: &FreshnessPolicy,
    ) -> Result<Option<u64>, FreshnessError> {
        if self.records.is_empty() {
            self.policy = policy.clone();
        } else if policy != &self.policy {
            return Err(FreshnessError::InvalidPolicy);
        }
        let boundary = self.boundary_for(memory, record, as_of_turn)?;
        self.index.replace_boundary(memory, boundary);
        self.records.insert(memory, *record);
        Ok(boundary)
    }

    /// Refreshes a schedule and reports a state reversal caused by the latest mutation.
    pub fn refresh_with_transition(
        &mut self,
        memory: MemoryId,
        record: &FreshnessRecord,
        accepted_turn: u64,
        policy: &FreshnessPolicy,
    ) -> Result<FreshnessRefresh, FreshnessError> {
        let prior = self.records.get(&memory).copied();
        let next_boundary = self.refresh(memory, record, accepted_turn, policy)?;
        let reversal = if let Some(previous) = prior {
            let from_state = self.state_for(memory, &previous, accepted_turn)?;
            let to_state = self.state_for(memory, record, accepted_turn)?;
            (from_state != to_state).then_some(FreshnessReversal {
                memory,
                at_turn: accepted_turn,
                from_state,
                to_state,
            })
        } else {
            None
        };
        Ok(FreshnessRefresh {
            next_boundary,
            reversal,
        })
    }

    /// Removes a Memory from the derived schedule.
    pub fn remove(&mut self, memory: MemoryId) -> bool {
        self.records.remove(&memory);
        self.index.remove(memory)
    }

    /// Removes no more than limit due entries through this accepted REL turn.
    ///
    /// Results are ordered by boundary turn and then stable Memory ID. Each
    /// entry must be settled at boundary_turn; after settlement, call refresh at
    /// that boundary to preserve subsequent missed transitions.
    pub fn drain_due(&mut self, accepted_turn: u64, limit: usize) -> Vec<FreshnessDue> {
        self.index
            .drain_through_limit(accepted_turn, limit)
            .into_iter()
            .filter_map(|(boundary_turn, memory)| {
                let record = self.records.get(&memory)?;
                let from_state = self
                    .state_for(memory, record, boundary_turn.saturating_sub(1))
                    .ok()?;
                let to_state = self.state_for(memory, record, boundary_turn).ok()?;
                Some(FreshnessDue {
                    boundary_turn,
                    memory,
                    from_state,
                    to_state,
                })
            })
            .collect()
    }

    /// Re-queues a due boundary unchanged when the notification buffer is full.
    /// Captures this affected Memory's exact missed crossings and advances only its
    /// derived snapshot. The score owner remains unchanged.
    pub fn take_due_for(
        &mut self,
        memory: MemoryId,
        through_turn: u64,
    ) -> Result<Vec<FreshnessDue>, FreshnessError> {
        let mut result = Vec::with_capacity(2);
        loop {
            let Some(record) = self.records.get(&memory).copied() else {
                break;
            };
            let boundary = self.boundary_for(
                memory,
                &record,
                through_turn.max(record.accounted_turn.unwrap_or(through_turn)),
            )?;
            let Some(boundary_turn) = boundary.filter(|boundary| *boundary <= through_turn) else {
                break;
            };
            let from_state = self.state_for(memory, &record, boundary_turn - 1)?;
            let to_state = self.state_for(memory, &record, boundary_turn)?;
            let due = FreshnessDue {
                boundary_turn,
                memory,
                from_state,
                to_state,
            };
            self.index.remove(memory);
            self.advance(due)?;
            result.push(due);
        }
        Ok(result)
    }

    pub fn defer(&mut self, due: FreshnessDue) -> Result<(), FreshnessError> {
        let record = *self
            .records
            .get(&due.memory)
            .ok_or(FreshnessError::InvalidScore)?;
        let boundary = self.boundary_for(due.memory, &record, due.boundary_turn)?;
        self.index.replace_boundary(due.memory, boundary);
        Ok(())
    }

    /// Advances the dispatcher's derived snapshot exactly to the boundary just emitted.
    /// Durable score records remain owned by FreshnessStore and are never changed here.
    pub fn advance(&mut self, due: FreshnessDue) -> Result<Option<u64>, FreshnessError> {
        let carry = self
            .records
            .get(&due.memory)
            .map(|r| self.carry_for(due.memory, r))
            .ok_or(FreshnessError::InvalidScore)?;
        let record = self
            .records
            .get_mut(&due.memory)
            .ok_or(FreshnessError::InvalidScore)?;
        crate::settle_with_initial_decay_carry(record, due.boundary_turn, carry, &self.policy)?;
        self.index
            .insert_with_policy(due.memory, record, due.boundary_turn, &self.policy)
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn admitted() -> FreshnessRecord {
        let mut record = FreshnessRecord::created();
        record.admit(0).unwrap();
        record
    }

    #[test]
    fn bounded_drains_keep_backlog_and_requeue_each_missed_crossing() {
        let policy = FreshnessPolicy::default();
        let memory = MemoryId([4; 32]);
        let record = admitted();
        let mut dispatcher = FreshnessDueDispatcher::default();
        dispatcher
            .rebuild([(memory, record)], 1_510, &policy)
            .unwrap();

        let first = dispatcher.drain_due(1_510, 1);
        assert_eq!(
            first,
            vec![FreshnessDue {
                boundary_turn: 1_000,
                memory,
                from_state: FreshnessState::Fresh,
                to_state: FreshnessState::Stale,
            }]
        );

        let mut settled = record;
        settled
            .settle_with_policy(first[0].boundary_turn, &policy)
            .unwrap();
        assert_eq!(
            dispatcher.refresh(memory, &settled, first[0].boundary_turn, &policy),
            Ok(Some(1_510))
        );
        let second = dispatcher.drain_due(1_510, 1);
        assert_eq!(
            second,
            vec![FreshnessDue {
                boundary_turn: 1_510,
                memory,
                from_state: FreshnessState::Stale,
                to_state: FreshnessState::Dormant,
            }]
        );
        settled
            .settle_with_policy(second[0].boundary_turn, &policy)
            .unwrap();
        assert_eq!(
            dispatcher.refresh(memory, &settled, second[0].boundary_turn, &policy),
            Ok(None)
        );
        assert!(dispatcher.is_empty());
    }

    #[test]
    fn reinforcement_reverses_and_reschedules_crossing() {
        let policy = FreshnessPolicy::default();
        let memory = MemoryId([5; 32]);
        let mut record = admitted();
        record.settle_with_policy(1_010, &policy).unwrap();
        let mut dispatcher = FreshnessDueDispatcher::default();
        assert_eq!(
            dispatcher.refresh(memory, &record, 1_010, &policy),
            Ok(Some(1_510))
        );

        record.reinforce_with_policy(25, 1_010, &policy).unwrap();
        assert_eq!(
            dispatcher.refresh(memory, &record, 1_010, &policy),
            Ok(Some(1_250))
        );
        assert!(dispatcher.drain_due(1_249, 100).is_empty());
        assert_eq!(
            dispatcher.drain_due(1_250, 100),
            vec![FreshnessDue {
                boundary_turn: 1_250,
                memory,
                from_state: FreshnessState::Fresh,
                to_state: FreshnessState::Stale,
            }]
        );
    }

    #[test]
    fn refresh_reports_reverse_crossing_after_reinforcement() {
        let policy = FreshnessPolicy::default();
        let memory = MemoryId([6; 32]);
        let mut record = admitted();
        record.settle_with_policy(1_510, &policy).unwrap();
        assert_eq!(record.score, -51);
        let mut dispatcher = FreshnessDueDispatcher::default();
        dispatcher
            .rebuild([(memory, record)], 1_510, &policy)
            .unwrap();

        record.reinforce_with_policy(25, 1_510, &policy).unwrap();
        let refreshed = dispatcher
            .refresh_with_transition(memory, &record, 1_510, &policy)
            .unwrap();
        assert_eq!(refreshed.next_boundary, Some(1_760));
        assert_eq!(
            refreshed.reversal,
            Some(FreshnessReversal {
                memory,
                at_turn: 1_510,
                from_state: FreshnessState::Dormant,
                to_state: FreshnessState::Stale,
            })
        );
    }

    #[test]
    fn unadmitted_records_are_not_scheduled() {
        let mut dispatcher = FreshnessDueDispatcher::default();
        dispatcher
            .rebuild(
                [(MemoryId([0; 32]), FreshnessRecord::created())],
                100,
                &FreshnessPolicy::default(),
            )
            .unwrap();
        assert!(dispatcher.is_empty());
    }
    #[test]
    fn exact_advance_uses_custom_policy_and_rebuild_is_at_least_once() {
        let policy = FreshnessPolicy {
            decay_turns_per_point: 5,
            access_principal: 30,
            linkage_principal: 60,
            local_hop_cost: 6,
            outside_hop_cost: 12,
        };
        let memory = MemoryId([17; 32]);
        let mut record = FreshnessRecord::created();
        record.admit(10).unwrap();
        let mut dispatcher = FreshnessDueDispatcher::default();
        dispatcher
            .rebuild([(memory, record)], 800, &policy)
            .unwrap();

        let first = dispatcher.drain_due(800, 1).pop().unwrap();
        assert_eq!(
            (first.boundary_turn, first.from_state, first.to_state),
            (510, FreshnessState::Fresh, FreshnessState::Stale)
        );
        assert_eq!(dispatcher.advance(first), Ok(Some(765)));
        let second = dispatcher.drain_due(800, 1).pop().unwrap();
        assert_eq!(
            (second.boundary_turn, second.from_state, second.to_state),
            (765, FreshnessState::Stale, FreshnessState::Dormant)
        );
        assert_eq!(dispatcher.advance(second), Ok(None));
        assert_eq!(record, admitted_at(10));

        let mut reopened = FreshnessDueDispatcher::default();
        reopened.rebuild([(memory, record)], 800, &policy).unwrap();
        assert_eq!(reopened.drain_due(800, 1).pop().unwrap(), first);
    }

    #[test]
    fn large_backlog_drains_in_fixed_batches_and_saturation_can_cross_again() {
        let policy = FreshnessPolicy::default();
        let memories = (0u64..1024)
            .map(|n| {
                let mut bytes = [0; 32];
                bytes[..8].copy_from_slice(&n.to_le_bytes());
                (MemoryId(bytes), admitted())
            })
            .collect::<Vec<_>>();
        let mut dispatcher = FreshnessDueDispatcher::default();
        dispatcher.rebuild(memories, 1510, &policy).unwrap();
        let mut seen = std::collections::BTreeSet::new();
        loop {
            let due = dispatcher.drain_due(1510, 64);
            assert!(due.len() <= 64);
            if due.is_empty() {
                break;
            }
            for crossing in due {
                assert!(matches!(crossing.boundary_turn, 1000 | 1510));
                assert!(seen.insert((crossing.memory, crossing.boundary_turn)));
                dispatcher.advance(crossing).unwrap();
            }
        }
        assert_eq!(seen.len(), 2048);
        assert!(dispatcher.is_empty());
        let memory = MemoryId([33; 32]);
        let mut record = admitted();
        record.settle_with_policy(2100, &policy).unwrap();
        assert_eq!(record.score, -100);
        for _ in 0..9 {
            record.reinforce_with_policy(25, 2100, &policy).unwrap();
        }
        assert_eq!(record.score, 100);
        dispatcher.refresh(memory, &record, 2100, &policy).unwrap();
        let next = dispatcher.drain_due(3100, 1).pop().unwrap();
        assert_eq!(next.boundary_turn, 3100);
        assert_eq!(dispatcher.advance(next).unwrap(), Some(3610));
    }

    #[test]
    fn rewritten_partial_decay_crosses_at_the_remaining_interval() {
        let memory = MemoryId([91; 32]);
        let policy = FreshnessPolicy::default();
        let record = FreshnessRecord {
            score: 1,
            admitted_at_turn: Some(0),
            accounted_turn: Some(0),
        };
        let mut dispatcher = FreshnessDueDispatcher::default();
        dispatcher.rebuild([(memory, record)], 0, &policy).unwrap();
        dispatcher
            .configure_initial_decay_carries([(memory, 3)])
            .unwrap();
        assert!(dispatcher.drain_due(6, 1).is_empty());
        let crossing = dispatcher.drain_due(7, 1).pop().unwrap();
        assert_eq!(crossing.boundary_turn, 7);
        assert_eq!(crossing.from_state, FreshnessState::Fresh);
        assert_eq!(crossing.to_state, FreshnessState::Stale);
        assert_eq!(dispatcher.advance(crossing).unwrap(), Some(517));
    }

    fn admitted_at(turn: u64) -> FreshnessRecord {
        let mut record = FreshnessRecord::created();
        record.admit(turn).unwrap();
        record
    }
}
