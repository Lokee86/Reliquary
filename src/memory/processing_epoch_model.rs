#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ProcessingLaneId(pub(crate) u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProcessingEpochState {
    pub(crate) satisfied_through_epoch: u64,
    pub(crate) cadence_version: u32,
    pub(crate) checkpoint_at_ns: Option<i64>,
}
