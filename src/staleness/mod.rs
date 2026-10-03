//! Typed, owner-qualified clocks and pure review-threshold arithmetic.
//! Persistence, semantic eligibility and review completion belong to their owners.
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClockRef {
    RelActivity { owner_uuid: [u8; 16] },
    OwnerMemoryVersion { owner_uuid: [u8; 16] },
    UnixTimeNs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockValue {
    Sequence(u64),
    UnixNs(i64),
}

/// A value bound to one source; construction rejects incompatible units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockReading {
    source: ClockRef,
    value: ClockValue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockError {
    MissingOwner,
    SourceMismatch,
    UnitMismatch,
    InvalidInterval,
    UnsupportedPolicy,
    Overflow,
    FutureAnchor,
}

impl fmt::Display for ClockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ClockError {}

impl ClockReading {
    pub fn new(source: ClockRef, value: ClockValue) -> Result<Self, ClockError> {
        match (source, value) {
            (ClockRef::UnixTimeNs, ClockValue::UnixNs(_))
            | (ClockRef::RelActivity { .. }, ClockValue::Sequence(_))
            | (ClockRef::OwnerMemoryVersion { .. }, ClockValue::Sequence(_)) => {
                Ok(Self { source, value })
            }
            _ => Err(ClockError::UnitMismatch),
        }
    }

    pub fn source(self) -> ClockRef {
        self.source
    }
    pub fn value(self) -> ClockValue {
        self.value
    }

    /// Trusted caller sample. This module never reads a hidden wall clock.
    pub fn unix_ns(sample: i64) -> Self {
        Self {
            source: ClockRef::UnixTimeNs,
            value: ClockValue::UnixNs(sample),
        }
    }

    fn compare(self, other: Self) -> Result<std::cmp::Ordering, ClockError> {
        if self.source != other.source {
            return Err(ClockError::SourceMismatch);
        }
        match (self.value, other.value) {
            (ClockValue::Sequence(a), ClockValue::Sequence(b)) => Ok(a.cmp(&b)),
            (ClockValue::UnixNs(a), ClockValue::UnixNs(b)) => Ok(a.cmp(&b)),
            _ => Err(ClockError::UnitMismatch),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewInterval {
    Fixed(ClockValue),
    ProportionalCapped {
        numerator: u64,
        denominator: u64,
        max_delta: u64,
    },
}

impl ReviewInterval {
    pub fn validate(self, source: ClockRef) -> Result<(), ClockError> {
        match (self, source) {
            (Self::Fixed(ClockValue::UnixNs(delta)), ClockRef::UnixTimeNs) if delta > 0 => Ok(()),
            (
                Self::Fixed(ClockValue::Sequence(delta)),
                ClockRef::RelActivity { .. } | ClockRef::OwnerMemoryVersion { .. },
            ) if delta > 0 => Ok(()),
            (Self::ProportionalCapped { .. }, ClockRef::UnixTimeNs) => {
                Err(ClockError::UnsupportedPolicy)
            }
            (
                Self::ProportionalCapped {
                    numerator,
                    denominator,
                    max_delta,
                },
                _,
            ) if numerator > 0 && denominator > 0 && max_delta > 0 => Ok(()),
            (
                Self::Fixed(ClockValue::UnixNs(_)),
                ClockRef::RelActivity { .. } | ClockRef::OwnerMemoryVersion { .. },
            )
            | (Self::Fixed(ClockValue::Sequence(_)), ClockRef::UnixTimeNs) => {
                Err(ClockError::UnitMismatch)
            }
            _ => Err(ClockError::InvalidInterval),
        }
    }

    /// Checked absolute threshold, anchored to covered progress rather than finish time.
    pub fn next_after(self, anchor: ClockReading) -> Result<ClockReading, ClockError> {
        self.validate(anchor.source)?;
        let value = match (self, anchor.value) {
            (Self::Fixed(ClockValue::Sequence(delta)), ClockValue::Sequence(base)) => {
                ClockValue::Sequence(base.checked_add(delta).ok_or(ClockError::Overflow)?)
            }
            (Self::Fixed(ClockValue::UnixNs(delta)), ClockValue::UnixNs(base)) => {
                ClockValue::UnixNs(
                    i64::try_from(i128::from(base) + i128::from(delta))
                        .map_err(|_| ClockError::Overflow)?,
                )
            }
            (
                Self::ProportionalCapped {
                    numerator,
                    denominator,
                    max_delta,
                },
                ClockValue::Sequence(base),
            ) => {
                let product = u128::from(base) * u128::from(numerator);
                let divisor = u128::from(denominator);
                let ceiling = product / divisor + u128::from(product % divisor != 0);
                let delta = u64::try_from(ceiling.max(1).min(u128::from(max_delta)))
                    .map_err(|_| ClockError::Overflow)?;
                ClockValue::Sequence(base.checked_add(delta).ok_or(ClockError::Overflow)?)
            }
            _ => return Err(ClockError::UnitMismatch),
        };
        ClockReading::new(anchor.source, value)
    }
}

/// Pure comparison: no mutation, inference, queueing or reinforcement.
pub fn is_due(threshold: ClockReading, current: ClockReading) -> Result<bool, ClockError> {
    Ok(current.compare(threshold)? != std::cmp::Ordering::Less)
}

/// Reject a caller's future anchor before creating a threshold.
pub fn threshold_from_anchor(
    policy: ReviewInterval,
    anchor: ClockReading,
    current: ClockReading,
) -> Result<ClockReading, ClockError> {
    if anchor.compare(current)? == std::cmp::Ordering::Greater {
        return Err(ClockError::FutureAnchor);
    }
    policy.next_after(anchor)
}

/// Explicit consumer bootstrap; validates policy but does not invent eligibility.
pub fn due_now(policy: ReviewInterval, current: ClockReading) -> Result<ClockReading, ClockError> {
    policy.validate(current.source)?;
    Ok(current)
}

#[cfg(test)]
mod tests;
