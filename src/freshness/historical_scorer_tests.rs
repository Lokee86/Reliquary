use super::*;
use crate::freshness::FreshnessRecord;

fn id(n: u8) -> MemoryId {
    MemoryId([n; 32])
}

fn admitted_zero() -> FreshnessRecord {
    let mut record = FreshnessRecord::created();
    record.admit(0).unwrap();
    record.score = 0;
    record
}

fn event(recipient: u8, position: u64, id_byte: u8, amount: i16) -> HistoricalScoreEvent {
    HistoricalScoreEvent {
        recipient: id(recipient),
        position,
        event_id: [id_byte; 32],
        amount,
    }
}

#[test]
fn chronological_signed_events_preserve_caps_and_lazy_remainder() {
    let initial = [(id(1), admitted_zero())].into_iter().collect();
    let out = score_historical_records(
        &initial,
        &[event(1, 1, 1, -100), event(1, 1, 2, 50)],
        11,
        &FreshnessPolicy::default_v1(),
    )
    .unwrap();
    assert_eq!(out[&id(1)].score, -51);
    assert_eq!(out[&id(1)].accounted_turn, Some(10));
}

#[test]
fn event_identity_breaks_same_position_ties_and_retries() {
    let initial = [(id(1), admitted_zero())].into_iter().collect();
    let a = score_historical_records(
        &initial,
        &[event(1, 1, 2, 30), event(1, 1, 1, 20)],
        1,
        &FreshnessPolicy::default_v1(),
    )
    .unwrap();
    let b = score_historical_records(
        &initial,
        &[event(1, 1, 1, 20), event(1, 1, 2, 30)],
        1,
        &FreshnessPolicy::default_v1(),
    )
    .unwrap();
    assert_eq!(a, b);
    assert_eq!(a[&id(1)].score, 50);
    assert!(matches!(
        score_historical_records(
            &initial,
            &[event(1, 1, 1, 20), event(1, 1, 1, 30)],
            1,
            &FreshnessPolicy::default_v1(),
        ),
        Err(HistoricalScoreError::DuplicateEvent(_))
    ));
}

#[test]
fn chronological_order_diverges_for_early_and_late_reinforcement() {
    let mut initial_record = admitted_zero();
    initial_record.score = 60;
    let initial = [(id(1), initial_record)].into_iter().collect();
    let early = score_historical_records(
        &initial,
        &[event(1, 1, 1, 50)],
        21,
        &FreshnessPolicy::default_v1(),
    )
    .unwrap();
    let late = score_historical_records(
        &initial,
        &[event(1, 20, 1, 50)],
        21,
        &FreshnessPolicy::default_v1(),
    )
    .unwrap();
    assert_eq!(early[&id(1)].score, 98);
    assert_eq!(late[&id(1)].score, 100);
}

#[test]
fn dormancy_can_recover_and_parallel_projection_is_byte_identical() {
    let mut initial = admitted_zero();
    initial.score = -100;
    let initial = [(id(1), initial)].into_iter().collect();
    let events = vec![
        event(1, 1, 1, 100),
        event(1, 20, 2, 100),
        event(1, 30, 3, 3),
    ];
    let out =
        score_historical_records(&initial, &events, 30, &FreshnessPolicy::default_v1()).unwrap();
    assert_eq!(out[&id(1)].score, 100);
    assert_eq!(out[&id(1)].accounted_turn, Some(30));
    let reordered = score_historical_records(
        &initial,
        &[events[2].clone(), events[0].clone(), events[1].clone()],
        30,
        &FreshnessPolicy::default_v1(),
    )
    .unwrap();
    assert_eq!(out, reordered);
}
