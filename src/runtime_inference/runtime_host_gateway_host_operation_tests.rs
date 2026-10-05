use super::*;
#[path = "runtime_host_gateway_conversation_branch_host_tests.rs"]
mod branches;
use crate::runtime_host_test_support::test_path;
use crate::{Cva, EpisodePolicy, InsomniaWorkerConfig, InteractionRuntime, ReliquaryRuntimeRoutes};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(5);

#[derive(Default)]
struct HostState {
    online: bool,
    configured: BTreeSet<RuntimeCapability>,
    statuses: BTreeMap<String, HostOperationStatus>,
    digests: BTreeMap<String, [u8; 32]>,
    handoff_calls: Vec<(String, [u8; 32])>,
    cancel_calls: Vec<String>,
    handoff_entered: bool,
    block_handoff: bool,
    release_handoff: bool,
    lose_next_ack: bool,
}

struct ControlledHost {
    state: Mutex<HostState>,
    changed: Condvar,
}

impl ControlledHost {
    fn new(configured: &[RuntimeCapability]) -> Self {
        Self {
            state: Mutex::new(HostState {
                online: true,
                configured: configured.iter().copied().collect(),
                ..HostState::default()
            }),
            changed: Condvar::new(),
        }
    }

    fn lose_next_ack(&self) {
        self.state.lock().unwrap().lose_next_ack = true;
    }

    fn block_next_handoff(&self) {
        let mut state = self.state.lock().unwrap();
        state.block_handoff = true;
        state.release_handoff = false;
        state.handoff_entered = false;
    }

    fn wait_for_handoff(&self) {
        self.wait_until(
            |state| state.handoff_entered,
            "handoff did not reach adapter",
        );
    }

    fn wait_for_cancel(&self) {
        self.wait_until(
            |state| !state.cancel_calls.is_empty(),
            "cancel did not reach adapter",
        );
    }

    fn wait_until(&self, predicate: impl Fn(&HostState) -> bool, message: &str) {
        let deadline = Instant::now() + WAIT;
        let mut state = self.state.lock().unwrap();
        while !predicate(&state) {
            let now = Instant::now();
            assert!(now < deadline, "{message}");
            let (next, timeout) = self.changed.wait_timeout(state, deadline - now).unwrap();
            state = next;
            assert!(!timeout.timed_out() || predicate(&state), "{message}");
        }
    }

    fn release_handoff(&self) {
        let mut state = self.state.lock().unwrap();
        state.release_handoff = true;
        self.changed.notify_all();
    }

    fn set_status(&self, stable_id: &str, status: HostOperationStatus) {
        self.state
            .lock()
            .unwrap()
            .statuses
            .insert(stable_id.to_owned(), status);
    }

    fn seed_durable(&self, stable_id: &str, digest: [u8; 32], status: HostOperationStatus) {
        let mut state = self.state.lock().unwrap();
        state.digests.insert(stable_id.to_owned(), digest);
        state.statuses.insert(stable_id.to_owned(), status);
    }

    fn handoff_count(&self) -> usize {
        self.state.lock().unwrap().handoff_calls.len()
    }

    fn handoffs(&self) -> Vec<(String, [u8; 32])> {
        self.state.lock().unwrap().handoff_calls.clone()
    }

    fn cancel_count(&self) -> usize {
        self.state.lock().unwrap().cancel_calls.len()
    }
}

impl GatewayHostAdapter for ControlledHost {
    fn configured_capabilities(&self) -> BTreeSet<RuntimeCapability> {
        self.state.lock().unwrap().configured.clone()
    }

    fn connect(&self) -> Result<HostConnection, String> {
        let state = self.state.lock().unwrap();
        if !state.online {
            return Err("controlled host is offline".into());
        }
        let mut capabilities = BTreeMap::new();
        for capability in &state.configured {
            capabilities.insert(*capability, complete_contract());
        }
        Ok(HostConnection { capabilities })
    }

    fn reconcile(&self, capability: RuntimeCapability) -> Result<HostReconcileStatus, String> {
        let state = self.state.lock().unwrap();
        if !state.online {
            return Err("controlled host is offline".into());
        }
        if state.configured.contains(&capability) {
            if state.statuses.values().any(|status| {
                matches!(
                    status,
                    HostOperationStatus::Pending
                        | HostOperationStatus::Running
                        | HostOperationStatus::Unknown
                )
            }) {
                Ok(HostReconcileStatus::Ready)
            } else {
                Ok(HostReconcileStatus::NoOutstandingWork)
            }
        } else {
            Ok(HostReconcileStatus::NoOutstandingWork)
        }
    }

