use super::*;
use crate::runtime_host_test_support::test_path;
use crate::{
    Cva, EpisodeId, EpisodePolicy, InsomniaWorkerConfig, InteractionRuntime, MemorySourceRef,
    ReliquaryRuntimeRoutes,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

fn mounted_rel(path: PathBuf, conversation_id: &str, node_id: &str, content: &str) -> Cva {
    let mut cva = Cva::create_project(path).unwrap();
    cva.append_node(
        node_id.into(),
        conversation_id.into(),
        None,
        "user".into(),
        1,
        content,
    )
    .unwrap();
    cva.sync().unwrap();
    cva
}

fn view_grants(owner_ids: &[&str]) -> BTreeMap<String, BTreeSet<OwnerGrant>> {
    owner_ids
        .iter()
        .map(|owner_id| {
            (
                (*owner_id).to_owned(),
                BTreeSet::from([OwnerGrant::ViewRel]),
            )
        })
        .collect()
}

fn gateway() -> GatewayRuntime {
    GatewayRuntime::new(
        test_path("gateway-state"),
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
        EpisodePolicy::default(),
    )
}

#[test]
fn ambient_reads_follow_each_instances_selected_rel_and_explicit_reads_check_grants() {
    let gateway = gateway();
    let rel_a = mounted_rel(
        test_path("ambient-a.rel"),
        "conversation-a",
        "node-a",
        "The bluejay lives near the lake.",
    );
    let rel_b = mounted_rel(
        test_path("ambient-b.rel"),
        "conversation-b",
        "node-b",
        "The fox lives near the forest.",
    );
    let owner_a = gateway.mount_rel(InteractionRuntime::new(rel_a)).unwrap();
    let owner_b = gateway.mount_rel(InteractionRuntime::new(rel_b)).unwrap();

    let first = gateway
        .attach_instance("principal-a".into(), None, view_grants(&[&owner_a]))
        .unwrap();
    let second = gateway
        .attach_instance(
            "principal-b".into(),
            None,
            view_grants(&[&owner_a, &owner_b]),
        )
        .unwrap();

    gateway.select_rel(&first, &owner_a).unwrap();
    gateway.select_rel(&second, &owner_b).unwrap();

    assert_eq!(
        gateway.active_rel_id(&first).unwrap().as_deref(),
        Some(owner_a.as_str())
    );
    assert_eq!(
        gateway.active_rel_id(&second).unwrap().as_deref(),
        Some(owner_b.as_str())
    );

    let first_summaries = gateway.conversation_summaries(&first).unwrap();
    let second_summaries = gateway.conversation_summaries(&second).unwrap();
    assert!(
        first_summaries
            .iter()
            .any(|s| s.conversation_id == "conversation-a")
    );
    assert!(
        !first_summaries
            .iter()
            .any(|s| s.conversation_id == "conversation-b")
    );
    assert!(
        second_summaries
            .iter()
            .any(|s| s.conversation_id == "conversation-b")
    );
    assert!(
        !second_summaries
            .iter()
            .any(|s| s.conversation_id == "conversation-a")
    );

    let first_transcript = gateway
        .conversation_transcript(&first, "conversation-a", "node-a")
        .unwrap();
    let second_transcript = gateway
        .conversation_transcript(&second, "conversation-b", "node-b")
        .unwrap();
    assert_eq!(
        first_transcript[0].content,
        "The bluejay lives near the lake."
    );
    assert_eq!(
        second_transcript[0].content,
        "The fox lives near the forest."
    );

    assert!(
        !gateway
            .search_conversation_branch(&first, "conversation-a", "node-a", "bluejay", 5)
            .unwrap()
            .is_empty()
    );
    assert!(
        !gateway
            .search_conversation_branch(&second, "conversation-b", "node-b", "forest", 5)
            .unwrap()
            .is_empty()
    );
    assert!(
        !gateway
            .search_archive(&first, "bluejay", 5)
            .unwrap()
            .is_empty()
    );
    assert!(
        gateway
            .rel_metadata_for(&first, &owner_b)
            .unwrap_err()
            .to_string()
            .contains("ViewRel")
    );
    assert!(
        gateway
            .search_archive_for(&first, &owner_b, "forest", 5)
            .unwrap_err()
            .to_string()
            .contains("ViewRel")
    );

    let denied = gateway
        .conversation_transcript_for(&first, &owner_b, "conversation-b", "node-b")
        .unwrap_err()
        .to_string();
    assert!(denied.contains("ViewRel"), "{denied}");

    assert!(
        gateway
            .search_conversation_branch_for(
                &first,
                &owner_b,
                "conversation-b",
                "node-b",
                "forest",
                5,
            )
            .unwrap_err()
            .to_string()
            .contains("ViewRel")
    );

    gateway.detach_instance(&first).unwrap();
    let second_after_first_detaches = gateway
        .conversation_transcript(&second, "conversation-b", "node-b")
        .unwrap();
    assert_eq!(
        second_after_first_detaches[0].content,
        "The fox lives near the forest."
    );
    gateway.detach_instance(&second).unwrap();
    drop(gateway.unmount_rel(&owner_a).unwrap());
    drop(gateway.unmount_rel(&owner_b).unwrap());
}

#[test]
fn dependency_closure_excludes_ungranted_rel_and_metadata_cycles_fail_closed() {
    let gateway = gateway();
    let rel_a = mounted_rel(test_path("dependency-a.rel"), "a", "a-node", "A content.");
    let rel_b = mounted_rel(test_path("dependency-b.rel"), "b", "b-node", "B content.");
    let owner_a = gateway.mount_rel(InteractionRuntime::new(rel_a)).unwrap();
    let owner_b = gateway.mount_rel(InteractionRuntime::new(rel_b)).unwrap();

    let admin_grants = BTreeMap::from([
        (
            owner_a.clone(),
            BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
        ),
        (
            owner_b.clone(),
            BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
        ),
    ]);
    let admin = gateway
        .attach_instance("admin".into(), None, admin_grants)
        .unwrap();
    gateway
        .set_rel_metadata_for(&admin, &owner_a, None, vec![owner_b.clone()])
        .unwrap();

    let restricted = gateway
        .attach_instance("restricted".into(), None, view_grants(&[&owner_a]))
        .unwrap();
    gateway.select_rel(&restricted, &owner_a).unwrap();
    assert_eq!(
        gateway.dependency_closure(&restricted).unwrap(),
        vec![owner_a.clone()]
    );

    let cycle = gateway
        .set_rel_metadata_for(&admin, &owner_b, None, vec![owner_a.clone()])
        .unwrap_err()
        .to_string();
    assert!(cycle.contains("cycle"), "{cycle}");

    gateway.detach_instance(&restricted).unwrap();
    gateway.detach_instance(&admin).unwrap();
    drop(gateway.unmount_rel(&owner_a).unwrap());
    drop(gateway.unmount_rel(&owner_b).unwrap());
}

#[test]
fn echo_source_requires_view_grant_for_exact_source_rel() {
    let gateway = gateway();
    let owner_a = gateway
        .mount_rel(InteractionRuntime::new(mounted_rel(
            test_path("ambient-echo-a.rel"),
            "echo-a",
            "echo-node-a",
            "Echo owner A.",
        )))
        .unwrap();
    let owner_b = gateway
        .mount_rel(InteractionRuntime::new(mounted_rel(
            test_path("ambient-echo-b.rel"),
            "echo-b",
            "echo-node-b",
            "Echo owner B.",
        )))
        .unwrap();
    let instance = gateway
        .attach_instance("echo-reader".into(), None, view_grants(&[&owner_a]))
        .unwrap();
    let error = gateway
        .read_echo_source_for(&instance, &owner_b, "echo-b", "echo-node-b", "echo-node-b")
        .unwrap_err()
        .to_string();
    assert!(error.contains("ViewRel"), "{error}");
    gateway.detach_instance(&instance).unwrap();
    drop(gateway.unmount_rel(&owner_a).unwrap());
    drop(gateway.unmount_rel(&owner_b).unwrap());
}

#[test]
fn external_memory_provenance_requires_view_grant_for_exact_source_owner() {
    let gateway = gateway();
    let owner_a = gateway
        .mount_rel(InteractionRuntime::new(mounted_rel(
            test_path("ambient-provenance-a.rel"),
            "prov-a",
            "prov-node-a",
            "Memory owner.",
        )))
        .unwrap();
    let owner_b = gateway
        .mount_rel(InteractionRuntime::new(mounted_rel(
            test_path("ambient-provenance-b.rel"),
            "prov-b",
            "prov-node-b",
            "Source owner.",
        )))
        .unwrap();
    let grants = BTreeMap::from([(
        owner_a.clone(),
        BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
    )]);
    let instance = gateway
        .attach_instance("provenance-reader".into(), None, grants)
        .unwrap();
    let mut memory = gateway
        .create_knowledge_memory_for(
            &instance,
            &owner_a,
            "External evidence".into(),
            "Content".into(),
            "knowledge".into(),
            "fact".into(),
            1,
        )
        .unwrap();
    memory.source_ref = Some(MemorySourceRef {
        owner_id: owner_b.clone(),
        principal_id: None,
        source_episode_id: EpisodeId([4; 32]),
        source_node_id: "prov-node-b".into(),
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
    });
    let error = gateway
        .resolve_external_memory_provenance_for(&instance, &owner_b, memory)
        .unwrap_err()
        .to_string();
    assert!(error.contains("ViewRel"), "{error}");
    gateway.detach_instance(&instance).unwrap();
    drop(gateway.unmount_rel(&owner_a).unwrap());
    drop(gateway.unmount_rel(&owner_b).unwrap());
}

#[test]
fn project_attachment_mutation_requires_manage_project() {
    let gateway = gateway();
    let owner = gateway
        .mount_rel(InteractionRuntime::new(mounted_rel(
            test_path("ambient-project.rel"),
            "project",
            "project-node",
            "Project owner.",
        )))
        .unwrap();
    let instance = gateway
        .attach_instance("project-reader".into(), None, view_grants(&[&owner]))
        .unwrap();
    let error = gateway
        .ingest_project_attachment_for(
            &instance,
            &owner,
            std::path::Path::new("missing-project"),
            std::path::Path::new("missing-attachment"),
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("ManageProject"), "{error}");
    gateway.detach_instance(&instance).unwrap();
    drop(gateway.unmount_rel(&owner).unwrap());
}

#[test]
fn project_attachment_ingest_is_synced_and_survives_reopen() {
    let gateway = gateway();
    let project_dir = test_path("ingest-project");
    std::fs::create_dir_all(&project_dir).unwrap();
    let rel_path = test_path("ingest-project.rel");
    let cva = crate::project_environment::create_project_rel(
        &rel_path,
        &project_dir,
        crate::ProjectRepositoryMode::ManualLore,
    )
    .unwrap();
    let owner = gateway.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let instance = gateway
        .attach_instance(
            "project-manager".into(),
            None,
            BTreeMap::from([(
                owner.clone(),
                BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageProject]),
            )]),
        )
        .unwrap();
    let source = test_path("upload.txt");
    let bytes = b"persisted project attachment";
    std::fs::write(&source, bytes).unwrap();

    let file = gateway
        .ingest_project_attachment_for(&instance, &owner, &project_dir, &source)
        .unwrap();
    let reference = gateway
        .project_file_ref_for(&instance, &owner, file.id)
        .unwrap()
        .unwrap();
    gateway.detach_instance(&instance).unwrap();
    drop(gateway.unmount_rel(&owner).unwrap());

    let reopened = Cva::open_project(&rel_path).unwrap();
    assert_eq!(reopened.project_file_ref(file.id), Some(reference.clone()));
    assert_eq!(
        crate::project_environment::read_project_file(&project_dir, &reference).unwrap(),
        bytes
    );
}

