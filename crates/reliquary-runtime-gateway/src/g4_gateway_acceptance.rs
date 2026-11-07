//! Joined G4-T05 evidence for durable Gateway queue, trusted host, and workers.
//!
//! This module is test-only and is wired by the crate root when the G4 slice is
//! accepted. It composes the existing Gateway seams without adding product API.

#[cfg(test)]
use super::*;
use crate::owner_workers::{OwnerWorkerDriver, OwnerWorkerPoll};
use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
use reliquary_vector_store::{
    ArchiveVectorStore, CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore,
    VectorGenerationStore,
};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, Barrier, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(format!("g4-gateway-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
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

#[derive(Default)]
struct LocalHost {
    reconcile_unknown: Mutex<bool>,
    operation_status: Mutex<std::collections::HashMap<String, HostOperationStatus>>,
    handoffs: AtomicUsize,
}
impl LocalHost {
    fn set_status(&self, stable_id: &str, status: HostOperationStatus) {
        self.operation_status
            .lock()
            .unwrap()
            .insert(stable_id.to_owned(), status);
    }
}
impl TrustedHostAdapter for LocalHost {
    fn configured_capabilities(&self) -> BTreeSet<RuntimeCapability> {
        [RuntimeCapability::ConversationExecution]
            .into_iter()
            .collect()
    }
    fn connect(&self) -> Result<HostConnection, String> {
        Ok(HostConnection {
            capabilities: [(
                RuntimeCapability::ConversationExecution,
                HostCapabilityContract::pending_queue(
                    self.queue_guarantees(RuntimeCapability::ConversationExecution),
                ),
            )]
            .into_iter()
            .collect(),
        })
    }
    fn queue_guarantees(&self, _: RuntimeCapability) -> HostQueueGuarantees {
        HostQueueGuarantees {
            durable_acceptance: true,
            stable_submission_ids: true,
            linearizable_cancel_dequeue: true,
            queryable_operation_status: true,
            idempotent_handoff: true,
        }
    }
    fn reconcile(&self, _: RuntimeCapability) -> Result<HostReconcileStatus, String> {
        if *self.reconcile_unknown.lock().unwrap() {
            Ok(HostReconcileStatus::UnknownStatus)
        } else {
            Ok(HostReconcileStatus::NoOutstandingWork)
        }
    }
    fn handoff(
        &self,
        _: RuntimeCapability,
        operation: &HostOperation,
    ) -> Result<HandoffReceipt, String> {
        self.handoffs.fetch_add(1, Ordering::SeqCst);
        Ok(HandoffReceipt {
            stable_id: operation.stable_id.clone(),
            payload_digest: operation.payload_digest,
            durably_accepted: true,
        })
    }
    fn status(&self, _: RuntimeCapability, stable_id: &str) -> Result<HostOperationStatus, String> {
        Ok(self
            .operation_status
            .lock()
            .unwrap()
            .get(stable_id)
            .copied()
            .unwrap_or(HostOperationStatus::Unknown))
    }
    fn cancel(&self, _: RuntimeCapability, _: &str) -> Result<HostCancelOutcome, String> {
        Ok(HostCancelOutcome::Unknown)
    }
}

struct Poller(Arc<AtomicUsize>);
impl OwnerWorkerDriver for Poller {
    fn kind(&self) -> OwnerWorkerKind {
        OwnerWorkerKind::Freshness
    }
    fn poll(&self) -> Result<OwnerWorkerPoll, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(OwnerWorkerPoll::Idle)
    }
}

fn wait_for_poll(counter: &AtomicUsize, prior: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while counter.load(Ordering::SeqCst) <= prior && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(2));
    }
    assert!(
        counter.load(Ordering::SeqCst) > prior,
        "worker did not poll"
    );
}

