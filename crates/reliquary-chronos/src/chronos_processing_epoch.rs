#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessingCadence {
    interval_ns: i64,
}

impl ProcessingCadence {
    pub const fn new(interval_ns: i64) -> Option<Self> {
        if interval_ns > 0 {
            Some(Self { interval_ns })
        } else {
            None
        }
    }
}

pub fn processing_epoch(anchor_ns: i64, now_ns: i64, cadence: ProcessingCadence) -> u64 {
    let elapsed_ns = (i128::from(now_ns) - i128::from(anchor_ns)).max(0);
    let epoch = elapsed_ns / i128::from(cadence.interval_ns);

    u64::try_from(epoch).unwrap_or(u64::MAX)
}

pub fn epoch_has_advanced(
    anchor_ns: i64,
    satisfied_epoch: u64,
    now_ns: i64,
    cadence: ProcessingCadence,
) -> Option<u64> {
    let current_epoch = processing_epoch(anchor_ns, now_ns, cadence);
    (current_epoch > satisfied_epoch).then_some(current_epoch)
}

pub fn cadence_elapsed(checkpoint_ns: i64, now_ns: i64, cadence: ProcessingCadence) -> bool {
    i128::from(now_ns) - i128::from(checkpoint_ns) >= i128::from(cadence.interval_ns)
}
