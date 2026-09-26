use crate::memory_store::MemoryStore;
use crate::processing_epoch_model::{ProcessingEpochState, ProcessingLaneId};
use crate::processing_epoch_store::ProcessingEpochStore;
use crate::{Container, MemoryError, MemoryId};
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "continuity-processing-epoch-store-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn memory_id(byte: u8) -> MemoryId {
    MemoryId([byte; 32])
}

fn state(epoch: u64, cadence_version: u32, checkpoint_at_ns: Option<i64>) -> ProcessingEpochState {
    ProcessingEpochState {
        satisfied_through_epoch: epoch,
        cadence_version,
        checkpoint_at_ns,
    }
}

fn payload(memory_id: MemoryId, lane_id: ProcessingLaneId, value: ProcessingEpochState) -> Vec<u8> {
    let mut payload = vec![0_u8; 63];
    payload[..8].copy_from_slice(b"CVAPEP01");
    payload[8..40].copy_from_slice(&memory_id.0);
    payload[40..42].copy_from_slice(&lane_id.0.to_le_bytes());
    payload[42..50].copy_from_slice(&value.satisfied_through_epoch.to_le_bytes());
    payload[50..54].copy_from_slice(&value.cadence_version.to_le_bytes());
    if let Some(checkpoint_at_ns) = value.checkpoint_at_ns {
        payload[54] = 1;
        payload[55..63].copy_from_slice(&checkpoint_at_ns.to_le_bytes());
    }
    payload
}

#[test]
fn processing_epoch_store_ignores_unrelated_payloads() {
    let mut store = ProcessingEpochStore::default();

    store.ingest(b"not-a-processing-epoch").unwrap();

    assert!(store.records().is_empty());
}

#[test]
fn processing_epoch_store_rejects_malformed_current_records() {
    let mut store = ProcessingEpochStore::default();
    let mut malformed = b"CVAPEP01".to_vec();
    malformed.extend_from_slice(&[0_u8; 8]);

    assert!(matches!(
        store.ingest(&malformed),
        Err(MemoryError::CorruptRecord(
            "invalid processing epoch record"
        ))
    ));

    let mut invalid_checkpoint = payload(memory_id(1), ProcessingLaneId(1), state(2, 1, None));
    invalid_checkpoint[54] = 2;
    assert!(matches!(
        store.ingest(&invalid_checkpoint),
        Err(MemoryError::CorruptRecord(
            "invalid processing epoch checkpoint"
        ))
    ));
}

#[test]
fn processing_epoch_store_merges_same_cadence_independently() {
    let id = memory_id(2);
    let lane = ProcessingLaneId(7);
    let mut store = ProcessingEpochStore::default();

    store
        .ingest(&payload(id, lane, state(5, 3, Some(100))))
        .unwrap();
    store
        .ingest(&payload(id, lane, state(3, 3, Some(200))))
        .unwrap();

    assert_eq!(store.state(id, lane), Some(state(5, 3, Some(200))));
}

#[test]
fn processing_epoch_store_rejects_cadence_version_conflicts() {
    let id = memory_id(3);
    let lane = ProcessingLaneId(1);
    let mut store = ProcessingEpochStore::default();

    store
        .ingest(&payload(id, lane, state(1, 1, Some(10))))
        .unwrap();

    assert!(matches!(
        store.ingest(&payload(id, lane, state(2, 2, Some(20)))),
        Err(MemoryError::CorruptRecord(
            "processing epoch cadence version conflict"
        ))
    ));
}

#[test]
fn processing_epoch_store_keeps_lanes_independent() {
    let id = memory_id(4);
    let first_lane = ProcessingLaneId(1);
    let second_lane = ProcessingLaneId(2);
    let mut store = ProcessingEpochStore::default();

    store
        .ingest(&payload(id, first_lane, state(4, 1, Some(40))))
        .unwrap();
    store
        .ingest(&payload(id, second_lane, state(9, 5, Some(90))))
        .unwrap();

    assert_eq!(store.state(id, first_lane), Some(state(4, 1, Some(40))));
    assert_eq!(store.state(id, second_lane), Some(state(9, 5, Some(90))));
}

#[test]
fn processing_epoch_records_are_sorted_by_memory_then_lane() {
    let mut store = ProcessingEpochStore::default();
    for (id, lane) in [
        (memory_id(2), ProcessingLaneId(2)),
        (memory_id(1), ProcessingLaneId(3)),
        (memory_id(1), ProcessingLaneId(1)),
    ] {
        store.ingest(&payload(id, lane, state(1, 1, None))).unwrap();
    }

    let keys: Vec<_> = store
        .records()
        .into_iter()
        .map(|(id, lane, _)| (id, lane))
        .collect();
    assert_eq!(
        keys,
        vec![
            (memory_id(1), ProcessingLaneId(1)),
            (memory_id(1), ProcessingLaneId(3)),
            (memory_id(2), ProcessingLaneId(2)),
        ]
    );
}

#[test]
fn processing_epoch_store_validates_memory_references() {
    let mut store = ProcessingEpochStore::default();
    store
        .ingest(&payload(
            memory_id(5),
            ProcessingLaneId(1),
            state(1, 1, None),
        ))
        .unwrap();

    assert!(matches!(
        store.validate(&MemoryStore::empty()),
        Err(MemoryError::CorruptRecord(
            "processing epoch references a missing Memory"
        ))
    ));
}

#[test]
fn processing_epoch_put_persists_current_format_and_is_idempotent() {
    let path = test_path("processing-epochs.cva");
    let id = memory_id(6);
    let lane = ProcessingLaneId(11);
    let mut container = Container::create(&path).unwrap();
    let mut store = ProcessingEpochStore::default();

    assert!(
        store
            .put(&mut container, id, lane, state(2, 4, Some(20)))
            .unwrap()
    );
    assert!(
        store
            .put(&mut container, id, lane, state(5, 4, Some(10)))
            .unwrap()
    );
    assert_eq!(store.state(id, lane), Some(state(5, 4, Some(20))));
    assert!(
        !store
            .put(&mut container, id, lane, state(3, 4, Some(15)))
            .unwrap()
    );
    drop(container);

    let mut reopened = ProcessingEpochStore::default();
    let _container: Container =
        Container::open_scanned(&path, |_, bytes, _| reopened.ingest(bytes)).unwrap();
    assert_eq!(reopened.state(id, lane), Some(state(5, 4, Some(20))));
}

#[test]
fn processing_epoch_put_rejects_cadence_version_changes() {
    let path = test_path("processing-epoch-version.cva");
    let id = memory_id(7);
    let lane = ProcessingLaneId(1);
    let mut container = Container::create(path).unwrap();
    let mut store = ProcessingEpochStore::default();

    store
        .put(&mut container, id, lane, state(1, 1, Some(10)))
        .unwrap();

    assert!(matches!(
        store.put(&mut container, id, lane, state(2, 2, Some(20))),
        Err(MemoryError::InvalidField(
            "Processing epoch cadence version"
        ))
    ));
    assert_eq!(store.state(id, lane), Some(state(1, 1, Some(10))));
}
