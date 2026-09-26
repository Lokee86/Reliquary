use crate::processing_epoch_model::{ProcessingEpochState, ProcessingLaneId};

#[test]
fn processing_lane_ids_are_value_ordered_and_hashable() {
    use std::collections::{BTreeSet, HashSet};

    let first = ProcessingLaneId(1);
    let second = ProcessingLaneId(2);

    assert!(first < second);
    assert_eq!(
        BTreeSet::from([second, first])
            .into_iter()
            .collect::<Vec<_>>(),
        vec![first, second]
    );
    assert_eq!(HashSet::from([first, first]).len(), 1);
}

#[test]
fn processing_epoch_state_keeps_epoch_cadence_and_checkpoint_independent() {
    let state = ProcessingEpochState {
        satisfied_through_epoch: 17,
        cadence_version: 3,
        checkpoint_at_ns: Some(-42),
    };

    assert_eq!(state.satisfied_through_epoch, 17);
    assert_eq!(state.cadence_version, 3);
    assert_eq!(state.checkpoint_at_ns, Some(-42));
}

#[test]
fn processing_epoch_state_allows_missing_checkpoint() {
    let state = ProcessingEpochState {
        satisfied_through_epoch: 0,
        cadence_version: 1,
        checkpoint_at_ns: None,
    };

    assert_eq!(state.checkpoint_at_ns, None);
}
