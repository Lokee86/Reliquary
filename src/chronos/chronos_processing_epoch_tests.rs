use crate::chronos_processing_epoch::{
    ProcessingCadence, cadence_elapsed, epoch_has_advanced, processing_epoch,
};

fn cadence(interval_ns: i64) -> ProcessingCadence {
    ProcessingCadence::new(interval_ns).unwrap()
}

#[test]
fn processing_cadence_requires_a_positive_interval() {
    assert!(ProcessingCadence::new(1).is_some());
    assert_eq!(ProcessingCadence::new(0), None);
    assert_eq!(ProcessingCadence::new(-1), None);
}

#[test]
fn processing_epoch_clamps_before_anchor_and_starts_at_zero() {
    let cadence = cadence(10);

    assert_eq!(processing_epoch(100, 99, cadence), 0);
    assert_eq!(processing_epoch(100, 100, cadence), 0);
}

#[test]
fn processing_epoch_changes_only_at_cadence_boundaries() {
    let cadence = cadence(10);

    assert_eq!(processing_epoch(100, 109, cadence), 0);
    assert_eq!(processing_epoch(100, 110, cadence), 1);
    assert_eq!(processing_epoch(100, 159, cadence), 5);
    assert_eq!(processing_epoch(100, 160, cadence), 6);
}

#[test]
fn processing_epoch_handles_full_timestamp_range_without_overflow() {
    assert_eq!(processing_epoch(i64::MIN, i64::MAX, cadence(1)), u64::MAX);
}

#[test]
fn epoch_has_advanced_returns_only_the_current_epoch() {
    let cadence = cadence(10);

    assert_eq!(epoch_has_advanced(100, 0, 109, cadence), None);
    assert_eq!(epoch_has_advanced(100, 0, 110, cadence), Some(1));
    assert_eq!(epoch_has_advanced(100, 1, 160, cadence), Some(6));
    assert_eq!(epoch_has_advanced(100, 6, 160, cadence), None);
}

#[test]
fn cadence_elapsed_changes_at_the_exact_boundary() {
    let cadence = cadence(10);

    assert!(!cadence_elapsed(100, 99, cadence));
    assert!(!cadence_elapsed(100, 109, cadence));
    assert!(cadence_elapsed(100, 110, cadence));
}

#[test]
fn cadence_elapsed_handles_extreme_timestamps_without_overflow() {
    assert!(cadence_elapsed(i64::MIN, i64::MAX, cadence(i64::MAX)));
    assert!(!cadence_elapsed(i64::MAX, i64::MIN, cadence(1)));
}
