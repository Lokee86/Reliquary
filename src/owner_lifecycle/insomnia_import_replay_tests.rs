use crate::insomnia::codec::{self, InsomniaRecord};
use crate::insomnia::store::InsomniaStore;
use crate::{
    Container, EpisodeId, InsomniaAttempt, InsomniaPriority, InsomniaWork, InsomniaWorkState,
    MemoryId,
};
use std::fs;
use std::path::PathBuf;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("insomnia-import-replay-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn legacy_attempt_payload(
    episode_id: EpisodeId,
    attempt: u32,
    state: InsomniaWorkState,
    started_at_ns: i64,
    completed_at_ns: i64,
) -> Vec<u8> {
    // Test-only compatibility fixture for the existing decode_attempt layout.
    // Current processing writes grouped completion records and has no CVAINSA1 writer.
    let mut payload = b"CVAINSA1".to_vec();
    payload.extend_from_slice(&episode_id.0);
    payload.extend_from_slice(&attempt.to_le_bytes());
    payload.push(state.tag());
    payload.extend_from_slice(&started_at_ns.to_le_bytes());
    payload.extend_from_slice(&completed_at_ns.to_le_bytes());
    payload.extend_from_slice(&0_u32.to_le_bytes()); // rejected count
    push_string(&mut payload, "legacy-fixture");
    push_string(&mut payload, "1");
    payload.extend_from_slice(&1_u32.to_le_bytes());
    payload.extend_from_slice(&[7; 32]); // one historical MemoryId
    payload.push(0); // no error
    payload
}

fn push_string(payload: &mut Vec<u8>, value: &str) {
    payload.extend_from_slice(&(value.len() as u32).to_le_bytes());
    payload.extend_from_slice(value.as_bytes());
}

#[test]
fn import_replay_applies_work_and_legacy_attempts_in_physical_order_across_reopen() {
    let fixture = Fixture::new();
    let path = fixture.0.join("owner.rel");
    let episode_id = EpisodeId([3; 32]);
    let work = InsomniaWork {
        episode_id,
        priority: InsomniaPriority::Import,
        state: InsomniaWorkState::Complete,
        attempt_count: 2,
        lease_owner: None,
        lease_token: None,
        lease_expires_ns: None,
        retry_after_ns: None,
        last_error: None,
        updated_at_ns: 30,
    };
    let work_payload = codec::encode_work(&work).unwrap();
    let failed_payload = legacy_attempt_payload(episode_id, 1, InsomniaWorkState::Failed, 10, 20);
    let complete_payload =
        legacy_attempt_payload(episode_id, 2, InsomniaWorkState::Complete, 21, 30);

    // Pin the test fixture to the already-supported decoder before exercising
    // the owner-lifecycle replay path.
    assert!(matches!(
        codec::decode_record(&failed_payload).unwrap(),
        Some(InsomniaRecord::Attempt(_))
    ));
    assert!(matches!(
        codec::decode_record(&complete_payload).unwrap(),
        Some(InsomniaRecord::Attempt(_))
    ));

    let mut container = Container::create(&path).unwrap();
    let mut insomnia = InsomniaStore::empty();
    for payload in [&work_payload, &failed_payload, &complete_payload] {
        assert!(
            super::insomnia_import_replay::replay_record(&mut container, &mut insomnia, payload,)
                .unwrap()
        );
    }
    assert_eq!(insomnia.work(episode_id), Some(&work));
    let expected_attempts = vec![
        InsomniaAttempt {
            episode_id,
            attempt: 1,
            state: InsomniaWorkState::Failed,
            started_at_ns: 10,
            completed_at_ns: 20,
            extractor_model: "legacy-fixture".into(),
            extractor_version: "1".into(),
            memory_ids: vec![MemoryId([7; 32])],
            external_memory_refs: Vec::new(),
            rejected_count: 0,
            error: None,
        },
        InsomniaAttempt {
            episode_id,
            attempt: 2,
            state: InsomniaWorkState::Complete,
            started_at_ns: 21,
            completed_at_ns: 30,
            extractor_model: "legacy-fixture".into(),
            extractor_version: "1".into(),
            memory_ids: vec![MemoryId([7; 32])],
            external_memory_refs: Vec::new(),
            rejected_count: 0,
            error: None,
        },
    ];
    assert_eq!(insomnia.attempts(episode_id), expected_attempts);

    let written: Vec<Vec<u8>> = container
        .chunks()
        .unwrap()
        .into_iter()
        .map(|object| container.read(object).unwrap())
        .collect();
    assert_eq!(
        written,
        vec![work_payload, failed_payload, complete_payload]
    );
    container.sync().unwrap();
    drop(container);

    let mut reopened = Container::open(&path).unwrap();
    let reopened_payloads: Vec<Vec<u8>> = reopened
        .chunks()
        .unwrap()
        .into_iter()
        .map(|object| reopened.read(object).unwrap())
        .collect();
    assert_eq!(reopened_payloads, written);

    let mut rebuilt = InsomniaStore::empty();
    for payload in &reopened_payloads {
        match codec::decode_record(payload).unwrap() {
            Some(InsomniaRecord::Work(work)) => rebuilt.apply_work(work),
            Some(InsomniaRecord::Attempt(attempt)) => rebuilt.apply_attempt(attempt),
            None => {}
        }
    }
    assert_eq!(rebuilt.work(episode_id), Some(&work));
    assert_eq!(rebuilt.attempts(episode_id), expected_attempts);
}
