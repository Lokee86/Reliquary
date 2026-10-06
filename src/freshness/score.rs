use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::policy::{
    FRESHNESS_SCORE_MAX, FRESHNESS_SCORE_MIN, FreshnessPolicy, FreshnessPolicyError,
};

/// One owner's independent score and REL activity anchors for one Memory.
///
/// Event identity and durable replay receipts are owned by the persistence layer.
/// An unadmitted record is initialized at +100 but has no decay clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreshnessRecord {
    pub score: i16,
    pub admitted_at_turn: Option<u64>,
    /// Last fully accounted ten-turn decay boundary.
    pub accounted_turn: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreshnessState {
    Fresh,
    Stale,
    Dormant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreshnessError {
    ClockRegression,
    InvalidScore,
    InvalidReinforcement,
    ArithmeticOverflow,
    InvalidPolicy,
}

impl fmt::Display for FreshnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClockRegression => f.write_str("REL activity position moved backwards"),
            Self::InvalidScore => f.write_str("Freshness score or anchors are invalid"),
            Self::InvalidReinforcement => f.write_str("reinforcement must be in 1..=100"),
            Self::ArithmeticOverflow => f.write_str("Freshness activity arithmetic overflowed"),
            Self::InvalidPolicy => f.write_str("Freshness policy is invalid"),
        }
    }
}

impl std::error::Error for FreshnessError {}

fn validate_policy(policy: &FreshnessPolicy) -> Result<(), FreshnessError> {
    policy
        .validate()
        .map_err(|_: FreshnessPolicyError| FreshnessError::InvalidPolicy)
}

impl FreshnessRecord {
    /// Initializes a newly published Memory. This does not admit it to the Web.
    pub const fn created() -> Self {
        Self {
            score: 100,
            admitted_at_turn: None,
            accounted_turn: None,
        }
    }

    /// Sets the first Web-admission anchor. Retries and later promotions preserve it.
    pub fn admit(&mut self, turn: u64) -> Result<bool, FreshnessError> {
        self.validate_score()?;
        match (self.admitted_at_turn, self.accounted_turn) {
            (None, None) => {
                self.admitted_at_turn = Some(turn);
                self.accounted_turn = Some(turn);
                Ok(true)
            }
            (Some(admitted), Some(accounted)) if accounted >= admitted => Ok(false),
            _ => Err(FreshnessError::InvalidScore),
        }
    }

    /// Returns the score at an explicit owner-REL activity cut using the v1 defaults.
    pub fn score_at(&self, turn: u64) -> Result<i16, FreshnessError> {
        self.score_at_with_policy(turn, &FreshnessPolicy::default())
    }

    /// Returns the score at an explicit cut without mutation under an owner-pinned policy.
    pub fn score_at_with_policy(
        &self,
        turn: u64,
        policy: &FreshnessPolicy,
    ) -> Result<i16, FreshnessError> {
        validate_policy(policy)?;
        self.validate_score()?;
        let Some(accounted) = self.optional_accounted_turn()? else {
            return Ok(self.score);
        };
        if turn < accounted {
            return Err(FreshnessError::ClockRegression);
        }
        let steps = (turn - accounted) / policy.decay_turns_per_point;
        let value = i128::from(self.score) - i128::from(steps);
        Ok(value.max(i128::from(FRESHNESS_SCORE_MIN)) as i16)
    }

    pub fn state_at(&self, turn: u64) -> Result<FreshnessState, FreshnessError> {
        self.state_at_with_policy(turn, &FreshnessPolicy::default())
    }

    pub fn state_at_with_policy(
        &self,
        turn: u64,
        policy: &FreshnessPolicy,
    ) -> Result<FreshnessState, FreshnessError> {
        Ok(state_for(self.score_at_with_policy(turn, policy)?))
    }

    /// Materializes decay through this cut using the v1 defaults.
    pub fn settle(&mut self, turn: u64) -> Result<i16, FreshnessError> {
        self.settle_with_policy(turn, &FreshnessPolicy::default())
    }

