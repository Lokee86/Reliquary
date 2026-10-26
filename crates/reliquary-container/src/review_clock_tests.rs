use super::*;

fn sequence(value: u64) -> ClockReading {
    ClockReading::new(
        ClockRef::RelActivity {
            owner_uuid: [1; 16],
        },
        ClockValue::Sequence(value),
    )
    .unwrap()
}

#[test]
fn equality_and_boundaries() {
    assert!(!is_due(sequence(10), sequence(9)).unwrap());
    assert!(is_due(sequence(10), sequence(10)).unwrap());
    assert!(is_due(sequence(10), sequence(11)).unwrap());
}

#[test]
fn source_identity_and_units_are_not_interchangeable() {
    let foreign = ClockReading::new(
        ClockRef::RelActivity {
            owner_uuid: [2; 16],
        },
        ClockValue::Sequence(10),
    )
    .unwrap();
    let memory = ClockReading::new(
        ClockRef::OwnerMemoryVersion {
            owner_uuid: [1; 16],
        },
        ClockValue::Sequence(10),
    )
    .unwrap();
    for other in [foreign, memory, ClockReading::unix_ns(10)] {
        assert_eq!(is_due(sequence(10), other), Err(ClockError::SourceMismatch));
    }
    assert_eq!(
        ClockReading::new(sequence(0).source(), ClockValue::UnixNs(0)),
        Err(ClockError::UnitMismatch)
    );
    assert_eq!(
        ClockReading::new(ClockRef::UnixTimeNs, ClockValue::Sequence(0)),
        Err(ClockError::UnitMismatch)
    );
}

#[test]
fn proportional_integer_ceiling_minimum_and_cap() {
    let policy = ReviewInterval::ProportionalCapped {
        numerator: 1,
        denominator: 3,
        max_delta: 10,
    };
    for (base, next) in [(0, 1), (1, 2), (3, 4), (4, 6), (100, 110)] {
        assert_eq!(policy.next_after(sequence(base)).unwrap(), sequence(next));
    }
    // Product exceeds u64 but is safely capped before conversion.
    let large = ReviewInterval::ProportionalCapped {
        numerator: u64::MAX,
        denominator: u64::MAX,
        max_delta: 1,
    };
    assert_eq!(
        large.next_after(sequence(u64::MAX - 1)).unwrap(),
        sequence(u64::MAX)
    );
}

#[test]
fn invalid_policies_and_overflow_are_rejected() {
    for policy in [
        ReviewInterval::Fixed(ClockValue::Sequence(0)),
        ReviewInterval::ProportionalCapped {
            numerator: 0,
            denominator: 1,
            max_delta: 1,
        },
        ReviewInterval::ProportionalCapped {
            numerator: 1,
            denominator: 0,
            max_delta: 1,
        },
        ReviewInterval::ProportionalCapped {
            numerator: 1,
            denominator: 1,
            max_delta: 0,
        },
    ] {
        assert_eq!(
            policy.next_after(sequence(0)),
            Err(ClockError::InvalidInterval)
        );
    }
    for delta in [0, -1, i64::MIN] {
        assert_eq!(
            ReviewInterval::Fixed(ClockValue::UnixNs(delta)).next_after(ClockReading::unix_ns(0)),
            Err(ClockError::InvalidInterval)
        );
    }
    assert_eq!(
        ReviewInterval::Fixed(ClockValue::Sequence(1)).next_after(sequence(u64::MAX)),
        Err(ClockError::Overflow)
    );
    assert_eq!(
        ReviewInterval::Fixed(ClockValue::UnixNs(1)).next_after(ClockReading::unix_ns(i64::MAX)),
        Err(ClockError::Overflow)
    );
    assert_eq!(
        ReviewInterval::Fixed(ClockValue::Sequence(1)).next_after(ClockReading::unix_ns(0)),
        Err(ClockError::UnitMismatch)
    );
    assert_eq!(
        ReviewInterval::ProportionalCapped {
            numerator: 1,
            denominator: 1,
            max_delta: 1
        }
        .next_after(ClockReading::unix_ns(0)),
        Err(ClockError::UnsupportedPolicy)
    );
}

#[test]
fn signed_time_and_backward_samples_leave_threshold_intact() {
    let policy = ReviewInterval::Fixed(ClockValue::UnixNs(i64::MAX));
    let threshold = policy.next_after(ClockReading::unix_ns(i64::MIN)).unwrap();
    assert_eq!(threshold, ClockReading::unix_ns(-1));
    assert!(is_due(threshold, ClockReading::unix_ns(0)).unwrap());
    assert!(!is_due(threshold, ClockReading::unix_ns(-2)).unwrap());
    assert_eq!(threshold.value(), ClockValue::UnixNs(-1));
    assert!(is_due(threshold, ClockReading::unix_ns(-1)).unwrap());
}

#[test]
fn anchor_validation_and_explicit_bootstrap() {
    let policy = ReviewInterval::Fixed(ClockValue::Sequence(250));
    assert_eq!(
        threshold_from_anchor(policy, sequence(1500), sequence(1600)).unwrap(),
        sequence(1750)
    );
    assert_eq!(
        threshold_from_anchor(policy, sequence(1601), sequence(1600)),
        Err(ClockError::FutureAnchor)
    );
    let threshold = due_now(policy, sequence(1600)).unwrap();
    assert!(is_due(threshold, sequence(1600)).unwrap());
    assert_eq!(
        due_now(ReviewInterval::Fixed(ClockValue::Sequence(0)), sequence(0)),
        Err(ClockError::InvalidInterval)
    );
}
