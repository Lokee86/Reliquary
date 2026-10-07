use super::*;
use crate::runtime_host_test_support::test_path;
use crate::{Cva, MemoryDraft, MemoryId, SimulatedEmbeddingEndpoint, VectorNormalization};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

fn draft(title: &str) -> MemoryDraft {
    MemoryDraft {
        category: "knowledge".into(),
        memory_type: "fact".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: title.into(),
        content: format!("{title} private evidence"),
        scope: "user".into(),
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
        mutation_id: Uuid::new_v4().to_string(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

struct Endpoint {
    simulated: SimulatedEmbeddingEndpoint,
    queries: AtomicUsize,
    block: AtomicBool,
    state: (Mutex<(bool, bool)>, Condvar),
}
impl Endpoint {
    fn new() -> Self {
        Self {
            simulated: SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 77),
            queries: AtomicUsize::new(0),
            block: AtomicBool::new(false),
            state: (Mutex::new((false, false)), Condvar::new()),
        }
    }
    fn wait_for_query(&self) {
        let state = self.state.0.lock().unwrap();
        let (state, _) = self
            .state
            .1
            .wait_timeout_while(state, Duration::from_secs(10), |state| !state.0)
            .unwrap();
        assert!(state.0, "query did not reach embedding");
    }
    fn release(&self) {
        self.state.0.lock().unwrap().1 = true;
        self.state.1.notify_all();
    }
}
impl crate::EmbeddingEndpoint for Endpoint {
    fn dimensions(&self) -> u32 {
        8
    }
    fn normalization(&self) -> VectorNormalization {
        VectorNormalization::L2
    }
    fn embed(
        &self,
        mode: crate::EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, crate::EmbeddingEndpointError> {
        if mode == crate::EmbeddingMode::Query && inputs == ["evidence"] {
            self.queries.fetch_add(1, Ordering::SeqCst);
            if self.block.load(Ordering::SeqCst) {
                let mut state = self.state.0.lock().unwrap();
                state.0 = true;
                self.state.1.notify_all();
                while !state.1 {
                    state = self.state.1.wait(state).unwrap();
                }
            }
        }
        self.simulated.embed(mode, inputs)
    }
}

struct Fixture {
    g: Arc<GatewayRuntime>,
    endpoint: Arc<Endpoint>,
    rel: String,
    dependency: String,
    a: String,
    b: String,
    alice: Arc<InstanceRuntime>,
    bob: Arc<InstanceRuntime>,
    a_memory: MemoryId,
    b_memory: MemoryId,
}
impl Fixture {
    fn new() -> Self {
        let endpoint = Arc::new(Endpoint::new());
        let g = Arc::new(GatewayRuntime::new(
            test_path("private-state"),
            ReliquaryRuntimeRoutes::new(None, None, None, None, Some(endpoint.clone())),
            InsomniaWorkerConfig {
                workers: 0,
                ..InsomniaWorkerConfig::default()
            },
            EpisodePolicy::default(),
        ));
        let mut a_store = Phylactery::create(test_path("private-a.phy")).unwrap();
        let a_memory = a_store
            .publish_memory(None, 0, draft("Alice"))
            .unwrap()
            .0
            .id;
        let mut b_store = Phylactery::create(test_path("private-b.phy")).unwrap();
        let b_memory = b_store.publish_memory(None, 0, draft("Bob")).unwrap().0.id;
        for owner in [&mut a_store, &mut b_store] {
            let profile = owner
                .establish_compatibility_profile(&endpoint.simulated)
                .unwrap()
                .id;
            owner
                .build_missing_memory_vectors(profile, &endpoint.simulated)
                .unwrap();
            owner.sync().unwrap();
        }
        let a = g.mount_phy(a_store).unwrap();
        let b = g.mount_phy(b_store).unwrap();
        let mut cva = Cva::create_project(test_path("private-shared.rel")).unwrap();
        for phy in [&a, &b] {
            cva.publish_entity(
                Some(crate::entity_principal::principal_entity_id(phy)),
                0,
                crate::EntityDraft {
                    canonical_name: phy.clone(),
                    aliases: Vec::new(),
                    kind: "principal".into(),
                    summary: "Durable Phylactery-backed user principal.".into(),
                    mutation_id: Uuid::new_v4().to_string(),
                    created_at_ns: 0,
                    updated_at_ns: 0,
                },
            )
            .unwrap();
        }
        let mut shared = draft("Shared");
        shared.scope = "project".into();
        cva.publish_memory(None, 0, shared).unwrap();
        let profile = cva
            .establish_compatibility_profile(&endpoint.simulated)
            .unwrap()
            .id;
        cva.build_missing_memory_vectors(profile, &endpoint.simulated)
            .unwrap();
        cva.sync().unwrap();
        let rel = g.mount_rel(InteractionRuntime::new(cva)).unwrap();
        // The query consumes an established owner profile; it never establishes one.
        *g.rel_executions
            .read()
            .unwrap()
            .get(&rel)
            .unwrap()
            .owner
            .lock()
            .unwrap()
            .memory_profile
            .lock()
            .unwrap() = Some(profile);
        let dependency = g
            .mount_rel(InteractionRuntime::new(
                Cva::create_project(test_path("private-dependency.rel")).unwrap(),
            ))
            .unwrap();
        let caps = |phy: &str| {
            BTreeMap::from([
                (
                    rel.clone(),
                    BTreeSet::from([
                        OwnerGrant::ViewRel,
                        OwnerGrant::SubmitConversation,
                        OwnerGrant::ManageRel,
                    ]),
                ),
                (phy.into(), BTreeSet::from([OwnerGrant::UseOwnPhy])),
            ])
        };
        let alice = g
            .attach_instance("alice".into(), Some(a.clone()), caps(&a))
            .unwrap();
        let bob = g
            .attach_instance("bob".into(), Some(b.clone()), caps(&b))
            .unwrap();
        g.select_rel(&alice, &rel).unwrap();
        g.select_rel(&bob, &rel).unwrap();
        let original = alice.grants_snapshot().unwrap();
        let mut metadata_grants = original.clone();
        metadata_grants.insert(dependency.clone(), BTreeSet::from([OwnerGrant::ViewRel]));
        g.replace_instance_grants(&alice, metadata_grants).unwrap();
        g.set_rel_metadata_for(&alice, &rel, None, vec![dependency.clone()])
            .unwrap();
        g.replace_instance_grants(&alice, original).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let ready = [&a, &b].iter().all(|phy| {
                let runtime =
                    Arc::clone(&g.phy_executions.read().unwrap().get(*phy).unwrap().runtime);
                let runtime = runtime.lock().unwrap();
                runtime.memory_profile().unwrap().is_some() && runtime.wake_epoch_for_test() >= 2
            });
            if ready {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "personal profile worker did not finish"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        Self {
            g,
            endpoint,
            rel,
            dependency,
            a,
            b,
            alice,
            bob,
            a_memory,
            b_memory,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.endpoint.release();
        self.g.shutdown().unwrap();
    }
}

#[test]
fn combined_search_and_private_reads_isolate_principals_and_dependency_grants() {
    let f = Fixture::new();
    for (instance, own, other, memory, other_memory) in [
        (&f.alice, &f.a, &f.b, f.a_memory, f.b_memory),
        (&f.bob, &f.b, &f.a, f.b_memory, f.a_memory),
    ] {
        let before = f.endpoint.queries.load(Ordering::SeqCst);
        let result =
            f.g.search_visible_memories(instance, "evidence", 5, 3)
                .unwrap();
        assert_eq!(f.endpoint.queries.load(Ordering::SeqCst), before + 1);
        assert_eq!(
            result
                .owners
                .iter()
                .map(|lane| lane.owner.owner_id.as_str())
                .collect::<Vec<_>>(),
            vec![f.rel.as_str(), own.as_str()]
        );
        assert!(
            result.owners.iter().all(|lane| lane.error.is_none()),
            "{result:?}"
        );
        let private = result.owners.last().unwrap().lane.as_ref().unwrap();
        assert!(private.items.iter().any(|item| item.memory.id == memory));
        assert!(
            !private
                .items
                .iter()
                .any(|item| item.memory.id == other_memory)
        );
        assert!(
            f.g.knowledge_memory_for_owner(instance, own, memory)
                .is_ok()
        );
        assert!(
            f.g.knowledge_memory_for_owner(instance, other, other_memory)
                .is_err()
        );
        assert!(
            f.g.semantic_owner(instance, other)
                .unwrap_err()
                .to_string()
                .contains("UseOwnPhy")
        );
        assert!(
            f.g.semantic_owner(instance, "phy-guessed")
                .unwrap_err()
                .to_string()
                .contains("UseOwnPhy")
        );
        assert!(f.g.read_knowledge_for_owner(instance, other).is_err());
        assert!(f.g.entities_for_owner(instance, other).is_err());
        assert!(f.g.entity_stats_for_owner(instance, other).is_err());
        assert!(
            f.g.semantic_graph_relations_for_owner(instance, other)
                .is_err()
        );
        assert!(
            f.g.memory_routing_metadata_for_owner(instance, other, other_memory)
                .is_err()
        );
    }
    let mut grants = f.alice.grants_snapshot().unwrap();
    grants.insert(f.dependency.clone(), BTreeSet::from([OwnerGrant::ViewRel]));
    f.g.replace_instance_grants(&f.alice, grants.clone())
        .unwrap();
    let result =
        f.g.search_visible_memories(&f.alice, "evidence", 5, 3)
            .unwrap();
    assert!(
        result
            .owners
            .iter()
            .any(|lane| lane.owner.owner_id == f.dependency)
    );
    grants.remove(&f.a);
    f.g.replace_instance_grants(&f.alice, grants).unwrap();
    assert!(
        f.g.visible_semantic_owners(&f.alice)
            .unwrap()
            .iter()
            .all(|owner| owner.kind == RuntimeSemanticOwnerKind::Reliquary)
    );
    let result =
        f.g.search_visible_memories(&f.alice, "evidence", 5, 3)
            .unwrap();
    assert!(
        result
            .owners
            .iter()
            .all(|owner| owner.owner.kind == RuntimeSemanticOwnerKind::Reliquary)
    );
    assert!(f.g.phylactery_profile(&f.alice).is_err());
}

#[test]
fn private_search_rejects_revocation_detach_and_replaced_mount_during_embedding() {
    for transition in 0..4 {
        let f = Fixture::new();
        f.endpoint.block.store(true, Ordering::SeqCst);
        let g = f.g.clone();
        let instance = f.alice.clone();
        let query =
            std::thread::spawn(move || g.search_visible_memories(&instance, "evidence", 5, 3));
        f.endpoint.wait_for_query();
        match transition {
            0 | 1 => {
                let mut grants = f.alice.grants_snapshot().unwrap();
                grants.remove(if transition == 0 { &f.a } else { &f.rel });
                f.g.replace_instance_grants(&f.alice, grants).unwrap();
            }
            2 => {
                f.g.detach_instance(&f.alice).unwrap();
            }
            _ => {
                let owner = f.g.unmount_phy(&f.a).unwrap();
                f.g.mount_phy(owner).unwrap();
            }
        }
        f.endpoint.release();
        assert!(
            query.join().unwrap().is_err(),
            "transition {transition} delivered stale data"
        );
        assert!(f.g.phylactery_profile(&f.bob).is_ok());
    }
}

#[test]
fn explicit_profile_projection_updates_only_bound_principal_and_exact_rel() {
    let f = Fixture::new();
    f.g.set_phylactery_profile(&f.alice, Some("Alice One".into()))
        .unwrap();
    f.g.set_phylactery_profile(&f.bob, Some("Bob One".into()))
        .unwrap();
    assert!(
        f.g.refresh_principal_projection_for(&f.alice, &f.rel)
            .unwrap()
    );
    assert!(
        f.g.refresh_principal_projection_for(&f.bob, &f.rel)
            .unwrap()
    );
    f.g.set_phylactery_profile(&f.alice, Some("Alice Two".into()))
        .unwrap();
    assert_eq!(
        f.g.phylactery_profile(&f.bob)
            .unwrap()
            .display_name
            .as_deref(),
        Some("Bob One")
    );
    let rel =
        f.g.authorize(&f.alice, &f.rel, OwnerGrant::SubmitConversation)
            .unwrap();
    let phy =
        f.g.authorize(&f.alice, &f.a, OwnerGrant::UseOwnPhy)
            .unwrap();
    let captured = f.g.with_phy(&phy, |owner| Ok(owner.profile())).unwrap();
    let mut grants = f.alice.grants_snapshot().unwrap();
    grants.insert(f.dependency.clone(), BTreeSet::from([OwnerGrant::ViewRel]));
    f.g.replace_instance_grants(&f.alice, grants.clone())
        .unwrap();
    f.g.select_rel(&f.alice, &f.dependency).unwrap();
    assert!(
        f.g.refresh_principal_projection_for(&f.alice, &f.rel)
            .unwrap()
    );
    let aliases = |owner: &str| {
        f.g.with_rel(&rel, |runtime| {
            Ok(runtime.cva.principal_entity(owner).unwrap().aliases)
        })
        .unwrap()
    };
    assert_eq!(aliases(&f.a), vec!["Alice Two"]);
    assert_eq!(aliases(&f.b), vec!["Bob One"]);
    f.g.set_phylactery_profile(&f.alice, Some("Do Not Publish".into()))
        .unwrap();
    grants.remove(&f.a);
    f.g.replace_instance_grants(&f.alice, grants).unwrap();
    assert!(
        f.g.commit_principal_projection(&f.alice, &rel, &phy, &captured)
            .is_err()
    );
    assert_eq!(aliases(&f.a), vec!["Alice Two"]);
    assert_eq!(aliases(&f.b), vec!["Bob One"]);
    assert!(
        f.g.refresh_principal_projection_for(&f.alice, &f.rel)
            .is_err()
    );
}

#[test]
fn projection_revalidates_both_original_capabilities_and_private_mount() {
    for transition in 0..4 {
        let f = Fixture::new();
        f.g.set_phylactery_profile(&f.alice, Some("Captured".into()))
            .unwrap();
        let rel =
            f.g.authorize(&f.alice, &f.rel, OwnerGrant::SubmitConversation)
                .unwrap();
        let phy =
            f.g.authorize(&f.alice, &f.a, OwnerGrant::UseOwnPhy)
                .unwrap();
        let profile = f.g.with_phy(&phy, |owner| Ok(owner.profile())).unwrap();
        match transition {
            0 | 1 => {
                let mut grants = f.alice.grants_snapshot().unwrap();
                if transition == 0 {
                    grants.remove(&f.a);
                } else {
                    grants
                        .get_mut(&f.rel)
                        .unwrap()
                        .remove(&OwnerGrant::SubmitConversation);
                }
                f.g.replace_instance_grants(&f.alice, grants).unwrap();
            }
            2 => {
                f.g.detach_instance(&f.alice).unwrap();
            }
            _ => {
                let owner = f.g.unmount_phy(&f.a).unwrap();
                f.g.mount_phy(owner).unwrap();
            }
        }
        assert!(
            f.g.commit_principal_projection(&f.alice, &rel, &phy, &profile)
                .is_err()
        );
        let reader = f.g.authorize(&f.bob, &f.rel, OwnerGrant::ViewRel).unwrap();
        f.g.with_rel(&reader, |runtime| {
            assert!(
                runtime
                    .cva
                    .principal_entity(&f.a)
                    .unwrap()
                    .aliases
                    .is_empty()
            );
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn profile_wakes_only_the_exact_phy_and_projection_only_the_rel() {
    let f = Fixture::new();
    let epoch = |phy: &str| {
        f.g.phy_executions
            .read()
            .unwrap()
            .get(phy)
            .unwrap()
            .runtime
            .lock()
            .unwrap()
            .wake_epoch_for_test()
    };
    let a = epoch(&f.a);
    let b = epoch(&f.b);
    f.g.set_phylactery_profile(&f.alice, Some("Alice".into()))
        .unwrap();
    assert_eq!(epoch(&f.a), a + 1);
    assert_eq!(epoch(&f.b), b);
    f.g.refresh_principal_projection_for(&f.alice, &f.rel)
        .unwrap();
    assert_eq!(epoch(&f.a), a + 1);
    assert_eq!(epoch(&f.b), b);
    assert!(
        !f.g.set_phylactery_profile(&f.alice, Some("Alice".into()))
            .unwrap()
    );
    assert_eq!(epoch(&f.a), a + 1);
}

#[test]
fn p5a_semantic_mutations_preserve_phy_privacy_and_replacement_lineage() {
    let f = Fixture::new();
    let memory =
        f.g.create_knowledge_memory_for_owner(
            &f.alice,
            &f.a,
            "Private".into(),
            "source content".into(),
            "knowledge".into(),
            "fact".into(),
            100,
        )
        .unwrap();
    assert_eq!(
        f.g.knowledge_memory_for_owner(&f.alice, &f.a, memory.id)
            .unwrap()
            .title,
        "Private"
    );
    assert!(
        f.g.knowledge_memory_for_owner(&f.bob, &f.a, memory.id)
            .is_err()
    );
    assert!(
        f.g.create_knowledge_memory_for_owner(
            &f.bob,
            &f.a,
            "Forbidden".into(),
            "secret".into(),
            "knowledge".into(),
            "fact".into(),
            101,
        )
        .is_err()
    );
    assert!(
        f.g.create_knowledge_memory_for_owner(
            &f.alice,
            "phy-guessed",
            "Forbidden".into(),
            "secret".into(),
            "knowledge".into(),
            "fact".into(),
            101,
        )
        .is_err()
    );
    let successor =
        f.g.replace_knowledge_memory_for_owner(
            &f.alice,
            &f.a,
            memory.id,
            "Revised".into(),
            "updated".into(),
            102,
        )
        .unwrap();
    let original =
        f.g.knowledge_memory_for_owner(&f.alice, &f.a, memory.id)
            .unwrap();
    assert!(original.archived);
    assert_eq!(original.superseded_by, Some(successor.id));
    assert_eq!(successor.title, "Revised");
    assert!(
        f.g.semantic_graph_relations_for_owner(&f.alice, &f.a)
            .unwrap()
            .iter()
            .any(|r| r.source == successor.id
                && r.target == memory.id
                && r.kind == crate::GraphRelationKind::Supersedes)
    );
    assert!(
        f.g.mutate_knowledge_relation_for_owner(
            &f.bob,
            &f.a,
            successor.id,
            memory.id,
            None,
            Some(crate::GraphRelationKind::Supersedes),
        )
        .is_err()
    );
    f.g.mutate_knowledge_relation_for_owner(
        &f.alice,
        &f.a,
        successor.id,
        memory.id,
        None,
        Some(crate::GraphRelationKind::Supersedes),
    )
    .unwrap();
    assert!(
        f.g.read_memory_provenance_for_owner(&f.alice, &f.a, memory.id)
            .is_err()
    );
    assert!(
        f.g.resolve_knowledge_provenance_for_owner(&f.bob, &f.a, memory.id)
            .is_err()
    );

    let mut grants = f.bob.grants_snapshot().unwrap();
    grants
        .get_mut(&f.rel)
        .unwrap()
        .remove(&OwnerGrant::ManageRel);
    f.g.replace_instance_grants(&f.bob, grants).unwrap();
    assert!(
        f.g.create_knowledge_memory_for_owner(
            &f.bob,
            &f.rel,
            "Denied".into(),
            "content".into(),
            "knowledge".into(),
            "fact".into(),
            103,
        )
        .is_err()
    );
    let rel_memory =
        f.g.create_knowledge_memory_for_owner(
            &f.alice,
            &f.rel,
            "Allowed".into(),
            "content".into(),
            "knowledge".into(),
            "fact".into(),
            104,
        )
        .unwrap();
    assert!(
        f.g.knowledge_memory_for_owner(&f.bob, &f.rel, rel_memory.id)
            .is_ok()
    );
}