    fn handoff(
        &self,
        _capability: RuntimeCapability,
        operation: &HostOperation,
    ) -> Result<HandoffReceipt, String> {
        operation.validate().map_err(|error| error.to_string())?;
        let mut state = self.state.lock().unwrap();
        if !state.online {
            return Err("controlled host disconnected".into());
        }
        if let Some(previous) = state.digests.get(&operation.stable_id) {
            if previous != &operation.payload_digest {
                return Err("stable operation ID reused with a different digest".into());
            }
        } else {
            state
                .digests
                .insert(operation.stable_id.clone(), operation.payload_digest);
        }
        state
            .handoff_calls
            .push((operation.stable_id.clone(), operation.payload_digest));
        state
            .statuses
            .entry(operation.stable_id.clone())
            .or_insert(HostOperationStatus::Pending);
        state.handoff_entered = true;
        self.changed.notify_all();

        let block_this_handoff = state.block_handoff;
        state.block_handoff = false;
        while block_this_handoff && !state.release_handoff {
            let (next, timeout) = self.changed.wait_timeout(state, WAIT).unwrap();
            state = next;
            if timeout.timed_out() && !state.release_handoff {
                return Err("test timed out waiting for handoff release".into());
            }
        }
        state.block_handoff = false;
        if state.lose_next_ack {
            state.lose_next_ack = false;
            return Err("durable receipt was accepted but its acknowledgment was lost".into());
        }
        Ok(HandoffReceipt {
            stable_id: operation.stable_id.clone(),
            payload_digest: operation.payload_digest,
            durably_accepted: true,
        })
    }

    fn cancel(
        &self,
        _capability: RuntimeCapability,
        stable_id: &str,
    ) -> Result<HostCancelOutcome, String> {
        let mut state = self.state.lock().unwrap();
        if !state.online {
            return Err("controlled host disconnected".into());
        }
        state.cancel_calls.push(stable_id.to_owned());
        self.changed.notify_all();
        match state
            .statuses
            .get(stable_id)
            .copied()
            .unwrap_or(HostOperationStatus::Unknown)
        {
            HostOperationStatus::Pending => {
                state
                    .statuses
                    .insert(stable_id.to_owned(), HostOperationStatus::Cancelled);
                Ok(HostCancelOutcome::CancelledBeforeStart)
            }
            HostOperationStatus::Running => Ok(HostCancelOutcome::RunningCancellationRequested),
            status @ (HostOperationStatus::Completed | HostOperationStatus::Cancelled) => {
                Ok(HostCancelOutcome::AlreadyTerminal(status))
            }
            HostOperationStatus::Unknown => Ok(HostCancelOutcome::Unknown),
        }
    }

    fn status(
        &self,
        _capability: RuntimeCapability,
        stable_id: &str,
    ) -> Result<HostOperationStatus, String> {
        let state = self.state.lock().unwrap();
        if !state.online {
            return Err("controlled host disconnected".into());
        }
        Ok(state
            .statuses
            .get(stable_id)
            .copied()
            .unwrap_or(HostOperationStatus::Unknown))
    }
}

fn complete_contract() -> HostCapabilityContract {
    HostCapabilityContract {
        host_executes_stage: true,
        queue: HostQueueGuarantees {
            durable_acceptance: true,
            stable_submission_ids: true,
            linearizable_cancel_dequeue: true,
            queryable_operation_status: true,
            idempotent_handoff: true,
        },
    }
}

fn gateway_with_host(state_dir: std::path::PathBuf, host: Arc<ControlledHost>) -> GatewayRuntime {
    GatewayRuntime::new_with_host(
        state_dir,
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
        EpisodePolicy::default(),
        host,
    )
}

fn seeded_runtime(path: std::path::PathBuf) -> InteractionRuntime {
    let mut cva = Cva::create_project(path).unwrap();
    cva.append_node(
        "root".into(),
        "host-conversation".into(),
        None,
        "user".into(),
        1,
        "existing turn",
    )
    .unwrap();
    cva.append_branch(Branch {
        id: "main".into(),
        conversation_id: "host-conversation".into(),
        leaf_node_id: "root".into(),
        canonical: true,
    })
    .unwrap();
    cva.sync().unwrap();
    InteractionRuntime::new(cva)
}

fn writer_grants(owner_id: &str) -> BTreeMap<String, BTreeSet<OwnerGrant>> {
    BTreeMap::from([(
        owner_id.to_owned(),
        BTreeSet::from([
            OwnerGrant::ViewRel,
            OwnerGrant::SubmitConversation,
            OwnerGrant::CancelOwnPendingSubmission,
        ]),
    )])
}