    /// Materializes decay while conserving the partial configured interval remainder.
    pub fn settle_with_policy(
        &mut self,
        turn: u64,
        policy: &FreshnessPolicy,
    ) -> Result<i16, FreshnessError> {
        validate_policy(policy)?;
        self.validate_score()?;
        let Some(accounted) = self.optional_accounted_turn()? else {
            return Ok(self.score);
        };
        if turn < accounted {
            return Err(FreshnessError::ClockRegression);
        }
        let steps = (turn - accounted) / policy.decay_turns_per_point;
        let score = (i128::from(self.score) - i128::from(steps))
            .max(i128::from(FRESHNESS_SCORE_MIN)) as i16;
        self.score = score;
        self.accounted_turn = Some(
            accounted
                .checked_add(
                    steps
                        .checked_mul(policy.decay_turns_per_point)
                        .ok_or(FreshnessError::ArithmeticOverflow)?,
                )
                .ok_or(FreshnessError::ArithmeticOverflow)?,
        );
        Ok(score)
    }

    /// Settles decay and applies one event using the v1 defaults.
    pub fn reinforce(&mut self, amount: i16, turn: u64) -> Result<i16, FreshnessError> {
        self.reinforce_with_policy(amount, turn, &FreshnessPolicy::default())
    }

    /// Settles decay, then applies one originating-event contribution under owner policy.
    pub fn reinforce_with_policy(
        &mut self,
        amount: i16,
        turn: u64,
        policy: &FreshnessPolicy,
    ) -> Result<i16, FreshnessError> {
        validate_policy(policy)?;
        if !(1..=FRESHNESS_SCORE_MAX).contains(&amount) {
            return Err(FreshnessError::InvalidReinforcement);
        }
        self.validate_score()?;
        if self.optional_accounted_turn()?.is_none() {
            self.score = self.score.saturating_add(amount).min(FRESHNESS_SCORE_MAX);
            return Ok(self.score);
        }
        let settled = self.settle_with_policy(turn, policy)?;
        self.score = settled.saturating_add(amount).min(FRESHNESS_SCORE_MAX);
        Ok(self.score)
    }

    fn validate_score(&self) -> Result<(), FreshnessError> {
        if !(FRESHNESS_SCORE_MIN..=FRESHNESS_SCORE_MAX).contains(&self.score) {
            return Err(FreshnessError::InvalidScore);
        }
        Ok(())
    }

    fn optional_accounted_turn(&self) -> Result<Option<u64>, FreshnessError> {
        match (self.admitted_at_turn, self.accounted_turn) {
            (Some(admitted), Some(accounted)) if accounted >= admitted => Ok(Some(accounted)),
            (None, None) => Ok(None),
            _ => Err(FreshnessError::InvalidScore),
        }
    }

    fn validated_accounted_turn(&self) -> Result<Option<u64>, FreshnessError> {
        self.validate_score()?;
        self.optional_accounted_turn()
    }
}

const fn state_for(score: i16) -> FreshnessState {
    match score {
        1..=100 => FreshnessState::Fresh,
        -50..=0 => FreshnessState::Stale,
        _ => FreshnessState::Dormant,
    }
}

/// Computes the first lifecycle boundary implied by this materialized record.
/// It may be earlier than `as_of_turn`; that means the notification is already due.
/// The index is derived state and is rebuilt from owner records after opening.
pub fn next_state_boundary(
    record: &FreshnessRecord,
    as_of_turn: u64,
) -> Result<Option<u64>, FreshnessError> {
    next_state_boundary_with_policy(record, as_of_turn, &FreshnessPolicy::default())
}