fn run_freshness_worker(
    gateway: &Gateway,
    instance: Arc<Instance>,
    owner: OwnerId,
) -> (u64, Vec<reliquary_application::FreshnessTransition>) {
    let freshness = Arc::new(FreshnessWorker::new(instance.clone(), owner, 8));
    let mut workers = gateway
        .start_owner_workflows(
            instance,
            owner,
            OwnerWorkflowSet::new().freshness(freshness.clone()),
            OwnerWorkerConfig {
                idle_poll: Duration::from_millis(2),
            },
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let evaluated = loop {
        if let Some(turn) = freshness.evaluated_turn().unwrap() {
            break turn;
        }
        assert!(
            Instant::now() < deadline,
            "Freshness worker did not evaluate pending owner cut"
        );
        thread::sleep(Duration::from_millis(2));
    };
    let transitions = freshness.transitions().unwrap();
    workers.stop().unwrap();
    (evaluated, transitions)
}

#[test]
fn g4_joined_gateway_queue_host_stream_reopen_and_worker_lifecycle() {
    let temp = Temp::new();
    let owner_path = temp.0.join("owner.rel");
    let cascade_path = temp.0.join("rel-cascade.rel");
    let phy_a_path = temp.0.join("phy-reader-a.phy");
    let phy_b_path = temp.0.join("phy-reader-b.phy");
    let raw_owner = [115; 16];
    let cascade_raw = [116; 16];
    let phy_a_raw = [117; 16];
    let phy_b_raw = [118; 16];
    initialize_rel(&owner_path, raw_owner);
    drop(RelWriter::create_path(&cascade_path, ReliquaryScope::Connection, cascade_raw).unwrap());
    drop(PhyWriter::create_path(&phy_a_path, phy_a_raw).unwrap());
    drop(PhyWriter::create_path(&phy_b_path, phy_b_raw).unwrap());

    run_lifecycle(
        &temp.0,
        [&owner_path, &cascade_path, &phy_a_path, &phy_b_path],
        true,
    );
}

pub(crate) fn complete_cohort_lifecycle(root: &Path, paths: [&Path; 4]) -> serde_json::Value {
    run_lifecycle(root, paths, false)
}

fn run_lifecycle(root: &Path, paths: [&Path; 4], seed_manifest: bool) -> serde_json::Value {
    let [owner_path, cascade_path, phy_a_path, phy_b_path] = paths;
    let queue_path = root.join("lifecycle-queue");
    let raw_owner = RelSession::open_path_current(owner_path)
        .unwrap()
        .owner_id();
    let cascade_raw = RelSession::open_path_current(cascade_path)
        .unwrap()
        .owner_id();
    let phy_a_raw = PhySession::open_path_current(phy_a_path)
        .unwrap()
        .owner_id();
    let phy_b_raw = PhySession::open_path_current(phy_b_path)
        .unwrap()
        .owner_id();
    let owner = OwnerId::from_bytes(raw_owner);

    let gateway = Gateway::new();
    let mounted = gateway
        .mount_writable_reliquary(
            &owner_path,
            &GraphAdmission::for_replay(raw_owner, [], [], [], [], []),
            [],
            [],
        )
        .unwrap();
    let cascade_admission = GraphAdmission::for_replay(cascade_raw, [], [], [], [], []);
    let phy_a_admission = GraphAdmission::for_replay(phy_a_raw, [], [], [], [], []);
    let phy_b_admission = GraphAdmission::for_replay(phy_b_raw, [], [], [], [], []);
    let cascade = gateway
        .mount_writable_reliquary(&cascade_path, &cascade_admission, [], [])
        .unwrap();
    let phy_a = gateway
        .mount_writable_phylactery(&phy_a_path, &phy_a_admission, [])
        .unwrap();
    let phy_b = gateway
        .mount_writable_phylactery(&phy_b_path, &phy_b_admission, [])
        .unwrap();
    let alice_token = "g4 principal alice token with sufficient entropy";
    let bob_token = "g4 principal bob token with sufficient entropy";
    let (admission, control) = TrustedHostAdmission::new([
        HostCredentialBinding::new("principal-a", alice_token, [mounted, cascade], [phy_a]),
        HostCredentialBinding::new("principal-b", bob_token, [mounted, cascade], [phy_b]),
    ])
    .install(&gateway)
    .unwrap();
    let alice = Arc::new(admission.authenticate(alice_token).unwrap());
    let bob = admission.authenticate(bob_token).unwrap();
    let manifest = seed_manifest.then(|| {
        crate::g4_test_cohort::replay_authored_manifest(
            &alice,
            root,
            mounted,
            cascade,
            "principal-a",
        )
    });
    let original_turns = alice
        .with_writable_reliquary(mounted, |rel| rel.accepted_turn_sources().len())
        .unwrap();
    let cascade_turns = alice
        .with_writable_reliquary(cascade, |rel| rel.accepted_turn_sources().len())
        .unwrap();
    let mut queue = alice.open_conversation_queue(&queue_path, mounted).unwrap();

    // The finalized branch is durable and exclusive against later cancellation.
    let accepted = ConversationSubmissionId::new(
        "g4-conversation",
        SubmissionId::new("accepted-turn").unwrap(),
    )
    .unwrap();
    alice
        .submit_conversation(
            &mut queue,
            accepted.conversation_id.clone(),
            accepted.submission_id.clone(),
            "accepted Archive text",
            0,
        )
        .unwrap();
    assert!(matches!(
        bob.cancel_own_pending_submission(&mut queue, &accepted, 1),
        Err(GatewayError::UnauthorizedQueueSubmission { .. })
    ));
    let finalized = alice
        .finalize_conversation_submission(
            &mut queue,
            &accepted,
            1,
            FinalizeCapture {
                branch_id: "main".into(),
                parent_node_id: None,
                committed_timestamp_ns: 100,
                verified_principal_id: "principal-a".into(),
            },
        )
        .unwrap();
    assert_eq!(finalized.state, QueueState::Committed);
    assert!(matches!(
        alice.cancel_own_pending_submission(&mut queue, &accepted, 2),
        Err(GatewayError::ConversationQueue(QueueError::InvalidState))
    ));

    // A different submission wins cancel; finalization cannot publish it.
    let cancelled = ConversationSubmissionId::new(
        "g4-conversation",
        SubmissionId::new("cancelled-turn").unwrap(),
    )
    .unwrap();
    alice
        .submit_conversation(
            &mut queue,
            cancelled.conversation_id.clone(),
            cancelled.submission_id.clone(),
            "must stay out of Archive",
            3,
        )
        .unwrap();
    assert!(matches!(
        bob.cancel_own_pending_submission(&mut queue, &cancelled, 4),
        Err(GatewayError::UnauthorizedQueueSubmission { .. })
    ));
    assert_eq!(
        alice
            .cancel_own_pending_submission(&mut queue, &cancelled, 4)
            .unwrap()
            .state,
        QueueState::Cancelled
    );
    assert!(
        alice
            .finalize_conversation_submission(
                &mut queue,
                &cancelled,
                4,
                FinalizeCapture {
                    branch_id: "main".into(),
                    parent_node_id: None,
                    committed_timestamp_ns: 101,
                    verified_principal_id: "principal-a".into(),
                }
            )
            .is_err()
    );

    let race_queue = Arc::new(Mutex::new(
        alice
            .open_conversation_queue(root.join("lifecycle-race-queue"), mounted)
            .unwrap(),
    ));
    let raced =
        ConversationSubmissionId::new("g4-race", SubmissionId::new("exclusive-turn").unwrap())
            .unwrap();
    alice
        .submit_conversation(
            &mut race_queue.lock().unwrap(),
            raced.conversation_id.clone(),
            raced.submission_id.clone(),
            "only finalize or cancel may win",
            0,
        )
        .unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let finalize_thread = {
        let alice = Arc::clone(&alice);
        let queue = Arc::clone(&race_queue);
        let raced = raced.clone();
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            alice
                .finalize_conversation_submission(
                    &mut queue.lock().unwrap(),
                    &raced,
                    1,
                    FinalizeCapture {
                        branch_id: "main".into(),
                        parent_node_id: None,
                        committed_timestamp_ns: 102,
                        verified_principal_id: "principal-a".into(),
                    },
                )
                .is_ok()
        })
    };
    let cancel_thread = {
        let alice = Arc::clone(&alice);
        let queue = Arc::clone(&race_queue);
        let raced = raced.clone();
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            alice
                .cancel_own_pending_submission(&mut queue.lock().unwrap(), &raced, 1)
                .is_ok()
        })
    };
    barrier.wait();
    let finalize_won = finalize_thread.join().unwrap();
    let cancel_won = cancel_thread.join().unwrap();
    assert_ne!(finalize_won, cancel_won);
    let race_state = alice
        .conversation_submission_status(&race_queue.lock().unwrap(), &raced)
        .unwrap()
        .unwrap()
        .state;
    assert_eq!(race_state == QueueState::Committed, finalize_won);
    assert_eq!(race_state == QueueState::Cancelled, cancel_won);
    drop(race_queue);

    // Checkpoints become visible only after the queue has durably recorded them.
    let held = ConversationSubmissionId::new(
        "g4-conversation",
        SubmissionId::new("host-held-turn").unwrap(),
    )
    .unwrap();
    let held_revision = queue.conversation_revision(&held.conversation_id);
    alice
        .submit_conversation(
            &mut queue,
            held.conversation_id.clone(),
            held.submission_id.clone(),
            "provisional host input",
            held_revision,
        )
        .unwrap();
    queue
        .begin_generation(
            &held,
            "operation-g4".into(),
            "generation-g4".into(),
            "fork".into(),
            "accepted-turn".into(),
            5,
        )
        .unwrap();
    let (initial, mut stream) = alice.subscribe_generation_stream(&queue, &held).unwrap();
    assert_eq!(initial.watermark, 0);
    assert_eq!(
        alice
            .next_generation_stream_delivery(&queue, &mut stream)
            .unwrap(),
        None
    );
    queue
        .record_generation_checkpoint(&held, 1, "private provisional prefix".into())
        .unwrap();
    let Some(GenerationStreamDelivery::Event(event)) = alice
        .next_generation_stream_delivery(&queue, &mut stream)
        .unwrap()
    else {
        panic!("durable checkpoint was not delivered");
    };
    assert!(matches!(
        event.payload,
        GenerationStreamPayload::Checkpoint(DurableGenerationCheckpoint {
            content_prefix,
            ..
        }) if content_prefix == "private provisional prefix"
    ));

    // Another Instance cannot reuse Alice's stream subscription or hand off her work.
    assert!(matches!(
        bob.next_generation_stream_delivery(&queue, &mut stream),
        Err(GatewayError::InvalidGenerationSubscription)
    ));
    let host = Arc::new(LocalHost::default());
    let router = admission.connect_runtime_host(host.clone());
    assert!(matches!(
        router.state(RuntimeCapability::ConversationExecution),
        HostCapabilityState::Ready
    ));
    assert!(matches!(
        bob.dispatch_conversation_submission(
            &mut queue,
            &held,
            RuntimeCapability::ConversationExecution,
            &router,
        ),
        Err(GatewayError::UnauthorizedQueueSubmission { .. })
    ));
    let dispatched = alice
        .dispatch_conversation_submission(
            &mut queue,
            &held,
            RuntimeCapability::ConversationExecution,
            &router,
        )
        .unwrap();
    assert!(matches!(dispatched, HostDispatchOutcome::Dispatched(_)));
    assert_eq!(host.handoffs.load(Ordering::SeqCst), 1);

    // Bob owns a separate accepted host operation. The host completes that
    // exact stable operation after Gateway shutdown; a fresh valid Bob
    // Instance must reconcile its durable receipt without a second handoff.
    let resumable = ConversationSubmissionId::new(
        "g4-conversation",
        SubmissionId::new("host-resume-turn").unwrap(),
    )
    .unwrap();
    let resume_revision = queue.conversation_revision(&resumable.conversation_id);
    bob.submit_conversation(
        &mut queue,
        resumable.conversation_id.clone(),
        resumable.submission_id.clone(),
        "durable host work to resume after Gateway restart",
        resume_revision,
    )
    .unwrap();
    let resume_dispatched = bob
        .dispatch_conversation_submission(
            &mut queue,
            &resumable,
            RuntimeCapability::ConversationExecution,
            &router,
        )
        .unwrap();
    assert!(matches!(
        resume_dispatched,
        HostDispatchOutcome::Dispatched(_)
    ));
    assert!(matches!(
        alice.dispatch_conversation_submission(
            &mut queue,
            &resumable,
            RuntimeCapability::ConversationExecution,
            &router,
        ),
        Err(GatewayError::UnauthorizedQueueSubmission { .. })
    ));
    let pre_restart_operation = bob
        .host_operation_status(&queue, &resumable, RuntimeCapability::ConversationExecution)
        .unwrap()
        .unwrap();
    assert_eq!(
        pre_restart_operation.phase,
        DurableHostOperationPhase::Accepted
    );
    host.set_status(
        &pre_restart_operation.stable_id,
        HostOperationStatus::Completed,
    );
    assert_eq!(host.handoffs.load(Ordering::SeqCst), 2);
    *host.reconcile_unknown.lock().unwrap() = true;
    router.reconnect();
    assert_eq!(
        router.state(RuntimeCapability::ConversationExecution),
        HostCapabilityState::UnknownStatusHeld
    );
    let reconciled = alice
        .reconcile_host_operation(
            &mut queue,
            &held,
            RuntimeCapability::ConversationExecution,
            &router,
        )
        .unwrap();
    assert!(matches!(reconciled, HostDispatchOutcome::Reconciled(_)));
    assert_eq!(
        alice
            .host_operation_status(&queue, &held, RuntimeCapability::ConversationExecution)
            .unwrap()
            .unwrap()
            .phase,
        DurableHostOperationPhase::Held
    );
    assert_eq!(host.handoffs.load(Ordering::SeqCst), 2);
    assert!(
        !alice
            .with_writable_reliquary(mounted, |rel| {
                rel.archive_node("g4-conversation", "host-held-turn")
                    .is_some()
            })
            .unwrap()
    );

    // Only one worker of a specialty can hold its owner lease; stop joins it,
    // and the released lease can be acquired again after restart.
    let instance = Arc::new(admission.authenticate(alice_token).unwrap());
    let polls = Arc::new(AtomicUsize::new(0));
    let make_driver = || -> Arc<dyn OwnerWorkerDriver> { Arc::new(Poller(Arc::clone(&polls))) };
    let worker_config = OwnerWorkerConfig {
        idle_poll: Duration::from_millis(2),
    };
    let mut workers = gateway
        .start_owner_workers(
            Arc::clone(&instance),
            mounted,
            [make_driver()],
            worker_config.clone(),
        )
        .unwrap();
    wait_for_poll(&polls, 0);
    assert!(
        gateway
            .start_owner_workers(
                Arc::clone(&instance),
                mounted,
                [make_driver()],
                worker_config.clone(),
            )
            .is_err()
    );
    let before_stop = polls.load(Ordering::SeqCst);
    workers.stop().unwrap();
    assert!(workers.is_stopped());
    let after_stop = polls.load(Ordering::SeqCst);
    thread::sleep(Duration::from_millis(10));
    assert_eq!(polls.load(Ordering::SeqCst), after_stop);
    assert!(after_stop >= before_stop);
    let mut restarted = gateway
        .start_owner_workers(
            Arc::clone(&instance),
            mounted,
            [make_driver()],
            worker_config,
        )
        .unwrap();
    wait_for_poll(&polls, after_stop);
    restarted.stop().unwrap();

    // Exercise the real durable-owner workflow, which was idle in the
    // scheduler-only Poller test. Its work key is the persisted REL turn cut.
    let (freshness_cut, freshness_transitions) =
        run_freshness_worker(&gateway, Arc::clone(&instance), mounted);

    // Revocation is checked on access for an already authenticated Instance.
    control.revoke(alice_token).unwrap();
    assert!(matches!(
        alice.generation_checkpoint_snapshot(&queue, &held),
        Err(GatewayError::CredentialRevoked)
    ));
    assert!(matches!(
        admission.authenticate(alice_token),
        Err(GatewayError::CredentialRevoked)
    ));

    drop(manifest);
    drop(queue);
    drop(instance);
    drop((alice, bob, admission, control, gateway));

    let queue_journal = std::fs::read_dir(queue_path.join("conversation-queues"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "rqj"))
        .unwrap();
    let complete_journal_bytes = std::fs::metadata(&queue_journal).unwrap().len();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&queue_journal)
        .unwrap()
        .write_all(&[0x04, 0x00])
        .unwrap();
    assert!(std::fs::metadata(&queue_journal).unwrap().len() > complete_journal_bytes);

    // Fresh Gateway, owner session, and queue readers observe receipts and no
    // canceled or host-private provisional text in Archive.
    let reopened_gateway = Gateway::new();
    let reopened_owner = reopened_gateway
        .mount_writable_reliquary(
            &owner_path,
            &GraphAdmission::for_replay(raw_owner, [], [], [], [], []),
            [],
            [],
        )
        .unwrap();
    reopened_gateway
        .mount_reliquary(&cascade_path, &cascade_admission, [], [])
        .unwrap();
    reopened_gateway
        .mount_phylactery(&phy_a_path, &phy_a_admission, [])
        .unwrap();
    reopened_gateway
        .mount_phylactery(&phy_b_path, &phy_b_admission, [])
        .unwrap();
    assert_eq!(reopened_owner, owner);
    let reopened = reopened_gateway
        .instance("principal-a", [reopened_owner], [])
        .unwrap();
    let recovery_token = "g4 principal bob recovery token with sufficient entropy";
    let (recovery_admission, recovery_control) =
        TrustedHostAdmission::new([HostCredentialBinding::new(
            "principal-b",
            recovery_token,
            [reopened_owner],
            [phy_b],
        )])
        .install(&reopened_gateway)
        .unwrap();
    let recovered_bob = Arc::new(recovery_admission.authenticate(recovery_token).unwrap());
    let (recovered_freshness_cut, recovered_freshness_transitions) = run_freshness_worker(
        &reopened_gateway,
        Arc::clone(&recovered_bob),
        reopened_owner,
    );
    assert_eq!(recovered_freshness_cut, freshness_cut);
    assert_eq!(recovered_freshness_transitions, freshness_transitions);
    let mut reopened_queue = reopened
        .open_conversation_queue(&queue_path, reopened_owner)
        .unwrap();
    assert_eq!(
        std::fs::metadata(&queue_journal).unwrap().len(),
        complete_journal_bytes
    );
    assert_eq!(
        reopened
            .conversation_submission_status(&reopened_queue, &accepted)
            .unwrap()
            .unwrap()
            .state,
        QueueState::Committed
    );
    assert_eq!(
        reopened
            .conversation_submission_status(&reopened_queue, &cancelled)
            .unwrap()
            .unwrap()
            .state,
        QueueState::Cancelled
    );
    assert_eq!(
        reopened
            .generation_checkpoint_snapshot(&reopened_queue, &held)
            .unwrap()
            .unwrap()
            .content_prefix,
        "private provisional prefix"
    );
    let replay = reopened
        .replay_generation_checkpoints(&reopened_queue, &held, 0, 8)
        .unwrap();
    let GenerationReplay::Checkpoints { items, .. } = replay else {
        panic!("fresh queue reader could not replay the durable checkpoint");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].content_prefix, "private provisional prefix");
    assert_eq!(
        reopened
            .host_operation_status(
                &reopened_queue,
                &held,
                RuntimeCapability::ConversationExecution
            )
            .unwrap()
            .unwrap()
            .phase,
        DurableHostOperationPhase::Held
    );
    let recovery_router = recovery_admission.connect_runtime_host(host.clone());
    *host.reconcile_unknown.lock().unwrap() = false;
    let resumed = recovered_bob
        .dispatch_conversation_submission(
            &mut reopened_queue,
            &resumable,
            RuntimeCapability::ConversationExecution,
            &recovery_router,
        )
        .unwrap();
    assert!(matches!(resumed, HostDispatchOutcome::Reconciled(_)));
    let resumed_operation = recovered_bob
        .host_operation_status(
            &reopened_queue,
            &resumable,
            RuntimeCapability::ConversationExecution,
        )
        .unwrap()
        .unwrap();
    assert_eq!(resumed_operation.stable_id, pre_restart_operation.stable_id);
    assert_eq!(
        resumed_operation.payload_digest,
        pre_restart_operation.payload_digest
    );
    assert_eq!(
        resumed_operation.phase,
        DurableHostOperationPhase::Completed
    );
    assert_eq!(host.handoffs.load(Ordering::SeqCst), 2);
    let repeated = recovered_bob
        .dispatch_conversation_submission(
            &mut reopened_queue,
            &resumable,
            RuntimeCapability::ConversationExecution,
            &recovery_router,
        )
        .unwrap();
    assert!(matches!(repeated, HostDispatchOutcome::Reconciled(_)));
    assert_eq!(host.handoffs.load(Ordering::SeqCst), 2);
    drop(reopened_queue);
    let receipt_reopen = recovered_bob
        .open_conversation_queue(&queue_path, reopened_owner)
        .unwrap();
    let durable_resume = recovered_bob
        .host_operation_status(
            &receipt_reopen,
            &resumable,
            RuntimeCapability::ConversationExecution,
        )
        .unwrap()
        .unwrap();
    assert_eq!(durable_resume.stable_id, pre_restart_operation.stable_id);
    assert_eq!(
        durable_resume.payload_digest,
        pre_restart_operation.payload_digest
    );
    assert_eq!(durable_resume.phase, DurableHostOperationPhase::Completed);
    assert_eq!(
        recovered_bob
            .host_operation_status(
                &receipt_reopen,
                &held,
                RuntimeCapability::ConversationExecution
            )
            .unwrap()
            .unwrap()
            .phase,
        DurableHostOperationPhase::Held
    );
    recovery_control.revoke(recovery_token).unwrap();
    assert!(matches!(
        recovered_bob.host_operation_status(
            &receipt_reopen,
            &resumable,
            RuntimeCapability::ConversationExecution
        ),
        Err(GatewayError::CredentialRevoked)
    ));
    drop(receipt_reopen);
    let (accepted_turns, archive_presence) = reopened
        .with_writable_reliquary(reopened_owner, |rel| {
            (
                rel.accepted_turn_sources().len(),
                [
                    "accepted-turn",
                    "cancelled-turn",
                    "host-held-turn",
                    "host-resume-turn",
                ]
                .map(|id| rel.archive_node("g4-conversation", id).is_some()),
            )
        })
        .unwrap();
    assert_eq!(
        accepted_turns,
        original_turns + if finalize_won { 2 } else { 1 }
    );
    assert_eq!(archive_presence, [true, false, false, false]);
    assert_eq!(
        reopened
            .with_writable_reliquary(reopened_owner, |rel| {
                rel.archive_node("g4-race", "exclusive-turn").is_some()
            })
            .unwrap(),
        finalize_won
    );
    let cascade_session = RelSession::open_path(&cascade_path, &cascade_admission, [], []).unwrap();
    let phy_a_session = PhySession::open_path(&phy_a_path, &phy_a_admission, []).unwrap();
    let phy_b_session = PhySession::open_path(&phy_b_path, &phy_b_admission, []).unwrap();
    assert_eq!(cascade_session.accepted_turn_sources().len(), cascade_turns);
    assert_eq!(phy_a_session.owner_id(), phy_a_raw);
    assert_eq!(phy_b_session.owner_id(), phy_b_raw);

    let reopened_token = "outsider token should not have owner grant";
    let (outsider_admission, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "outsider",
        reopened_token,
        [],
        [],
    )])
    .install(&reopened_gateway)
    .unwrap();
    let outsider = outsider_admission.authenticate(reopened_token).unwrap();
    assert!(matches!(
        outsider.open_conversation_queue(&queue_path, reopened_owner),
        Err(GatewayError::UnauthorizedRel { .. })
    ));
    serde_json::json!({
        "original_turns": original_turns,
        "reopened_turns": accepted_turns,
        "race_finalize_won": finalize_won,
        "accepted_present": archive_presence[0],
        "cancelled_present": archive_presence[1],
        "held_present": archive_presence[2],
        "resumed_host_text_present": archive_presence[3],
        "checkpoint_replayed": items.len(),
        "journal_tail_recovered": true,
        "worker_restart_verified": true,
        "recovered_host_operation": resumed_operation.stable_id,
        "recovered_host_payload_digest": crate::hex(&resumed_operation.payload_digest),
        "recovered_host_operation_phase": "completed",
        "host_handoffs_after_retry": host.handoffs.load(Ordering::SeqCst),
        "held_operation_phase": "held",
        "freshness_cut_resumed": recovered_freshness_cut,
        "freshness_transitions_resumed": recovered_freshness_transitions.len()
    })
}