#[derive(Clone)]
struct BlockingEmbedding {
    state: Arc<(Mutex<(bool, bool)>, Condvar)>,
}

impl BlockingEmbedding {
    fn wait_until_started(&self) -> bool {
        let (lock, ready) = &*self.state;
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut state = lock.lock().unwrap();
        while !state.0 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let (next, timeout) = ready.wait_timeout(state, remaining).unwrap();
            state = next;
            if timeout.timed_out() && !state.0 {
                return false;
            }
        }
        true
    }

    fn release(&self) {
        let (lock, ready) = &*self.state;
        let mut state = lock.lock().unwrap();
        state.1 = true;
        ready.notify_all();
    }
}

impl crate::EmbeddingEndpoint for BlockingEmbedding {
    fn dimensions(&self) -> u32 {
        1
    }

    fn normalization(&self) -> crate::VectorNormalization {
        crate::VectorNormalization::None
    }

    fn embed(
        &self,
        _mode: crate::EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, crate::EmbeddingEndpointError> {
        let (lock, ready) = &*self.state;
        let mut state = lock.lock().unwrap();
        state.0 = true;
        ready.notify_all();
        while !state.1 {
            state = ready.wait(state).unwrap();
        }
        Ok(inputs.iter().map(|_| vec![1.0]).collect())
    }
}

#[test]
fn revocation_during_embedding_invalidates_the_original_owner_token() {
    let embedding = BlockingEmbedding {
        state: Arc::new((Mutex::new((false, false)), Condvar::new())),
    };
    let endpoint = Arc::new(embedding.clone());
    let gateway = Arc::new(GatewayRuntime::new(
        test_path("gateway-embedding-state"),
        ReliquaryRuntimeRoutes::new(None, None, None, None, Some(endpoint)),
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
        EpisodePolicy::default(),
    ));
    let owner = gateway
        .mount_rel(InteractionRuntime::new(mounted_rel(
            test_path("embedding-owner.rel"),
            "embedding",
            "embedding-node",
            "Embedding owner.",
        )))
        .unwrap();
    let instance = gateway
        .attach_instance("embedding-reader".into(), None, view_grants(&[&owner]))
        .unwrap();
    let worker_gateway = Arc::clone(&gateway);
    let worker_instance = Arc::clone(&instance);
    let worker_owner = owner.clone();
    let query = std::thread::spawn(move || {
        worker_gateway.search_memories_by_text_for(
            &worker_instance,
            &worker_owner,
            crate::CompatibilityProfileId([0; 32]),
            "query",
            crate::MemoryRetrievalConfig::default(),
        )
    });

    let started = embedding.wait_until_started();
    if !started {
        embedding.release();
        let _ = query.join();
        panic!("embedding provider did not start");
    }
    gateway
        .replace_instance_grants(&instance, BTreeMap::new())
        .unwrap();
    embedding.release();
    let error = query.join().unwrap().unwrap_err().to_string();
    assert!(
        error.contains("authorization is stale")
            || error.contains("instance lacks ViewRel access")
            || error.contains("runtime admission is fenced"),
        "{error}"
    );
    gateway.detach_instance(&instance).unwrap();
    drop(gateway.unmount_rel(&owner).unwrap());
}

#[test]
fn instance_delivery_leases_drain_before_revoke_and_detach() {
    use std::sync::mpsc;
    for detach in [false, true] {
        let gateway = Arc::new(gateway());
        let owner = gateway
            .mount_rel(InteractionRuntime::new(mounted_rel(
                test_path("delivery-lease.rel"),
                "delivery",
                "delivery-node",
                "Delivery owner.",
            )))
            .unwrap();
        let instance = gateway
            .attach_instance("delivery-reader".into(), None, view_grants(&[&owner]))
            .unwrap();
        let authorization = instance.authorization().unwrap();
        let gate = Arc::clone(&gateway.attachment_entry(&instance).unwrap().leases);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker_gateway = Arc::clone(&gateway);
        let worker_instance = Arc::clone(&instance);
        let worker_owner = owner.clone();
        let delivery = std::thread::spawn(move || {
            worker_gateway.with_instance_delivery(&worker_instance, authorization, || {
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                Ok(vec![worker_owner])
            })
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let (done_tx, done_rx) = mpsc::channel();
        let transition_gateway = Arc::clone(&gateway);
        let transition_instance = Arc::clone(&instance);
        let transition = std::thread::spawn(move || {
            let result = if detach {
                transition_gateway
                    .detach_instance(&transition_instance)
                    .map(|_| ())
            } else {
                transition_gateway.replace_instance_grants(&transition_instance, BTreeMap::new())
            };
            done_tx.send(result).unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while gate.snapshot().unwrap().0 != crate::OwnerAdmission::Closed {
            if Instant::now() >= deadline {
                release_tx.send(()).unwrap();
                panic!("delivery transition did not close admission");
            }
            std::thread::yield_now();
        }
        assert!(matches!(done_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(
            gateway
                .with_instance_delivery(&instance, authorization, || Ok(()))
                .is_err()
        );
        release_tx.send(()).unwrap();
        assert_eq!(delivery.join().unwrap().unwrap(), vec![owner.clone()]);
        done_rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap();
        transition.join().unwrap();
        assert!(
            gateway
                .with_instance_delivery(&instance, authorization, || Ok(()))
                .is_err()
        );
        if !detach {
            gateway.detach_instance(&instance).unwrap();
        }
        drop(gate);
        drop(gateway.unmount_rel(&owner).unwrap());
    }
}

#[test]
fn detach_during_embedding_denies_delivery_without_holding_owner_locks() {
    let embedding = BlockingEmbedding {
        state: Arc::new((Mutex::new((false, false)), Condvar::new())),
    };
    let gateway = Arc::new(GatewayRuntime::new(
        test_path("detach-embedding-state"),
        ReliquaryRuntimeRoutes::new(None, None, None, None, Some(Arc::new(embedding.clone()))),
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
        EpisodePolicy::default(),
    ));
    let owner = gateway
        .mount_rel(InteractionRuntime::new(mounted_rel(
            test_path("detach-embedding.rel"),
            "embedding",
            "node",
            "Embedding owner.",
        )))
        .unwrap();
    let instance = gateway
        .attach_instance(
            "detached-embedding-reader".into(),
            None,
            view_grants(&[&owner]),
        )
        .unwrap();
    let worker_gateway = Arc::clone(&gateway);
    let worker_instance = Arc::clone(&instance);
    let worker_owner = owner.clone();
    let query = std::thread::spawn(move || {
        worker_gateway.search_memories_by_text_for(
            &worker_instance,
            &worker_owner,
            crate::CompatibilityProfileId([0; 32]),
            "query",
            crate::MemoryRetrievalConfig::default(),
        )
    });
    if !embedding.wait_until_started() {
        embedding.release();
        let _ = query.join();
        panic!("embedding provider did not start");
    }
    let execution = Arc::clone(
        &gateway
            .rel_executions
            .read()
            .unwrap()
            .get(&owner)
            .unwrap()
            .owner,
    );
    let runtime = {
        let execution = execution
            .try_lock()
            .expect("embedding held owner execution");
        Arc::clone(execution.runtime.as_ref().unwrap())
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if runtime.try_lock().is_ok() {
            break;
        }
        if Instant::now() >= deadline {
            embedding.release();
            panic!("embedding held semantic runtime");
        }
        std::thread::yield_now();
    }
    gateway.detach_instance(&instance).unwrap();
    embedding.release();
    let error = query.join().unwrap().unwrap_err().to_string();
    assert!(
        error.contains("not attached") || error.contains("detached") || error.contains("fenced"),
        "{error}"
    );
    drop(runtime);
    drop(execution);
    drop(gateway.unmount_rel(&owner).unwrap());
}
