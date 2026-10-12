use super::*;
use crate::runtime_host_test_support::test_path;
use crate::{Cva, MemoryDraft, SimulatedEmbeddingEndpoint, VectorNormalization};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn gateway() -> GatewayRuntime {
    GatewayRuntime::new(
        test_path("p5b-state"),
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
        EpisodePolicy::default(),
    )
}

fn rel(gateway: &GatewayRuntime, name: &str) -> String {
    gateway
        .mount_rel(InteractionRuntime::new(
            Cva::create_project(test_path(name)).unwrap(),
        ))
        .unwrap()
}
fn phy(gateway: &GatewayRuntime, name: &str) -> String {
    gateway
        .mount_phy(Phylactery::create(test_path(name)).unwrap())
        .unwrap()
}
fn attach(
    gateway: &GatewayRuntime,
    principal: &str,
    rels: &[&str],
    phy: Option<&str>,
    manage: bool,
) -> Arc<InstanceRuntime> {
    let mut grants = BTreeMap::new();
    for rel in rels {
        let mut caps = BTreeSet::from([OwnerGrant::ViewRel]);
        if manage {
            caps.insert(OwnerGrant::ManageRel);
        }
        grants.insert((*rel).into(), caps);
    }
    if let Some(phy) = phy {
        grants.insert(phy.into(), BTreeSet::from([OwnerGrant::UseOwnPhy]));
    }
    gateway
        .attach_instance(principal.into(), phy.map(str::to_owned), grants)
        .unwrap()
}
fn draft(title: &str) -> MemoryDraft {
    MemoryDraft {
        category: "knowledge".into(),
        memory_type: "fact".into(),
        authority_kind: "direct".into(),
        title: title.into(),
        content: format!("{title} content"),
        scope: "private".into(),
        lifecycle_state: "canonical".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(1),
        temporal_status: "unknown".into(),
        mutation_id: format!("p5b-{title}"),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}
fn seed_phy(gateway: &GatewayRuntime, instance: &InstanceRuntime, id: &str, title: &str) {
    let token = gateway
        .authorize(instance, id, OwnerGrant::UseOwnPhy)
        .unwrap();
    gateway
        .with_phy(&token, |owner| {
            owner
                .publish_memory(None, 0, draft(title))
                .map_err(operation)?;
            owner.sync().map_err(operation)
        })
        .unwrap();
    gateway.wake_phy_background_for(instance, id).unwrap();
}
fn wait_until(mut condition: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < until, "background work did not progress");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn exact_background_controls_share_workers_and_never_follow_selected_rel_or_other_phy() {
    let gateway = gateway();
    let first = rel(&gateway, "p5b-first.rel");
    let second = rel(&gateway, "p5b-second.rel");
    let alice_phy = phy(&gateway, "p5b-alice.phy");
    let bob_phy = phy(&gateway, "p5b-bob.phy");
    let alice = attach(
        &gateway,
        "alice",
        &[&first, &second],
        Some(&alice_phy),
        true,
    );
    let alice_again = attach(&gateway, "alice", &[&first], Some(&alice_phy), true);
    let bob = attach(&gateway, "bob", &[&first], Some(&bob_phy), true);
    let viewer = attach(&gateway, "viewer", &[&first], None, false);
    gateway.select_rel(&alice, &second).unwrap();
    gateway
        .create_knowledge_memory_for(
            &alice,
            &first,
            "shared".into(),
            "REL memory".into(),
            "knowledge".into(),
            "fact".into(),
            1,
        )
        .unwrap();
    seed_phy(&gateway, &alice, &alice_phy, "private-alice");
    seed_phy(&gateway, &bob, &bob_phy, "private-bob");

    let shared = gateway.background_status_for(&alice, &first).unwrap();
    assert_eq!(shared.reliquary_memories.memories, 1);
    assert!(shared.phylactery_memories.is_none());
    assert_eq!(
        gateway
            .background_status_for(&alice, &second)
            .unwrap()
            .reliquary_memories
            .memories,
        0
    );
    assert_eq!(
        gateway
            .phy_background_status_for(&alice_again, &alice_phy)
            .unwrap()
            .phylactery_memories
            .unwrap()
            .memories,
        1
    );
    assert!(gateway.phy_background_status_for(&alice, &bob_phy).is_err());
    assert!(gateway.wake_phy_background_for(&alice, &bob_phy).is_err());
    assert!(
        gateway
            .set_phy_background_enabled_for(&alice, &bob_phy, false)
            .is_err()
    );
    assert!(gateway.wake_rel_background_for(&viewer, &first).is_err());
    assert!(
        gateway
            .set_rel_background_enabled_for(&viewer, &first, false)
            .is_err()
    );

    assert_eq!(
        gateway
            .background_worker_state_for(&alice, &alice_phy)
            .unwrap(),
        (3, true)
    );
    gateway
        .set_phy_background_enabled_for(&alice, &alice_phy, false)
        .unwrap();
    assert_eq!(
        gateway
            .background_worker_state_for(&alice_again, &alice_phy)
            .unwrap(),
        (3, false)
    );
    assert_eq!(
        gateway.background_worker_state_for(&bob, &bob_phy).unwrap(),
        (3, true)
    );
    gateway
        .set_rel_background_enabled_for(&alice, &first, false)
        .unwrap();
    assert_eq!(
        gateway.background_worker_state_for(&alice, &first).unwrap(),
        (3, false)
    );
    assert_eq!(
        gateway
            .background_worker_state_for(&alice, &second)
            .unwrap(),
        (3, true)
    );
    gateway.detach_instance(&alice_again).unwrap();
    assert!(
        gateway
            .phy_background_status_for(&alice_again, &alice_phy)
            .is_err()
    );
    assert_eq!(
        gateway
            .background_worker_state_for(&alice, &alice_phy)
            .unwrap(),
        (3, false)
    );
}

#[test]
fn configured_routes_wake_native_owners_and_paused_private_work_recovers_exactly() {
    let gateway = gateway();
    let shared = rel(&gateway, "p5b-route.rel");
    let a = phy(&gateway, "p5b-route-a.phy");
    let b = phy(&gateway, "p5b-route-b.phy");
    let alice = attach(&gateway, "alice", &[&shared], Some(&a), true);
    let bob = attach(&gateway, "bob", &[&shared], Some(&b), true);
    gateway
        .set_phy_background_enabled_for(&alice, &a, false)
        .unwrap();
    seed_phy(&gateway, &alice, &a, "route-alice");
    seed_phy(&gateway, &bob, &b, "route-bob");
    gateway
        .set_routes(ReliquaryRuntimeRoutes::new(
            None,
            None,
            None,
            None,
            Some(Arc::new(SimulatedEmbeddingEndpoint::new(
                8,
                VectorNormalization::L2,
                42,
            ))),
        ))
        .unwrap();
    wait_until(|| {
        gateway
            .phy_background_status_for(&bob, &b)
            .unwrap()
            .phylactery_vectors
            .unwrap()
            .bindings
            == 1
    });
    assert_eq!(
        gateway
            .phy_background_status_for(&alice, &a)
            .unwrap()
            .phylactery_vectors
            .unwrap()
            .bindings,
        0
    );
    gateway
        .set_phy_background_enabled_for(&alice, &a, true)
        .unwrap();
    wait_until(|| {
        gateway
            .phy_background_status_for(&alice, &a)
            .unwrap()
            .phylactery_vectors
            .unwrap()
            .bindings
            == 1
    });

    let old = gateway
        .authorize(&alice, &a, OwnerGrant::UseOwnPhy)
        .unwrap();
    gateway.quiesce_phy_for(&alice, &a).unwrap();
    assert_eq!(
        gateway.background_worker_state_for(&bob, &b).unwrap(),
        (3, true)
    );
    gateway.reopen_phy_for(&alice, &a).unwrap();
    assert!(gateway.with_phy(&old, |_| Ok(())).is_err());
    assert_eq!(
        gateway.background_worker_state_for(&alice, &a).unwrap(),
        (3, true)
    );
    let runtime = Arc::clone(
        &gateway
            .phy_executions
            .read()
            .unwrap()
            .get(&a)
            .unwrap()
            .runtime,
    );
    runtime.lock().unwrap().restart_workers().unwrap();
    assert_eq!(
        gateway.background_worker_state_for(&alice, &a).unwrap(),
        (3, true)
    );
    gateway.wake_phy_background_for(&alice, &a).unwrap();
    assert_eq!(
        gateway
            .phy_background_status_for(&alice, &a)
            .unwrap()
            .phylactery_vectors
            .unwrap()
            .bindings,
        1
    );
}

struct BlockingEmbedding {
    entered: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
}
impl crate::EmbeddingEndpoint for BlockingEmbedding {
    fn dimensions(&self) -> u32 {
        2
    }
    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::L2
    }
    fn embed(
        &self,
        _: crate::EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, crate::EmbeddingEndpointError> {
        if inputs.iter().any(|text| text.contains("BLOCK")) {
            self.entered.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(10))
                .unwrap();
        }
        Ok(inputs.iter().map(|_| vec![1.0, 0.0]).collect())
    }
}