fn submit(
    gateway: &GatewayRuntime,
    instance: &InstanceRuntime,
    owner_id: &str,
    submission_id: &str,
) {
    gateway
        .submit_conversation(
            instance,
            owner_id,
            GatewayConversationSubmission {
                conversation_id: "host-conversation".into(),
                submission_id: submission_id.into(),
                expected_conversation_revision: 0,
                expected_parent_node_id: Some("root".into()),
                branch_id: "main".into(),
                content: format!("original user turn {submission_id}"),
                attachments: Vec::new(),
                client_timestamp_ns: None,
            },
        )
        .unwrap();
}

fn mount_writer(
    gateway: &GatewayRuntime,
    path: std::path::PathBuf,
    principal: &str,
) -> (String, Arc<InstanceRuntime>) {
    let owner_id = gateway.mount_rel(seeded_runtime(path)).unwrap();
    let instance = gateway
        .attach_instance(principal.into(), None, writer_grants(&owner_id))
        .unwrap();
    (owner_id, instance)
}

fn mount_open_writer(
    gateway: &GatewayRuntime,
    path: std::path::PathBuf,
    principal: &str,
) -> (String, Arc<InstanceRuntime>) {
    let owner_id = gateway
        .mount_rel(InteractionRuntime::new(Cva::open(path).unwrap()))
        .unwrap();
    let instance = gateway
        .attach_instance(principal.into(), None, writer_grants(&owner_id))
        .unwrap();
    (owner_id, instance)
}

fn journal_status(
    gateway: &GatewayRuntime,
    owner_id: &str,
    conversation_id: &str,
    submission_id: &str,
) -> SubmissionStatus {
    let key = ConversationSubmissionId {
        conversation_id: ConversationId::new(conversation_id).unwrap(),
        submission_id: SubmissionId::new(submission_id).unwrap(),
    };
    gateway
        .conversation
        .journal(owner_id)
        .unwrap()
        .lock()
        .unwrap()
        .status(&key)
        .unwrap()
}

fn archive_has_submission(gateway: &GatewayRuntime, owner_id: &str, submission_id: &str) -> bool {
    let token = gateway
        .authorize(
            &gateway
                .instances
                .read()
                .unwrap()
                .values()
                .next()
                .unwrap()
                .instance,
            owner_id,
            OwnerGrant::ViewRel,
        )
        .unwrap();
    gateway
        .with_rel(&token, |runtime| {
            Ok(runtime
                .cva()
                .archive()
                .has_node("host-conversation", submission_id))
        })
        .unwrap()
}

#[test]
fn host_identity_distinguishes_composite_ids_containing_separators() {
    let key = |conversation, submission| ConversationSubmissionId {
        conversation_id: ConversationId::new(conversation).unwrap(),
        submission_id: SubmissionId::new(submission).unwrap(),
    };
    let owner = "rel-00000000-0000-4000-8000-000000000001";
    let left = key("a/b", "c");
    let right = key("a", "b/c");
    assert_ne!(
        conversation_host_operation_id(owner, &left),
        conversation_host_operation_id(owner, &right)
    );
    assert_eq!(
        conversation_host_operation_id(owner, &left),
        conversation_host_operation_id(owner, &left)
    );
}

