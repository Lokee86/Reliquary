use crate::{EgoAnchorId, EgoIdentityId};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LifecycleTarget {
    EgoIdentity(EgoIdentityId),
    EgoAnchor(EgoAnchorId),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LifecycleError {
    TerminalTombstone(LifecycleTarget),
}
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
impl LifecycleState {
    pub fn require_active(self, target: LifecycleTarget) -> Result<(), LifecycleError> {
        match self {
            Self::Active => Ok(()),
            Self::Tombstoned(_) => Err(LifecycleError::TerminalTombstone(target)),
        }
    }
}
