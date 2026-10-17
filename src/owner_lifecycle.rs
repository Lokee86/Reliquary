//! Shared owner-local lifecycle vocabulary. Stores retain mutation and clock authority.
//! No state, persistence, clock allocation or provenance traversal lives here.

use crate::{CompatibilityProfileId, EgoAnchorId, EgoIdentityId, EntityId, MemoryId, RelationshipId};

/// An identity within the owner on which a lifecycle operation is invoked.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum LifecycleTarget {
    Conversation(String),
    Memory(MemoryId),
    Entity(EntityId),
    Relationship(RelationshipId),
    CompatibilityProfile(CompatibilityProfileId),
    EgoIdentity(EgoIdentityId),
    EgoAnchor(EgoAnchorId),
    EgoPersonality,
    EgoWebSynthesis,
}

/// A persisted domain tombstone's existing publication coordinates.
/// Clock-neutral owners omit coordinates they do not possess.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TombstoneCoordinates {
    pub global_version: Option<u64>,
    pub domain_version: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleState {
    Active,
    Tombstoned(TombstoneCoordinates),
}

/// Cascade targets are supplied explicitly; this never authorizes provenance walking.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum TombstonePolicy {
    #[default]
    DirectOnly,
    ExplicitCascade(Vec<LifecycleTarget>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadUnavailable {
    Pruned,
}

/// Payload availability is independent of lifecycle. A tombstone need not destroy bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoricalPayload<T, S> {
    Available(T),
    Tombstoned(T),
    Pruned {
        structural_history: S,
        reason: PayloadUnavailable,
    },
}

/// A repeated delete returns the original coordinates without allocating another ticket.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TombstoneOutcome {
    Published(TombstoneCoordinates),
    AlreadyTombstoned(TombstoneCoordinates),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LifecycleError {
    UnknownTarget(LifecycleTarget),
    TerminalTombstone(LifecycleTarget),
    ConflictingMutation(LifecycleTarget),
    PayloadRequiredByLiveTarget(LifecycleTarget),
    UnsupportedTarget(LifecycleTarget),
}

impl LifecycleState {
    /// Owners call this before an ordinary update or re-admission of an existing identity.
    pub fn require_active(self, target: LifecycleTarget) -> Result<(), LifecycleError> {
        match self {
            Self::Active => Ok(()),
            Self::Tombstoned(_) => Err(LifecycleError::TerminalTombstone(target)),
        }
    }

    /// Inspect idempotency before any owner clock allocation.
    pub fn existing_tombstone(self) -> Option<TombstoneOutcome> {
        match self {
            Self::Active => None,
            Self::Tombstoned(at) => Some(TombstoneOutcome::AlreadyTombstoned(at)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_identity_cannot_be_updated_and_delete_retry_keeps_original_clocks() {
        let at = TombstoneCoordinates {
            global_version: Some(17),
            domain_version: Some(8),
        };
        let state = LifecycleState::Tombstoned(at);
        let target = LifecycleTarget::Conversation("conversation".into());
        assert_eq!(
            state.require_active(target.clone()),
            Err(LifecycleError::TerminalTombstone(target))
        );
        assert_eq!(
            state.existing_tombstone(),
            Some(TombstoneOutcome::AlreadyTombstoned(at))
        );
    }

    #[test]
    fn active_state_allows_update_and_direct_only_is_default() {
        assert_eq!(
            LifecycleState::Active.require_active(LifecycleTarget::EgoPersonality),
            Ok(())
        );
        assert_eq!(LifecycleState::Active.existing_tombstone(), None);
        assert_eq!(TombstonePolicy::default(), TombstonePolicy::DirectOnly);
    }

    #[test]
    fn tombstone_payload_and_pruned_structure_are_distinct() {
        let available: HistoricalPayload<&str, &str> = HistoricalPayload::Tombstoned("body");
        let pruned: HistoricalPayload<&str, &str> = HistoricalPayload::Pruned {
            structural_history: "identity and ancestry",
            reason: PayloadUnavailable::Pruned,
        };
        assert_ne!(available, pruned);
    }
}