#[test]
fn recovery_retries_the_same_durable_handoff_id_and_digest() {
    let state_dir = test_path("host-operation-idempotent-retry-state");
    let rel_path = test_path("host-operation-idempotent-retry.rel");
    let host = Arc::new(ControlledHost::new(&[
        RuntimeCapability::ConversationExecution,
    ]));
    let gateway = gateway_with_host(state_dir.clone(), host.clone());
    let (owner_id, writer) = mount_writer(&gateway, rel_path.clone(), "retry-principal");
    submit(&gateway, &writer, &owner_id, "retry-submit");

    let key = ConversationSubmissionId {
        conversation_id: ConversationId::new("host-conversation").unwrap(),
        submission_id: SubmissionId::new("retry-submit").unwrap(),
    };
    let operation = {
        let journal = gateway.conversation.journal(&owner_id).unwrap();
        let mut journal = journal.lock().unwrap();
        let submission = journal.submission(&key).unwrap().clone();
        let operation =
            conversation_host_operation(&owner_id, &key, &submission, &journal).unwrap();
        journal
            .begin_host_handoff(
                &key,
                RuntimeCapability::ConversationExecution,
                operation.stable_id.clone(),
                operation.payload_digest,
            )
            .unwrap();
        operation
    };
    host.seed_durable(
        &operation.stable_id,
        operation.payload_digest,
        HostOperationStatus::Pending,
    );

    drop(writer);
    drop(gateway);
    let remounted = gateway_with_host(state_dir, host.clone());
    let (remounted_owner, remounted_writer) =
        mount_open_writer(&remounted, rel_path, "retry-principal");
    assert_eq!(remounted_owner, owner_id);
    assert_eq!(
        remounted
            .capability_authority(RuntimeCapability::ConversationExecution)
            .unwrap(),
        StageAuthority::Held,
        "mount recovery must restore the unresolved host hold before dispatch"
    );
    remounted.host_router.as_ref().unwrap().reconnect();
    assert_eq!(
        remounted
            .capability_authority(RuntimeCapability::ConversationExecution)
            .unwrap(),
        StageAuthority::Held,
        "reconnect must not erase the startup journal debt"
    );
    let receipt = remounted
        .process_next_conversation_submission(
            &remounted_writer,
            &owner_id,
            "host-conversation",
            "retry-submit",
        )
        .unwrap();
    assert_eq!(
        receipt.status.host_operations[&RuntimeCapability::ConversationExecution].phase,
        DurableHostOperationPhase::Accepted
    );
    assert_eq!(
        host.handoffs(),
        vec![(operation.stable_id, operation.payload_digest)],
        "a recovered intent must retry the exact stable identity and digest"
    );
    assert!(!archive_has_submission(
        &remounted,
        &owner_id,
        "retry-submit"
    ));
}

#[test]
fn durable_host_completion_survives_restart_without_the_adapter() {
    for cancel_requested in [false, true] {
        let label = format!("host-operation-terminal-proof-{cancel_requested}");
        let state_dir = test_path(&format!("{label}-state"));
        let rel_path = test_path(&format!("{label}.rel"));
        let host = Arc::new(ControlledHost::new(&[
            RuntimeCapability::ConversationExecution,
        ]));
        let gateway = gateway_with_host(state_dir.clone(), host);
        let (owner_id, writer) = mount_writer(&gateway, rel_path.clone(), "terminal-principal");
        submit(&gateway, &writer, &owner_id, "terminal-submit");
        gateway
            .process_next_conversation_submission(
                &writer,
                &owner_id,
                "host-conversation",
                "terminal-submit",
            )
            .unwrap();
        let key = ConversationSubmissionId {
            conversation_id: ConversationId::new("host-conversation").unwrap(),
            submission_id: SubmissionId::new("terminal-submit").unwrap(),
        };
        {
            let journal = gateway.conversation.journal(&owner_id).unwrap();
            let mut journal = journal.lock().unwrap();
            if cancel_requested {
                journal
                    .begin_host_cancel(&key, RuntimeCapability::ConversationExecution)
                    .unwrap();
            }
            journal
                .record_host_status(
                    &key,
                    RuntimeCapability::ConversationExecution,
                    HostOperationStatus::Completed,
                )
                .unwrap();
        }
        drop(writer);
        drop(gateway);
        let recovered = GatewayRuntime::new(
            state_dir,
            ReliquaryRuntimeRoutes::default(),
            InsomniaWorkerConfig {
                workers: 0,
                ..InsomniaWorkerConfig::default()
            },
            EpisodePolicy::default(),
        );
        let (recovered_owner, writer) =
            mount_open_writer(&recovered, rel_path, "terminal-principal");
        assert_eq!(recovered_owner, owner_id);
        for _ in 0..2 {
            let receipt = recovered
                .process_next_conversation_submission(
                    &writer,
                    &owner_id,
                    "host-conversation",
                    "terminal-submit",
                )
                .unwrap();
            assert_eq!(receipt.status.state, SubmissionState::Committed);
        }
        assert!(archive_has_submission(
            &recovered,
            &owner_id,
            "terminal-submit"
        ));
    }
}