pub fn next_state_boundary_with_policy(
    record: &FreshnessRecord,
    as_of_turn: u64,
    policy: &FreshnessPolicy,
) -> Result<Option<u64>, FreshnessError> {
    validate_policy(policy)?;
    let Some(accounted) = record.validated_accounted_turn()? else {
        return Ok(None);
    };
    if as_of_turn < accounted {
        return Err(FreshnessError::ClockRegression);
    }
    // Use the materialized score and its decay anchor. If the boundary is already
    // behind as_of_turn, it remains due so a sparse index rebuilt after downtime
    // can report the undrained transition before computing a later one.
    let target_steps = match state_for(record.score) {
        FreshnessState::Fresh => i64::from(record.score),
        FreshnessState::Stale => i64::from(record.score) + 51,
        FreshnessState::Dormant => return Ok(None),
    };
    if target_steps <= 0 {
        return Err(FreshnessError::InvalidScore);
    }
    let offset = u64::try_from(target_steps)
        .map_err(|_| FreshnessError::ArithmeticOverflow)?
        .checked_mul(policy.decay_turns_per_point)
        .ok_or(FreshnessError::ArithmeticOverflow)?;
    Ok(Some(
        accounted
            .checked_add(offset)
            .ok_or(FreshnessError::ArithmeticOverflow)?,
    ))
}

/// Rebuildable sparse index of each admitted Memory's next Freshness state boundary.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FreshnessBoundaryIndex {
    due: BTreeMap<u64, BTreeSet<crate::MemoryId>>,
    by_memory: BTreeMap<crate::MemoryId, u64>,
}

impl FreshnessBoundaryIndex {
    pub fn insert(
        &mut self,
        memory: crate::MemoryId,
        record: &FreshnessRecord,
        as_of_turn: u64,
    ) -> Result<Option<u64>, FreshnessError> {
        self.insert_with_policy(memory, record, as_of_turn, &FreshnessPolicy::default())
    }

    pub fn insert_with_policy(
        &mut self,
        memory: crate::MemoryId,
        record: &FreshnessRecord,
        as_of_turn: u64,
        policy: &FreshnessPolicy,
    ) -> Result<Option<u64>, FreshnessError> {
        let boundary = next_state_boundary_with_policy(record, as_of_turn, policy)?;
        self.remove(memory);
        let Some(turn) = boundary else {
            return Ok(None);
        };
        self.due.entry(turn).or_default().insert(memory);
        self.by_memory.insert(memory, turn);
        Ok(Some(turn))
    }

    pub fn remove(&mut self, memory: crate::MemoryId) -> bool {
        let Some(turn) = self.by_memory.remove(&memory) else {
            return false;
        };
        if let Some(memories) = self.due.get_mut(&turn) {
            memories.remove(&memory);
            if memories.is_empty() {
                self.due.remove(&turn);
            }
        }
        true
    }

    /// Drains at most `limit` crossed boundaries in stable turn and Memory-ID order.
    /// Remaining due entries stay indexed for a later bounded pass.
    pub fn drain_through_limit(&mut self, turn: u64, limit: usize) -> Vec<(u64, crate::MemoryId)> {
        let due: Vec<_> = self
            .due
            .range(..=turn)
            .flat_map(|(at, memories)| memories.iter().map(move |memory| (*at, *memory)))
            .take(limit)
            .collect();
        for (_, memory) in &due {
            self.remove(*memory);
        }
        due
    }

    pub fn drain_through(&mut self, turn: u64) -> Vec<(u64, crate::MemoryId)> {
        self.drain_through_limit(turn, usize::MAX)
    }

