use crate::*;
use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(1);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".insomnia-test-tmp");
        fs::create_dir_all(&dir).unwrap();
        Self(dir.join(format!(
            "insomnia-{}-{}.rel",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
        if let Some(p) = self.0.parent() {
            let _ = fs::remove_dir(p);
        }
    }
}
fn container(path: &PathBuf, owner: u8) -> Container {
    let mut id = [0; 16];
    id[0] = owner;
    Container::create(
        path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        id,
    )
    .unwrap()
}
fn order(id: EpisodeId) -> AcceptedEpisodeOrder {
    AcceptedEpisodeOrder {
        episode_id: id,
        source_through_ns: 4,
        conversation_id: "conversation".into(),
        start_node_id: "node".into(),
    }
}
fn receipt(id: EpisodeId) -> AcceptedCompletionState {
    AcceptedCompletionState {
        episode_id: id,
        attempt: 1,
        started_at_ns: 10,
        completed_at_ns: 20,
        extractor_model: "extractor".into(),
        extractor_version: "v1".into(),
        memory_ids: vec![MemoryId([5; 32])],
        external_memory_refs: vec![ExternalMemoryRef {
            owner_id: "phy-owner".into(),
            memory_id: MemoryId([6; 32]),
        }],
        rejected_count: 2,
    }
}

#[test]
fn unqueued_admission_batch_is_bounded_and_owner_ordered() {
    let path = Temp::new();
    let mut container = container(&path.0, 1);
    let first = EpisodeId([1; 32]);
    let second = EpisodeId([2; 32]);
    let mut store = InsomniaStore::open(&container, [order(second), order(first)]).unwrap();
    store
        .queue(&mut container, order(first), InsomniaPriority::Live, 10)
        .unwrap();
    assert_eq!(store.unqueued_orders(1), vec![order(second)]);
    assert!(store.unqueued_orders(0).is_empty());
}

#[test]
fn claim_and_completion_receipt_survive_reopen_and_duplicate_receipt_is_idempotent() {
    let path = Temp::new();
    let id = EpisodeId([1; 32]);
    let accepted = [order(id)];
    let token;
    {
        let mut c = container(&path.0, 1);
        let mut s = InsomniaStore::open(&c, accepted.clone()).unwrap();
        let (q, inserted) = s
            .queue(
                &mut c,
                accepted[0].clone(),
                InsomniaPriority::ImmediateLive,
                1,
            )
            .unwrap();
        assert!(inserted);
        assert_eq!(q.state, InsomniaWorkState::Pending);
        let claimed = s.claim_next(&mut c, "worker", 2, 100).unwrap().unwrap();
        token = claimed.lease_token.unwrap();
    }
    {
        let c = Container::open_writer(&path.0).unwrap();
        let s = InsomniaStore::open(&c, accepted.clone()).unwrap();
        let work = s.work(id).unwrap();
        assert_eq!(work.state, InsomniaWorkState::Processing);
        assert_eq!(work.lease_token, Some(token));
        drop(c);
        let mut c = Container::open_writer(&path.0).unwrap();
        let mut s = InsomniaStore::open(&c, accepted.clone()).unwrap();
        let r = receipt(id);
        assert_eq!(
            s.complete(&mut c, token, r.clone(), 15).unwrap(),
            CompletionCheck::Apply
        );
        assert_eq!(
            s.complete(&mut c, token, r.clone(), 15).unwrap(),
            CompletionCheck::AlreadyAccepted
        );
    }
    let c = Container::open_read_only(&path.0).unwrap();
    let s = InsomniaStore::open(&c, accepted).unwrap();
    assert_eq!(s.work(id).unwrap().state, InsomniaWorkState::Complete);
    assert_eq!(s.receipt(id, 1), Some(&receipt(id)));
}

#[test]
fn owner_binding_and_admission_are_enforced() {
    let path = Temp::new();
    let id = EpisodeId([3; 32]);
    let accepted = [order(id)];
    let mut c = container(&path.0, 2);
    let mut s = InsomniaStore::open(&c, accepted.clone()).unwrap();
    assert!(matches!(
        s.queue(&mut c, order(EpisodeId([4; 32])), InsomniaPriority::Live, 1),
        Err(InsomniaError::UnacceptedEpisode)
    ));
    s.queue(&mut c, accepted[0].clone(), InsomniaPriority::Live, 1)
        .unwrap();
    let other_path = Temp::new();
    let mut other = container(&other_path.0, 3);
    assert!(matches!(
        s.queue(&mut other, accepted[0].clone(), InsomniaPriority::Live, 1),
        Err(InsomniaError::WrongOwner)
    ));
    drop(other);
    drop(c);
    let source = Container::open_read_only(&path.0).unwrap();
    let payload = source.read_frames().unwrap()[0].payload.clone();
    drop(source);
    let copied_path = Temp::new();
    let mut copied = container(&copied_path.0, 9);
    copied.append(&payload).unwrap();
    copied.sync().unwrap();
    assert!(matches!(
        InsomniaStore::open(&copied, accepted),
        Err(InsomniaError::WrongOwner)
    ));
}
