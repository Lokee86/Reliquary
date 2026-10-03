//! Owner-validated readings for the pure generic review evaluator.
#[cfg(test)]
#[path = "review_clocks_tests.rs"]
mod tests;
use crate::staleness::{ClockError, ClockReading, ClockRef, ClockValue};
use crate::{Cva, Phylactery};

impl Cva {
    /// Read this REL's authoritative activity or Memory clock.
    /// Unix time is supplied separately as a trusted caller sample.
    pub fn review_clock(&self, source: ClockRef) -> Result<ClockReading, ClockError> {
        if source == ClockRef::UnixTimeNs {
            return Err(ClockError::SourceMismatch);
        }
        let owner_uuid = self.owner_uuid().ok_or(ClockError::MissingOwner)?;
        let value = match source {
            ClockRef::RelActivity { owner_uuid: id } if id == owner_uuid => self.rel_turn_count(),
            ClockRef::OwnerMemoryVersion { owner_uuid: id } if id == owner_uuid => {
                self.memory_version()
            }
            _ => return Err(ClockError::SourceMismatch),
        };
        ClockReading::new(source, ClockValue::Sequence(value))
    }
}

impl Phylactery {
    /// Read this PHY's existing Memory-version clock; REL activity is unavailable.
    pub fn review_clock(&self, source: ClockRef) -> Result<ClockReading, ClockError> {
        let owner_uuid = self.owner_uuid().ok_or(ClockError::MissingOwner)?;
        match source {
            ClockRef::OwnerMemoryVersion { owner_uuid: id } if id == owner_uuid => {
                ClockReading::new(source, ClockValue::Sequence(self.memory_version()))
            }
            _ => Err(ClockError::SourceMismatch),
        }
    }
}