#[test]
fn withdrawn_conversation_contract_holds_old_work_until_durable_completion() {
    let capability = RuntimeCapability::ConversationExecution;
    let host = Arc::new(ControlledHost::new(&[capability]));
    let gateway = gateway_with_host(test_path("host-operation-downgrade-state"), host.clone());
    let (owner_id, writer) = mount_writer(
        &gateway,
        test_path("host-operation-downgrade.rel"),
        "downgrade-principal",
    );
    submit(&gateway, &writer, &owner_id, "downgrade-submit");
    gateway
        .process_next_conversation_submission(
            &writer,
            &owner_id,
            "host-conversation",
            "downgrade-submit",
        )
        .unwrap();
    let (stable_id, _) = host.handoffs().pop().unwrap();
    host.state.lock().unwrap().configured.clear();
    gateway.host_router.as_ref().unwrap().reconnect();
    assert_eq!(
        gateway.capability_authority(capability).unwrap(),
        StageAuthority::Held
    );
    let pending = gateway
        .process_next_conversation_submission(
            &writer,
            &owner_id,
            "host-conversation",
            "downgrade-submit",
        )
        .unwrap();
    assert_eq!(pending.status.state, SubmissionState::Pending);
    assert!(!archive_has_submission(
        &gateway,
        &owner_id,
        "downgrade-submit"
    ));
    assert_eq!(host.handoff_count(), 1);
    host.set_status(&stable_id, HostOperationStatus::Completed);
    gateway.host_router.as_ref().unwrap().reconnect();
    assert_eq!(
        gateway.capability_authority(capability).unwrap(),
        StageAuthority::Held
    );
    let committed = gateway
        .process_next_conversation_submission(
            &writer,
            &owner_id,
            "host-conversation",
            "downgrade-submit",
        )
        .unwrap();
    assert_eq!(committed.status.state, SubmissionState::Committed);
    gateway.host_router.as_ref().unwrap().reconnect();
    assert_eq!(
        gateway.capability_authority(capability).unwrap(),
        StageAuthority::Native
    );
    assert_eq!(host.handoff_count(), 1);
    assert!(archive_has_submission(
        &gateway,
        &owner_id,
        "downgrade-submit"
    ));
}

#[test]
fn conflicting_durable_digest_fails_closed_before_host_dispatch() {
    let state_dir = test_path("host-operation-digest-conflict-state");
    let rel_path = test_path("host-operation-digest-conflict.rel");
    let host = Arc::new(ControlledHost::new(&[
        RuntimeCapability::ConversationExecution,
    ]));
    let gateway = gateway_with_host(state_dir.clone(), host.clone());
    let (owner_id, writer) = mount_writer(&gateway, rel_path.clone(), "digest-principal");
    submit(&gateway, &writer, &owner_id, "digest-submit");

    let key = ConversationSubmissionId {
        conversation_id: ConversationId::new("host-conversation").unwrap(),
        submission_id: SubmissionId::new("digest-submit").unwrap(),
    };
    gateway
        .conversation
        .journal(&owner_id)
        .unwrap()
        .lock()
        .unwrap()
        .begin_host_handoff(
            &key,
            RuntimeCapability::ConversationExecution,
            conversation_host_operation_id(&owner_id, &key),
            [0xA5; 32],
        )
        .unwrap();

    drop(writer);
    drop(gateway);
    let remounted = gateway_with_host(state_dir, host.clone());
    let (remounted_owner, remounted_writer) =
        mount_open_writer(&remounted, rel_path, "digest-principal");
    assert_eq!(remounted_owner, owner_id);
    assert!(
        remounted
            .process_next_conversation_submission(
                &remounted_writer,
                &owner_id,
                "host-conversation",
                "digest-submit",
            )
            .is_err(),
        "the payload derived after recovery must match the persisted digest"
    );
    assert_eq!(host.handoff_count(), 0);
    assert!(!archive_has_submission(
        &remounted,
        &owner_id,
        "digest-submit"
    ));
}

#[test]
fn lost_handoff_ack_holds_the_admitted_user_turn_without_blind_replay() {
    let state_dir = test_path("host-operation-lost-ack-state");
    let path = test_path("host-operation-lost-ack.rel");
    let host = Arc::new(ControlledHost::new(&[
        RuntimeCapability::ConversationExecution,
    ]));
    host.lose_next_ack();
    let gateway = gateway_with_host(state_dir, host.clone());
    let (owner_id, writer) = mount_writer(&gateway, path, "lost-ack-principal");
    submit(&gateway, &writer, &owner_id, "lost-ack-submit");

    let first = gateway
        .process_next_conversation_submission(
            &writer,
            &owner_id,
            "host-conversation",
            "lost-ack-submit",
        )
        .unwrap();
    assert_eq!(first.status.state, SubmissionState::Pending);
    assert_eq!(
        first.status.host_operations[&RuntimeCapability::ConversationExecution].phase,
        DurableHostOperationPhase::Unknown
    );
    assert!(!archive_has_submission(
        &gateway,
        &owner_id,
        "lost-ack-submit"
    ));

    let second = gateway
        .process_next_conversation_submission(
            &writer,
            &owner_id,
            "host-conversation",
            "lost-ack-submit",
        )
        .unwrap();
    assert_eq!(second.status.state, SubmissionState::Pending);
    assert_eq!(
        host.handoff_count(),
        1,
        "unknown status must not trigger a second dispatch"
    );
    assert!(!archive_has_submission(
        &gateway,
        &owner_id,
        "lost-ack-submit"
    ));
}

