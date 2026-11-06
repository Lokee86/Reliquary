use super::*;
use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
use reliquary_vector_store::{
    ArchiveVectorStore, CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore,
    VectorGenerationStore,
};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Driver {
    kind: OwnerWorkerKind,
    polls: Arc<AtomicUsize>,
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
}
impl OwnerWorkerDriver for Driver {
    fn kind(&self) -> OwnerWorkerKind {
        self.kind
    }
    fn poll(&self) -> Result<OwnerWorkerPoll, String> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        self.polls.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(2));
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(OwnerWorkerPoll::Idle)
    }
}

fn initialize_rel(path: &Path, owner: [u8; 16]) {
    let mut container = Container::create(
        path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    PackedVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    CompatibilityProfileStore::default()
        .initialize(&mut container)
        .unwrap();
    ArchiveVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    MemoryVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    VectorGenerationStore::empty()
        .initialize(&mut container)
        .unwrap();
    container.sync().unwrap();
}

fn driver(
    kind: OwnerWorkerKind,
    polls: &Arc<AtomicUsize>,
    active: &Arc<AtomicUsize>,
    peak: &Arc<AtomicUsize>,
) -> Arc<dyn OwnerWorkerDriver> {
    Arc::new(Driver {
        kind,
        polls: Arc::clone(polls),
        active: Arc::clone(active),
        peak: Arc::clone(peak),
    })
}

#[test]
fn specialty_workers_are_bounded_quiesced_and_restartable() {
    let root = std::env::current_dir()
        .unwrap()
        .join(".test-tmp")
        .join(format!("owner-workers-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("owner.rel");
    let raw_owner = [91; 16];
    initialize_rel(&path, raw_owner);
    let gateway = Gateway::new();
    let owner = gateway
        .mount_writable_reliquary(
            &path,
            &GraphAdmission::for_replay(raw_owner, [], [], [], [], []),
            [],
            [],
        )
        .unwrap();
    let instance = Arc::new(gateway.instance("alice", [owner], []).unwrap());
    let polls = Arc::new(AtomicUsize::new(0));
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let drivers = [
        OwnerWorkerKind::Dream,
        OwnerWorkerKind::Entity,
        OwnerWorkerKind::Vector,
        OwnerWorkerKind::Freshness,
    ]
    .map(|kind| driver(kind, &polls, &active, &peak));
    let config = OwnerWorkerConfig {
        idle_poll: Duration::from_millis(2),
    };
    let mut first = gateway
        .start_owner_workers(Arc::clone(&instance), owner, drivers, config.clone())
        .unwrap();
    while polls.load(Ordering::SeqCst) < 4 {
        std::thread::yield_now();
    }
    first.stop().unwrap();
    assert_eq!(active.load(Ordering::SeqCst), 0);
    assert!(peak.load(Ordering::SeqCst) <= 4);
    let before_restart = polls.load(Ordering::SeqCst);

    let mut second = gateway
        .start_owner_workers(
            instance,
            owner,
            [driver(OwnerWorkerKind::Freshness, &polls, &active, &peak)],
            config,
        )
        .unwrap();
    while polls.load(Ordering::SeqCst) == before_restart {
        std::thread::yield_now();
    }
    second.stop().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn specialty_workers_reject_foreign_instance_duplicate_kind_and_bad_bounds() {
    let root = std::env::current_dir()
        .unwrap()
        .join(".test-tmp")
        .join(format!("owner-worker-fences-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("owner.rel");
    let raw_owner = [92; 16];
    initialize_rel(&path, raw_owner);
    let gateway = Gateway::new();
    let owner = gateway
        .mount_writable_reliquary(
            &path,
            &GraphAdmission::for_replay(raw_owner, [], [], [], [], []),
            [],
            [],
        )
        .unwrap();
    let foreign_gateway = Gateway::new();
    let foreign = Arc::new(foreign_gateway.instance("alice", [], []).unwrap());
    let polls = Arc::new(AtomicUsize::new(0));
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    assert!(
        gateway
            .start_owner_workers(
                foreign,
                owner,
                [driver(OwnerWorkerKind::Dream, &polls, &active, &peak)],
                OwnerWorkerConfig::default()
            )
            .is_err()
    );
    let instance = Arc::new(gateway.instance("alice", [owner], []).unwrap());
    assert!(
        gateway
            .start_owner_workers(
                Arc::clone(&instance),
                owner,
                [
                    driver(OwnerWorkerKind::Dream, &polls, &active, &peak),
                    driver(OwnerWorkerKind::Dream, &polls, &active, &peak),
                ],
                OwnerWorkerConfig::default()
            )
            .is_err()
    );
    let mut active_coordinator = gateway
        .start_owner_workers(
            Arc::clone(&instance),
            owner,
            [driver(OwnerWorkerKind::Vector, &polls, &active, &peak)],
            OwnerWorkerConfig::default(),
        )
        .unwrap();
    assert!(
        gateway
            .start_owner_workers(
                Arc::clone(&instance),
                owner,
                [driver(OwnerWorkerKind::Vector, &polls, &active, &peak)],
                OwnerWorkerConfig::default()
            )
            .is_err()
    );
    assert!(
        gateway
            .clone()
            .start_owner_workers(
                Arc::clone(&instance),
                owner,
                [driver(OwnerWorkerKind::Vector, &polls, &active, &peak)],
                OwnerWorkerConfig::default()
            )
            .is_err()
    );
    active_coordinator.stop().unwrap();
    let mut replacement = gateway
        .start_owner_workers(
            Arc::clone(&instance),
            owner,
            [driver(OwnerWorkerKind::Vector, &polls, &active, &peak)],
            OwnerWorkerConfig::default(),
        )
        .unwrap();
    replacement.stop().unwrap();
    assert!(
        gateway
            .start_owner_workers(
                instance,
                owner,
                [driver(OwnerWorkerKind::Dream, &polls, &active, &peak)],
                OwnerWorkerConfig {
                    idle_poll: Duration::ZERO,
                }
            )
            .is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
}