#[test]
fn rel_provider_io_releases_store_locks_and_quiesce_fences_late_vector_result() {
    let gateway = Arc::new(gateway());
    let blocked = rel(&gateway, "p5b-blocked.rel");
    let other = rel(&gateway, "p5b-unrelated.rel");
    let private = phy(&gateway, "p5b-unrelated.phy");
    let instance = attach(&gateway, "alice", &[&blocked, &other], Some(&private), true);
    gateway
        .create_knowledge_memory_for(
            &instance,
            &blocked,
            "BLOCK".into(),
            "late result".into(),
            "knowledge".into(),
            "fact".into(),
            1,
        )
        .unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    gateway
        .set_routes(ReliquaryRuntimeRoutes::new(
            None,
            None,
            None,
            None,
            Some(Arc::new(BlockingEmbedding {
                entered: entered_tx,
                release: Mutex::new(release_rx),
            })),
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    // These exact-owner reads must complete while inference is still blocked.
    assert_eq!(
        gateway
            .background_status_for(&instance, &blocked)
            .unwrap()
            .reliquary_vectors
            .bindings,
        0
    );
    assert!(gateway.background_status_for(&instance, &other).is_ok());
    assert!(
        gateway
            .phy_background_status_for(&instance, &private)
            .is_ok()
    );
    gateway
        .set_routes(ReliquaryRuntimeRoutes::default())
        .unwrap();
    let old = gateway
        .authorize(&instance, &blocked, OwnerGrant::ViewRel)
        .unwrap();
    let stopping = Arc::clone(&gateway);
    let stopping_owner = blocked.clone();
    let transition = std::thread::spawn(move || stopping.quiesce_rel(&stopping_owner));
    wait_until(|| gateway.owner_admission(&blocked).unwrap() == OwnerAdmission::Closed);
    assert!(gateway.with_rel(&old, |_| Ok(())).is_err());
    assert!(gateway.background_status_for(&instance, &other).is_ok());
    release_tx.send(()).unwrap();
    transition.join().unwrap().unwrap();
    gateway.reopen_rel(&blocked).unwrap();
    assert!(gateway.with_rel(&old, |_| Ok(())).is_err());
    assert_eq!(
        gateway
            .background_status_for(&instance, &blocked)
            .unwrap()
            .reliquary_vectors
            .bindings,
        0
    );
    assert_eq!(
        gateway
            .background_worker_state_for(&instance, &blocked)
            .unwrap(),
        (3, true)
    );
    let runtime = Arc::clone(
        &gateway
            .rel_executions
            .read()
            .unwrap()
            .get(&blocked)
            .unwrap()
            .owner,
    );
    runtime.lock().unwrap().restart_workers().unwrap();
    assert_eq!(
        gateway
            .background_worker_state_for(&instance, &blocked)
            .unwrap(),
        (3, true)
    );
    gateway.shutdown().unwrap();
    assert!(
        gateway
            .set_routes(ReliquaryRuntimeRoutes::default())
            .is_err()
    );
}

#[test]
fn private_provider_io_releases_store_lock_and_fenced_result_stays_unpublished() {
    let gateway = Arc::new(gateway());
    let a = phy(&gateway, "p5b-block-phy-a.phy");
    let b = phy(&gateway, "p5b-block-phy-b.phy");
    let alice = attach(&gateway, "alice", &[], Some(&a), false);
    let bob = attach(&gateway, "bob", &[], Some(&b), false);
    seed_phy(&gateway, &alice, &a, "BLOCK-private");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    gateway
        .set_routes(ReliquaryRuntimeRoutes::new(
            None,
            None,
            None,
            None,
            Some(Arc::new(BlockingEmbedding {
                entered: entered_tx,
                release: Mutex::new(release_rx),
            })),
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(
        gateway
            .phy_background_status_for(&alice, &a)
            .unwrap()
            .phylactery_vectors
            .unwrap()
            .bindings,
        0
    );
    assert!(gateway.phy_background_status_for(&bob, &b).is_ok());
    gateway
        .set_routes(ReliquaryRuntimeRoutes::default())
        .unwrap();
    let old = gateway
        .authorize(&alice, &a, OwnerGrant::UseOwnPhy)
        .unwrap();
    let stopping = Arc::clone(&gateway);
    let owner = a.clone();
    let transition = std::thread::spawn(move || stopping.quiesce_phy(&owner));
    wait_until(|| gateway.owner_admission(&a).unwrap() == OwnerAdmission::Closed);
    assert!(gateway.with_phy(&old, |_| Ok(())).is_err());
    assert!(gateway.phy_background_status_for(&bob, &b).is_ok());
    release_tx.send(()).unwrap();
    transition.join().unwrap().unwrap();
    gateway.reopen_phy_for(&alice, &a).unwrap();
    assert!(gateway.with_phy(&old, |_| Ok(())).is_err());
    assert_eq!(
        gateway
            .phy_background_status_for(&alice, &a)
            .unwrap()
            .phylactery_vectors
            .unwrap()
            .bindings,
        0
    );
    assert_eq!(
        gateway.background_worker_state_for(&alice, &a).unwrap(),
        (3, true)
    );
}