#[test]
fn completed_host_stage_commits_the_original_turn_once_and_replays_idempotently() {
    let host = Arc::new(ControlledHost::new(&[
        RuntimeCapability::ConversationExecution,
    ]));
    let gateway = gateway_with_host(test_path("host-operation-completed-state"), host.clone());
    let (owner_id, writer) = mount_writer(
        &gateway,
        test_path("host-operation-completed.rel"),
        "completed-principal",
    );
    submit(&gateway, &writer, &owner_id, "completed-submit");

    let pending = gateway
        .process_next_conversation_submission(
            &writer,
            &owner_id,
            "host-conversation",
            "completed-submit",
        )
        .unwrap();
    assert_eq!(pending.status.state, SubmissionState::Pending);
    let (stable_id, digest) = host.handoffs().pop().expect("host accepted one operation");
    host.set_status(&stable_id, HostOperationStatus::Completed);

    let committed = gateway
        .process_next_conversation_submission(
            &writer,
            &owner_id,
            "host-conversation",
            "completed-submit",
        )
        .unwrap();
    assert_eq!(committed.status.state, SubmissionState::Committed);
    assert_eq!(
        committed.status.archive_receipt.as_ref().unwrap().node_id,
        "completed-submit"
    );
    assert!(archive_has_submission(
        &gateway,
        &owner_id,
        "completed-submit"
    ));

    let replay = gateway
        .process_next_conversation_submission(
            &writer,
            &owner_id,
            "host-conversation",
            "completed-submit",
        )
        .unwrap();
    assert_eq!(replay.status.state, SubmissionState::Committed);
    assert_eq!(host.handoff_count(), 1);
    assert_eq!(host.handoffs(), vec![(stable_id, digest)]);
    let token = gateway
        .authorize(&writer, &owner_id, OwnerGrant::ViewRel)
        .unwrap();
    let turns = gateway
        .with_rel(&token, |runtime| {
            runtime
                .conversation_turns("host-conversation", "completed-submit")
                .map_err(operation)
        })
        .unwrap();
    assert_eq!(
        turns
            .iter()
            .filter(|turn| turn.node_id == "completed-submit")
            .count(),
        1
    );
    assert_eq!(
        turns.last().unwrap().content,
        "original user turn completed-submit"
    );
}

#[test]
fn unknown_host_status_holds_host_owned_work_while_an_unrelated_native_capability_runs() {
    let host = Arc::new(ControlledHost::new(&[
        RuntimeCapability::ConversationExecution,
    ]));
    let gateway = gateway_with_host(test_path("host-operation-unknown-state"), host.clone());
    let (owner_id, writer) = mount_writer(
        &gateway,
        test_path("host-operation-unknown.rel"),
        "unknown-principal",
    );
    submit(&gateway, &writer, &owner_id, "unknown-submit");

    let accepted = gateway
        .process_next_conversation_submission(
            &writer,
            &owner_id,
            "host-conversation",
            "unknown-submit",
        )
        .unwrap();
    let (stable_id, _) = host.handoffs().pop().unwrap();
    host.set_status(&stable_id, HostOperationStatus::Unknown);
    let held = gateway
        .process_next_conversation_submission(
            &writer,
            &owner_id,
            "host-conversation",
            "unknown-submit",
        )
        .unwrap();
    assert_eq!(accepted.status.state, SubmissionState::Pending);
    assert_eq!(held.status.state, SubmissionState::Pending);
    assert_eq!(
        held.status.host_operations[&RuntimeCapability::ConversationExecution].phase,
        DurableHostOperationPhase::Unknown
    );
    assert_eq!(host.handoff_count(), 1);
    assert!(!archive_has_submission(
        &gateway,
        &owner_id,
        "unknown-submit"
    ));

    // A separate native-only capability remains usable while this host-owned stage is held.
    assert_eq!(
        gateway
            .capability_authority(RuntimeCapability::Dream)
            .unwrap(),
        StageAuthority::Native
    );
}

