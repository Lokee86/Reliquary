use crate::*;
use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

#[test]
fn initial_admission_preparation_retains_birth_and_never_reenrolls() {
    let temp = TempDir::new();
    let mut container = Container::create(
        temp.0.join("initial.rel"),
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [91; 16],
    )
    .unwrap();
    let mut store = FreshnessStore::new(Some([91; 16]));
    store.initialize(&mut container).unwrap();
    let id = FreshnessMemoryId([93; 32]);
    let before = container.read_frames().unwrap().len();
    let effects = store.prepare_initial_admission(id, 3, 5).unwrap();
    assert_eq!(effects.len(), 2);
    assert_eq!(container.read_frames().unwrap().len(), before);
    for effect in effects {
        store.ingest_payload(&effect).unwrap();
    }
    assert!(
        store
            .prepare_initial_admission(id, 3, 8)
            .unwrap()
            .is_empty()
    );
    assert!(store.prepare_initial_admission(id, 2, 8).is_err());
    assert!(store.prepare_initial_admission(id, 3, 4).is_err());
    let record = store.record_at(id, 8).unwrap().unwrap();
    assert!(record.score <= 100);
    assert!(store.prepare_initial_admission(id, 8, 3).is_err());
    let existing = FreshnessMemoryId([94; 32]);
    store
        .create_birth(&mut container, existing, 2, false)
        .unwrap();
    let effects = store.prepare_initial_admission(existing, 3, 5).unwrap();
    assert_eq!(effects.len(), 1);
    for effect in effects {
        store.ingest_payload(&effect).unwrap();
    }
    assert!(
        store
            .prepare_initial_admission(existing, 3, 9)
            .unwrap()
            .is_empty()
    );
}

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&base).unwrap();
        let path = base.join(format!(
            "freshness-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn event(owner: [u8; 16], memory: FreshnessMemoryId, turn: u64, amount: i16) -> FreshnessEvent {
    let event_id = format!("dream-initial:{}:{}", hex(&owner), hex(&memory.0));
    FreshnessEvent {
        event_id: event_id.clone(),
        accepted_turn: turn,
        provenance: FreshnessEventProvenance {
            producer_kind: 1,
            source_id: event_id,
            origin_owner: owner,
            accepted_turn: turn,
            policy_version: u64::from(FRESHNESS_POLICY_VERSION),
        },
        proof: FreshnessEventProof {
            graph_version: 4,
            community_generation: Some(2),
            community_graph_version: Some(4),
        },
        effects: [(memory, amount)].into_iter().collect(),
    }
}

#[test]
fn births_admissions_uses_and_settled_cuts_survive_reopen() {
    let temp = TempDir::new();
    let path = temp.0.join("owner.rel");
    let owner = [7; 16];
    let memory = FreshnessMemoryId([3; 32]);
    {
        let mut container = Container::create(
            &path,
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            owner,
        )
        .unwrap();
        let mut store = FreshnessStore::new(Some(owner));
        assert!(store.initialize(&mut container).unwrap());
        assert!(
            store
                .create_birth(&mut container, memory, 10, false)
                .unwrap()
        );
        let born = store.records().unwrap()[&memory];
        assert_eq!(born, FreshnessRecord::created());
        assert_eq!(store.record_at(memory, 100).unwrap(), Some(born));
        assert!(store.admit(&mut container, memory, 20).unwrap());
        assert_eq!(store.record_at(memory, 40).unwrap().unwrap().score, 98);
        assert!(
            store
                .append_event(&mut container, event(owner, memory, 30, 50))
                .unwrap()
        );

        assert!(store.accepted_use("use-42").is_none());
        assert!(
            store
                .accept_use(
                    &mut container,
                    "use-42".into(),
                    40,
                    vec![memory],
                    FreshnessEventProof {
                        graph_version: 4,
                        community_generation: None,
                        community_graph_version: None,
                    },
                )
                .unwrap()
        );
        let size_after_use = fs::metadata(&path).unwrap().len();
        assert!(
            !store
                .accept_use(
                    &mut container,
                    "use-42".into(),
                    40,
                    vec![memory],
                    FreshnessEventProof {
                        graph_version: 4,
                        community_generation: None,
                        community_graph_version: None,
                    },
                )
                .unwrap()
        );
        assert_eq!(fs::metadata(&path).unwrap().len(), size_after_use);
        container.sync().unwrap();
    }

    let bytes = fs::read(&path).unwrap();
    assert!(bytes.windows(8).any(|window| window == b"RELFRS02"));
    assert!(!bytes.windows(8).any(|window| window == b"RELFRS07"));
    let container = Container::open_read_only(&path).unwrap();
    let store = FreshnessStore::open(&container).unwrap();
    let current = store.records().unwrap()[&memory];
    assert_eq!(current.score, 100);
    assert_eq!(current.admitted_at_turn, Some(20));
    assert_eq!(current.accounted_turn, Some(40));
    assert_eq!(store.record_at(memory, 40).unwrap().unwrap().score, 99);
    assert_eq!(store.records_through_turn(40).unwrap()[&memory].score, 100);
    let settled = store.record_at(memory, 50).unwrap().unwrap();
    assert_eq!(settled.score, 99);
    assert_eq!(settled.accounted_turn, Some(50));
    assert_eq!(store.accepted_use("use-42").unwrap().sources, vec![memory]);
    let use_event = store
        .event(&format!(
            "accepted-use:{}:{}:{}",
            "07".repeat(16),
            "7573652d3432",
            "03".repeat(32)
        ))
        .unwrap();
    assert_eq!(use_event.provenance.producer_kind, 2);
    assert_eq!(use_event.effects.get(&memory), Some(&25));
    let replay = store.replay_entries();
    assert_eq!(replay.len(), 4);
    assert_eq!(
        replay
            .iter()
            .filter(|entry| matches!(entry, FreshnessReplayEntry::Event(_)))
            .count(),
        1,
        "the accepted-use access event is derived from its durable receipt"
    );
    assert!(replay.iter().any(|entry| matches!(
        entry,
        FreshnessReplayEntry::AcceptedUse(receipt, proof)
            if receipt.use_id == "use-42"
                && *proof == FreshnessEventProof {
                    graph_version: 4,
                    community_generation: None,
                    community_graph_version: None,
                }
    )));
}

#[test]
fn accepted_use_cut_exposes_only_the_complete_group_boundary() {
    let temp = TempDir::new();
    let path = temp.0.join("grouped-cut.rel");
    let owner = [0x27; 16];
    let first = FreshnessMemoryId([1; 32]);
    let second = FreshnessMemoryId([2; 32]);
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    let mut store = FreshnessStore::new(Some(owner));
    store.initialize(&mut container).unwrap();
    store.create_birth(&mut container, first, 1, true).unwrap();
    store.create_birth(&mut container, second, 1, true).unwrap();
    store
        .accept_use(
            &mut container,
            "grouped".into(),
            2,
            vec![first, second],
            FreshnessEventProof {
                graph_version: 1,
                community_generation: None,
                community_graph_version: None,
            },
        )
        .unwrap();

    let mut ids = store
        .accepted_events()
        .into_iter()
        .filter(|event| event.accepted_turn == 2)
        .map(|event| event.event_id)
        .collect::<Vec<_>>();
    ids.sort();
    assert_eq!(ids.len(), 2);
    assert!(!store.is_authoritative_cut(&FreshnessEventCut {
        rel_turn: 2,
        event_id_inclusive: Some(ids[0].clone()),
    }));
    let complete = store.cut_through_turn(2);
    assert_eq!(complete.event_id_inclusive.as_ref(), ids.last());
    assert!(store.is_authoritative_cut(&complete));
    assert_eq!(store.records_at(&complete).unwrap().len(), 2);
}

#[test]
fn owner_mismatch_and_forged_event_provenance_are_rejected() {
    let temp = TempDir::new();
    let path = temp.0.join("owner-bound.rel");
    let owner = [8; 16];
    let memory = FreshnessMemoryId([4; 32]);
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    let mut store = FreshnessStore::new(Some(owner));
    store.initialize(&mut container).unwrap();
    store.create_birth(&mut container, memory, 1, true).unwrap();
    let mut forged = event(owner, memory, 2, 50);
    forged.provenance.origin_owner = [9; 16];
    assert!(store.append_event(&mut container, forged).is_err());
    drop(store);
    drop(container);

    let mut wrong_owner = FreshnessStore::new(Some([9; 16]));
    assert!(wrong_owner.bind_owner(owner).is_err());
    let container = Container::open_read_only(&path).unwrap();
    assert_eq!(
        FreshnessStore::open(&container).unwrap().owner_uuid(),
        Some(owner)
    );
    drop(container);
    let mut writer = Container::open_writer(&path).unwrap();
    writer
        .append(&super::codec::encode_policy(
            [9; 16],
            FreshnessPolicy::default_v1(),
        ))
        .unwrap();
    writer.sync().unwrap();
    drop(writer);
    let container = Container::open_read_only(&path).unwrap();
    assert!(FreshnessStore::open(&container).is_err());
}

#[test]
fn access_without_accepted_use_receipt_cannot_reinforce_freshness() {
    let temp = TempDir::new();
    let path = temp.0.join("unreceipted-access.rel");
    let owner = [11; 16];
    let memory = FreshnessMemoryId([6; 32]);
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    let mut store = FreshnessStore::new(Some(owner));
    store.initialize(&mut container).unwrap();
    store.create_birth(&mut container, memory, 1, true).unwrap();
    let before = container.read_frames().unwrap().len();

    let mut unreceipted = event(owner, memory, 2, 25);
    unreceipted.event_id = format!(
        "accepted-use:{}:72657472696576616c:{}",
        hex(&owner),
        hex(&memory.0)
    );
    unreceipted.provenance.source_id = "retrieval".into();
    unreceipted.provenance.producer_kind = 2;
    unreceipted.effects.clear();
    unreceipted
        .effects
        .insert(memory, FreshnessPolicy::default_v1().access_principal);

    assert!(store.append_event(&mut container, unreceipted).is_err());
    assert_eq!(container.read_frames().unwrap().len(), before);
    assert_eq!(store.record_at(memory, 2).unwrap().unwrap().score, 100);
    assert!(store.accepted_use("retrieval").is_none());
}

#[test]
fn unsupported_freshness_versions_fail_closed() {
    let mut store = FreshnessStore::default();
    assert!(store.ingest_payload(b"RELFRS07").is_err());
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn fresh_store_cannot_append_a_second_policy() {
    let temp = TempDir::new();
    let path = temp.0.join("owner.rel");
    let owner = [8; 16];
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    let mut first = FreshnessStore::new(Some(owner));
    first.initialize(&mut container).unwrap();
    first
        .create_birth(&mut container, FreshnessMemoryId([4; 32]), 3, false)
        .unwrap();
    let before = container.read_frames().unwrap().len();
    let mut fresh = FreshnessStore::new(Some(owner));
    assert!(fresh.initialize(&mut container).is_err());
    assert_eq!(container.read_frames().unwrap().len(), before);
    assert_eq!(
        FreshnessStore::open(&container)
            .unwrap()
            .records()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn replay_rejects_duplicate_identical_policy_frames() {
    let temp = TempDir::new();
    let path = temp.0.join("duplicate-policy.rel");
    let owner = [12; 16];
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    let mut store = FreshnessStore::new(Some(owner));
    store.initialize(&mut container).unwrap();
    container
        .append(&super::codec::encode_policy(
            owner,
            FreshnessPolicy::default_v1(),
        ))
        .unwrap();
    container.sync().unwrap();
    drop(store);
    drop(container);

    let container = Container::open_read_only(&path).unwrap();
    assert!(
        FreshnessStore::open(&container)
            .unwrap_err()
            .contains("duplicate Freshness owner policy")
    );
}

#[test]
fn replay_rejects_duplicate_event_frames_even_when_identical() {
    let temp = TempDir::new();
    let path = temp.0.join("duplicate-event.rel");
    let owner = [10; 16];
    let memory = FreshnessMemoryId([5; 32]);
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    let mut store = FreshnessStore::new(Some(owner));
    store.initialize(&mut container).unwrap();
    store.create_birth(&mut container, memory, 1, true).unwrap();
    let accepted = event(owner, memory, 2, 50);
    store
        .append_event(&mut container, accepted.clone())
        .unwrap();
    container
        .append(
            &super::codec::encode_entry(owner, &super::codec::JournalEntry::Event(accepted))
                .unwrap(),
        )
        .unwrap();
    container.sync().unwrap();
    drop(store);
    drop(container);

    let container = Container::open_read_only(&path).unwrap();
    assert!(
        FreshnessStore::open(&container)
            .unwrap_err()
            .contains("duplicate Freshness event")
    );
}

#[test]
fn freshness_rejects_other_container_kind_with_same_owner_id() {
    let temp = TempDir::new();
    let owner = [9; 16];
    let mut rel = Container::create(
        &temp.0.join("owner.rel"),
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    let mut store = FreshnessStore::new(Some(owner));
    store.initialize(&mut rel).unwrap();
    let mut phy = Container::create(
        &temp.0.join("private.phy"),
        ContainerIdentity::phylactery(),
        owner,
    )
    .unwrap();
    assert!(
        store
            .create_birth(&mut phy, FreshnessMemoryId([7; 32]), 1, false)
            .is_err()
    );
    assert!(phy.read_frames().unwrap().is_empty());
    let mut reopened = FreshnessStore::open(&rel).unwrap();
    assert!(reopened.initialize(&mut phy).is_err());
}
