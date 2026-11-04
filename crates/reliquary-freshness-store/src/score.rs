use crate::policy::{
    FRESHNESS_SCORE_MAX, FRESHNESS_SCORE_MIN, FreshnessPolicy, FreshnessPolicyError,
};
use std::fmt;

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

pub fn validate_policy(policy: &FreshnessPolicy) -> Result<(), FreshnessError> {
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

    pub fn validated_accounted_turn(&self) -> Result<Option<u64>, FreshnessError> {
        self.validate_score()?;
        self.optional_accounted_turn()
    }
}

/// Apply a rewrite's inherited partial first interval without negative clock
/// anchors. Once materialized, ordinary unsigned anchors resume.
pub fn settle_with_initial_decay_carry(
    record: &mut FreshnessRecord,
    turn: u64,
    carry: u64,
    policy: &FreshnessPolicy,
) -> Result<i16, FreshnessError> {
    if carry != 0 && record.accounted_turn == Some(0) {
        let virtual_turn = turn
            .checked_add(carry)
            .ok_or(FreshnessError::ArithmeticOverflow)?;
        record.settle_with_policy(virtual_turn, policy)?;
        if let Some(accounted) = record.accounted_turn.filter(|at| *at > 0) {
            record.accounted_turn = Some(
                accounted
                    .checked_sub(carry)
                    .ok_or(FreshnessError::ArithmeticOverflow)?,
            );
        }
        Ok(record.score)
    } else {
        record.settle_with_policy(turn, policy)
    }
}

pub const fn state_for(score: i16) -> FreshnessState {
    match score {
        1..=100 => FreshnessState::Fresh,
        -50..=0 => FreshnessState::Stale,
        _ => FreshnessState::Dormant,
    }
}