#[test]
fn cancellation_reaches_host_before_blocked_handoff_ack_and_cannot_be_resurrected() {
    let state_dir = test_path("host-operation-cancel-race-state");
    let rel_path = test_path("host-operation-cancel-race.rel");
    let host = Arc::new(ControlledHost::new(&[
        RuntimeCapability::ConversationExecution,
    ]));
    host.block_next_handoff();
    let gateway = Arc::new(gateway_with_host(state_dir.clone(), host.clone()));
    let (owner_id, writer) = mount_writer(&gateway, rel_path.clone(), "cancel-race-principal");
    submit(&gateway, &writer, &owner_id, "cancel-race-submit");

    let process_gateway = Arc::clone(&gateway);
    let process_writer = Arc::clone(&writer);
    let process_owner = owner_id.clone();
    let process = std::thread::spawn(move || {
        process_gateway.process_next_conversation_submission(
            &process_writer,
            &process_owner,
            "host-conversation",
            "cancel-race-submit",
        )
    });
    host.wait_for_handoff();

    let cancel_gateway = Arc::clone(&gateway);
    let cancel_writer = Arc::clone(&writer);
    let cancel_owner = owner_id.clone();
    let (cancel_tx, cancel_rx) = std::sync::mpsc::channel();
    let cancel = std::thread::spawn(move || {
        let result = cancel_gateway.cancel_conversation_submission(
            &cancel_writer,
            &cancel_owner,
            "host-conversation",
            "cancel-race-submit",
        );
        let _ = cancel_tx.send(result);
    });

    host.wait_for_cancel();
    let cancel_result = cancel_rx
        .recv_timeout(WAIT)
        .expect("cancel should complete before handoff acknowledgment");
    let cancel_status = cancel_result.unwrap().status;
    assert_eq!(cancel_status.state, SubmissionState::Cancelled);
    host.release_handoff();

    let _ = process.join().unwrap();
    cancel.join().unwrap();
    let status = journal_status(
        &gateway,
        &owner_id,
        "host-conversation",
        "cancel-race-submit",
    );
    assert_eq!(status.state, SubmissionState::Cancelled);
    assert_eq!(
        status.host_operations[&RuntimeCapability::ConversationExecution].phase,
        DurableHostOperationPhase::Cancelled
    );
    assert_eq!(host.cancel_count(), 1);
    assert!(!archive_has_submission(
        &gateway,
        &owner_id,
        "cancel-race-submit"
    ));

    drop(writer);
    drop(gateway);
    let host_after_restart = Arc::new(ControlledHost::new(&[
        RuntimeCapability::ConversationExecution,
    ]));
    host_after_restart.set_status(
        &conversation_host_operation_id(
            &owner_id,
            &ConversationSubmissionId {
                conversation_id: ConversationId::new("host-conversation").unwrap(),
                submission_id: SubmissionId::new("cancel-race-submit").unwrap(),
            },
        ),
        HostOperationStatus::Cancelled,
    );
    let remounted = gateway_with_host(state_dir, host_after_restart);
    let runtime = InteractionRuntime::new(Cva::open(rel_path).unwrap());
    assert_eq!(remounted.mount_rel(runtime).unwrap(), owner_id);
    assert_eq!(
        journal_status(
            &remounted,
            &owner_id,
            "host-conversation",
            "cancel-race-submit"
        )
        .state,
        SubmissionState::Cancelled
    );
}

#[test]
fn detach_during_handoff_preserves_receipt_and_owner_continuation() {
    let host = Arc::new(ControlledHost::new(&[
        RuntimeCapability::ConversationExecution,
    ]));
    host.block_next_handoff();
    let gateway = Arc::new(gateway_with_host(
        test_path("f2-detached-host-state"),
        host.clone(),
    ));
    let (owner_id, writer) = mount_writer(
        &gateway,
        test_path("f2-detached-host.rel"),
        "host-submitter",
    );
    submit(&gateway, &writer, &owner_id, "detached-host-turn");
    let process_gateway = Arc::clone(&gateway);
    let process_writer = Arc::clone(&writer);
    let process_owner = owner_id.clone();
    let process = std::thread::spawn(move || {
        process_gateway.process_next_conversation_submission(
            &process_writer,
            &process_owner,
            "host-conversation",
            "detached-host-turn",
        )
    });
    host.wait_for_handoff();
    gateway.detach_instance(&writer).unwrap();
    assert!(gateway.instances.read().unwrap().is_empty());
    assert!(
        gateway
            .cancel_conversation_submission(
                &writer,
                &owner_id,
                "host-conversation",
                "detached-host-turn"
            )
            .is_err()
    );
    host.release_handoff();
    assert!(
        process.join().unwrap().is_err(),
        "detached callers cannot receive the receipt"
    );
    let pending = journal_status(
        &gateway,
        &owner_id,
        "host-conversation",
        "detached-host-turn",
    );
    assert_eq!(
        pending.host_operations[&RuntimeCapability::ConversationExecution].phase,
        DurableHostOperationPhase::Accepted
    );
    let (stable_id, _) = host.handoffs().pop().unwrap();
    host.set_status(&stable_id, HostOperationStatus::Completed);
    let committed = gateway
        .process_accepted_conversation_submission(
            &owner_id,
            "host-conversation",
            "detached-host-turn",
        )
        .unwrap();
    assert_eq!(committed.status.state, SubmissionState::Committed);
    assert_eq!(
        committed.status.archive_receipt.unwrap().principal_id,
        "host-submitter"
    );
    assert_eq!(host.handoff_count(), 1);
    drop(gateway.unmount_rel(&owner_id).unwrap());
}