    pub fn len(&self) -> usize {
        self.by_memory.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_memory.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn admitted(turn: u64) -> FreshnessRecord {
        let mut record = FreshnessRecord::created();
        record.admit(turn).unwrap();
        record
    }

    #[test]
    fn admission_and_decay_are_owner_local_and_preserve_remainder() {
        let mut record = FreshnessRecord::created();
        assert_eq!(record.score, 100);
        assert_eq!(record.score_at(100), Ok(100));
        assert_eq!(record.state_at(100), Ok(FreshnessState::Fresh));
        assert_eq!(record.reinforce(25, 100), Ok(100));
        assert_eq!(record.admitted_at_turn, None);
        assert!(record.admit(100).unwrap());
        assert!(!record.admit(200).unwrap());
        assert_eq!(record.score_at(109).unwrap(), 100);
        assert_eq!(record.score_at(110).unwrap(), 99);
        record.reinforce(25, 112).unwrap();
        assert_eq!(record.score, 100);
        assert_eq!(record.accounted_turn, Some(110));
        assert_eq!(record.score_at(119).unwrap(), 100);
        assert_eq!(record.score_at(120).unwrap(), 99);
    }

    #[test]
    fn thresholds_match_policy_and_reinforcement_reverses_derived_state() {
        let mut record = admitted(7);
        assert_eq!(record.score_at(996).unwrap(), 2);
        assert_eq!(record.score_at(997).unwrap(), 1);
        assert_eq!(record.state_at(1_007).unwrap(), FreshnessState::Stale);
        assert_eq!(record.score_at(1_007).unwrap(), 0);
        assert_eq!(record.score_at(1_507).unwrap(), -50);
        assert_eq!(record.state_at(1_517).unwrap(), FreshnessState::Dormant);
        assert_eq!(record.score_at(1_517).unwrap(), -51);
        record.reinforce(25, 1_517).unwrap();
        assert_eq!(record.state_at(1_517).unwrap(), FreshnessState::Stale);
    }

    #[test]
    fn boundary_index_is_sparse_deterministic_and_rebuildable() {
        let a = crate::MemoryId([2; 32]);
        let b = crate::MemoryId([1; 32]);
        let record = admitted(4);
        let mut index = FreshnessBoundaryIndex::default();
        assert_eq!(index.insert(a, &record, 4).unwrap(), Some(1004));
        assert_eq!(index.insert(b, &record, 4).unwrap(), Some(1004));
        assert_eq!(
            index.drain_through(1003),
            Vec::<(u64, crate::MemoryId)>::new()
        );
        assert_eq!(index.drain_through_limit(1004, 1), vec![(1004, b)]);
        assert_eq!(index.drain_through(1004), vec![(1004, a)]);
        assert!(index.is_empty());
    }

    #[test]
    fn boundary_backlog_survives_rebuild_and_advances_one_state_at_a_time() {
        let mut record = admitted(0);
        let id = crate::MemoryId([3; 32]);
        let mut index = FreshnessBoundaryIndex::default();
        assert_eq!(index.insert(id, &record, 1_510).unwrap(), Some(1_000));
        assert_eq!(index.drain_through(1_510), vec![(1_000, id)]);
        assert_eq!(record.settle(1_000).unwrap(), 0);
        assert_eq!(index.insert(id, &record, 1_510).unwrap(), Some(1_510));
        assert_eq!(index.drain_through(1_510), vec![(1_510, id)]);
        assert_eq!(record.settle(1_510).unwrap(), -51);
        assert_eq!(index.insert(id, &record, 1_510).unwrap(), None);
    }

    #[test]
    fn nondefault_policy_changes_decay_and_due_boundaries_coherently() {
        let policy = FreshnessPolicy {
            decay_turns_per_point: 5,
            access_principal: 30,
            linkage_principal: 60,
            local_hop_cost: 6,
            outside_hop_cost: 12,
        };
        let mut record = FreshnessRecord::created();
        record.admit(10).unwrap();
        assert_eq!(record.score_at_with_policy(14, &policy), Ok(100));
        assert_eq!(record.score_at_with_policy(15, &policy), Ok(99));
        assert_eq!(
            next_state_boundary_with_policy(&record, 10, &policy),
            Ok(Some(510))
        );
        assert_eq!(record.reinforce_with_policy(30, 15, &policy), Ok(100));
        assert_eq!(record.accounted_turn, Some(15));
    }

    #[test]
    fn decay_saturation_preserves_remainder_without_overflow() {
        let mut record = admitted(u64::MAX - 5);
        record.score = -100;
        record.accounted_turn = Some(u64::MAX - 5);
        assert_eq!(record.settle(u64::MAX).unwrap(), -100);
        assert_eq!(record.accounted_turn, Some(u64::MAX - 5));
        assert_eq!(record.reinforce(25, u64::MAX).unwrap(), -75);
    }
}