#[test]
fn quiesce_during_handoff_fences_late_ack_and_preserves_owner_debt() {
    let host = Arc::new(ControlledHost::new(&[
        RuntimeCapability::ConversationExecution,
    ]));
    host.block_next_handoff();
    let gateway = Arc::new(gateway_with_host(
        test_path("quiesce-handoff-state"),
        host.clone(),
    ));
    let (owner, writer) = mount_writer(
        &gateway,
        test_path("quiesce-handoff.rel"),
        "quiesce-host-principal",
    );
    submit(&gateway, &writer, &owner, "quiesce-handoff");
    let process_gateway = Arc::clone(&gateway);
    let process_writer = Arc::clone(&writer);
    let process_owner = owner.clone();
    let processing = std::thread::spawn(move || {
        process_gateway.process_next_conversation_submission(
            &process_writer,
            &process_owner,
            "host-conversation",
            "quiesce-handoff",
        )
    });
    host.wait_for_handoff();
    let key = ConversationSubmissionId {
        conversation_id: ConversationId::new("host-conversation").unwrap(),
        submission_id: SubmissionId::new("quiesce-handoff").unwrap(),
    };
    let journal = gateway.conversation.journal(&owner).unwrap();
    let owner_execution = Arc::clone(
        &gateway
            .rel_executions
            .read()
            .unwrap()
            .get(&owner)
            .unwrap()
            .owner,
    );
    let runtime = {
        let execution = owner_execution
            .try_lock()
            .expect("adapter I/O held the owner lock");
        Arc::clone(execution.runtime.as_ref().unwrap())
    };
    assert!(
        runtime.try_lock().is_ok(),
        "adapter I/O held semantic runtime"
    );
    assert!(
        journal.try_lock().is_ok(),
        "adapter I/O held journal writer"
    );
    gateway.quiesce_rel(&owner).unwrap();
    let before = journal.lock().unwrap().status(&key).unwrap();
    assert!(
        gateway
            .process_next_conversation_submission(
                &writer,
                &owner,
                "host-conversation",
                "quiesce-handoff"
            )
            .is_err()
    );
    assert_eq!(host.handoff_count(), 1);
    host.release_handoff();
    assert!(
        processing.join().unwrap().is_err(),
        "late ack crossed the closed owner fence"
    );
    assert_eq!(journal.lock().unwrap().status(&key).unwrap(), before);
    assert_eq!(
        gateway
            .capability_authority(RuntimeCapability::ConversationExecution)
            .unwrap(),
        StageAuthority::Host
    );
    drop(runtime);
    drop(owner_execution);
    gateway.reopen_rel(&owner).unwrap();
    let (stable_id, _) = host.handoffs()[0].clone();
    host.set_status(&stable_id, HostOperationStatus::Completed);
    let resumed = gateway
        .process_next_conversation_submission(
            &writer,
            &owner,
            "host-conversation",
            "quiesce-handoff",
        )
        .unwrap();
    assert_eq!(resumed.status.state, SubmissionState::Pending);
    assert_eq!(
        resumed.status.host_operations[&RuntimeCapability::ConversationExecution].phase,
        DurableHostOperationPhase::Accepted
    );
    let committed = gateway
        .process_next_conversation_submission(
            &writer,
            &owner,
            "host-conversation",
            "quiesce-handoff",
        )
        .unwrap();
    assert_eq!(committed.status.state, SubmissionState::Committed);
    assert!(
        host.handoffs()
            .iter()
            .all(|item| item == &host.handoffs()[0])
    );
    assert!(archive_has_submission(&gateway, &owner, "quiesce-handoff"));
    gateway.detach_instance(&writer).unwrap();
    drop(journal);
    drop(gateway.unmount_rel(&owner).unwrap());
}
