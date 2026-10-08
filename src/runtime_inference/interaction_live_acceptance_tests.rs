use super::super::super::{
    GatewayHostAdapter, HandoffReceipt, HostCancelOutcome, HostCapabilityContract, HostConnection,
    HostOperation, HostOperationStatus, HostQueueGuarantees, HostReconcileStatus,
    RuntimeCapability,
};
use super::*;
use crate::multiplexing_journal::{DurableHostOperationPhase, SubmissionState};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

fn start_native(
    g: &GatewayRuntime,
    owner: &str,
    instance: &InstanceRuntime,
    session: &LiveSession,
    id: &str,
) -> String {
    session.submit_turn(request(id)).unwrap();
    g.process_next_conversation_submission(instance, owner, "session")
        .unwrap()
        .unwrap();
    g.start_conversation_generation(owner, "session", id)
        .unwrap()
        .status
        .generation
        .unwrap()
        .operation_id
}
#[test]
fn first_turn_creates_one_durable_session_without_local_writer_authority() {
    let (g, owner, instance) = setup("e5-new-session");
    let adapter = g.live_runtime_adapter(instance.clone());
    let mut first = request("first");
    first.conversation_id = "new-session".into();
    first.expected_parent_node_id = None;
    let (mut session, original) = adapter.create_session(&owner, first.clone()).unwrap();
    assert_eq!(original.status.state, SubmissionState::Pending);
    assert_eq!(session.diagnostics().unwrap().pending_submissions, 1);
    let (_, retry) = adapter.create_session(&owner, first.clone()).unwrap();
    assert_eq!(retry, original);
    session.close().unwrap();
    assert_eq!(
        g.conversation_submission_status(&instance, &owner, "new-session", "first")
            .unwrap(),
        original
    );
    g.process_next_conversation_submission(&instance, &owner, "new-session")
        .unwrap()
        .unwrap();
    let source = g
        .start_conversation_generation(&owner, "new-session", "first")
        .unwrap();
    assert_eq!(
        source,
        g.start_conversation_generation(&owner, "new-session", "first")
            .unwrap()
    );
    let replacement = adapter.open_session(&owner, "new-session", "main").unwrap();
    let observation = g
        .conversation_observation_snapshot(&instance, &owner, "new-session")
        .unwrap();
    assert_eq!(observation.submissions.len(), 1);
    assert_eq!(observation.generations.len(), 1);
    let mut wrong = first;
    wrong.branch_id = "other".into();
    assert_eq!(
        replacement.submit_turn(wrong).unwrap_err(),
        LiveRuntimeError::OperationUnavailable
    );
}

fn tool(message: &str, sequence: u64, result: bool) -> crate::EchoEvent {
    crate::EchoEvent {
        conversation_id: "session".into(),
        message_id: message.into(),
        sequence,
        timestamp_ns: 2,
        model_round: None,
        kind: if result {
            crate::EchoEventKind::ToolResult
        } else {
            crate::EchoEventKind::ToolCall
        },
        correlation_id: Some("invocation-1".into()),
        name: Some("calculator".into()),
        content: if result { "4".into() } else { "2+2".into() },
    }
}
#[test]
fn tool_and_generated_artifact_receipts_preserve_order_identity_and_retry() {
    let (g, owner, instance) = setup("e5-events");
    let adapter = g.live_runtime_adapter(instance.clone());
    let session = adapter.open_session(&owner, "session", "main").unwrap();
    let op = start_native(&g, &owner, &instance, &session, "event-turn");
    let generation = g
        .conversation_generation_status(&instance, &owner, "session", &op)
        .unwrap()
        .status
        .generation
        .unwrap();
    let call = tool(&generation.generation_id, 0, false);
    let result = tool(&generation.generation_id, 1, true);
    assert!(
        g.record_conversation_generation_tool_event(
            &owner,
            "session",
            &op,
            tool(&generation.generation_id, 0, true)
        )
        .is_err()
    );
    let mut wrong = call.clone();
    wrong.message_id = "root".into();
    assert!(
        g.record_conversation_generation_tool_event(&owner, "session", &op, wrong)
            .is_err()
    );
    assert!(
        g.record_conversation_generation_tool_event(&owner, "session", &op, call.clone())
            .unwrap()
    );
    assert!(
        !g.record_conversation_generation_tool_event(&owner, "session", &op, call.clone())
            .unwrap()
    );
    let mut conflict = call.clone();
    conflict.content = "different".into();
    assert!(
        g.record_conversation_generation_tool_event(&owner, "session", &op, conflict)
            .is_err()
    );
    assert!(
        g.record_conversation_generation_tool_event(&owner, "session", &op, result.clone())
            .unwrap()
    );
    assert!(
        !g.record_conversation_generation_tool_event(&owner, "session", &op, result)
            .unwrap()
    );
    let events = session.tool_events(&op).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].kind, LiveToolEventKind::Invocation);
    assert_eq!(events[1].kind, LiveToolEventKind::Result);
    assert_eq!(events[0].invocation_id, events[1].invocation_id);
    assert_eq!(events[0].generation.message_id, generation.generation_id);
    g.checkpoint_conversation_generation(&owner, "session", &op, 1, "answer")
        .unwrap();
    assert!(session.generated_artifacts(&op).unwrap().is_empty());
    let file = crate::IncomingAttachment {
        filename: "answer.txt".into(),
        mime_type: Some("text/plain".into()),
        bytes: b"artifact".to_vec(),
    };
    g.complete_conversation_generation_with_artifacts(&owner, "session", &op, vec![file.clone()])
        .unwrap();
    let artifacts = session.generated_artifacts(&op).unwrap();
    assert_eq!(artifacts.len(), 1);
    assert_eq!(
        artifacts[0].generation.status,
        LiveGenerationStatus::Completed
    );
    assert_eq!(artifacts[0].byte_length, 8);
    assert_eq!(
        g.file_bytes_for(&instance, &owner, artifacts[0].file_id)
            .unwrap(),
        b"artifact"
    );
    assert!(
        g.files_for_source_for(&instance, &owner, "session", "root")
            .unwrap()
            .is_empty()
    );
    let token = g.authorize(&instance, &owner, OwnerGrant::ViewRel).unwrap();
    let version = g
        .with_rel(&token, |r| Ok(r.cva().archive_version()))
        .unwrap();
    g.complete_conversation_generation_with_artifacts(&owner, "session", &op, vec![file.clone()])
        .unwrap();
    assert_eq!(
        version,
        g.with_rel(&token, |r| Ok(r.cva().archive_version()))
            .unwrap()
    );
    let mut wrong_file = file;
    wrong_file.bytes = b"changed".to_vec();
    assert!(
        g.complete_conversation_generation_with_artifacts(&owner, "session", &op, vec![wrong_file])
            .is_err()
    );
    let mut late = call;
    late.sequence = 2;
    late.correlation_id = Some("late-call".into());
    assert!(
        g.record_conversation_generation_tool_event(&owner, "session", &op, late)
            .is_err()
    );
    assert_eq!(session.tool_events(&op).unwrap().len(), 2);
}
#[test]
fn original_grant_generation_cannot_be_revived_by_regrant() {
    let (g, owner, instance) = setup("e5-grant-generation");
    let adapter = g.live_runtime_adapter(instance.clone());
    let mut old = adapter.open_session(&owner, "session", "main").unwrap();
    let op = start_native(&g, &owner, &instance, &old, "grant-turn");
    old.watch_generation(&op, 4).unwrap();
    g.checkpoint_conversation_generation(&owner, "session", &op, 1, "private prefix")
        .unwrap();
    g.replace_instance_grants(
        &instance,
        BTreeMap::from([(
            owner.clone(),
            BTreeSet::from([
                OwnerGrant::ViewRel,
                OwnerGrant::SubmitConversation,
                OwnerGrant::CancelSharedGeneration,
            ]),
        )]),
    )
    .unwrap();
    assert_eq!(
        old.next_event().unwrap_err(),
        LiveRuntimeError::FreshSessionRequired
    );
    assert_eq!(
        old.resume_stream(4).unwrap_err(),
        LiveRuntimeError::FreshSessionRequired
    );
    let stale_unwatched = adapter.open_session(&owner, "session", "main").unwrap();
    g.replace_instance_grants(&instance, BTreeMap::new())
        .unwrap();
    g.replace_instance_grants(
        &instance,
        BTreeMap::from([(
            owner.clone(),
            BTreeSet::from([
                OwnerGrant::ViewRel,
                OwnerGrant::SubmitConversation,
                OwnerGrant::CancelSharedGeneration,
            ]),
        )]),
    )
    .unwrap();
    let mut stale_unwatched = stale_unwatched;
    assert_eq!(
        stale_unwatched.watch_generation(&op, 4).unwrap_err(),
        LiveRuntimeError::FreshSessionRequired
    );

    assert_eq!(
        old.cancel_generation(&op).unwrap_err(),
        LiveRuntimeError::FreshSessionRequired
    );
    assert_eq!(
        old.tool_events(&op).unwrap_err(),
        LiveRuntimeError::FreshSessionRequired
    );
    let mut fresh = adapter.open_session(&owner, "session", "main").unwrap();
    let snapshot = fresh.watch_generation(&op, 4).unwrap();
    assert_eq!(snapshot.checkpoint.unwrap().text, "private prefix");
    assert!(fresh.next_event().unwrap().is_none());
}
#[test]
fn moved_recipient_keeps_stream_position_and_resync_is_local() {
    let (g, owner, one) = setup("e5-migration");
    let two = attach(&g, &owner, "other-viewer");
    let mut source = g
        .live_runtime_adapter(one.clone())
        .open_session(&owner, "session", "main")
        .unwrap();
    let mut other = g
        .live_runtime_adapter(two)
        .open_session(&owner, "session", "main")
        .unwrap();
    let op = start_native(&g, &owner, &one, &source, "migration");
    source.watch_generation(&op, 1).unwrap();
    other.watch_generation(&op, 8).unwrap();
    let mut moved = source;
    assert!(!moved.diagnostics().unwrap().resync_required);
    g.checkpoint_conversation_generation(&owner, "session", &op, 1, "a")
        .unwrap();
    g.checkpoint_conversation_generation(&owner, "session", &op, 2, "ab")
        .unwrap();
    assert!(matches!(
        moved.next_event().unwrap(),
        Some(LiveStreamEvent::ResyncRequired { .. })
    ));
    assert!(moved.diagnostics().unwrap().resync_required);
    assert!(!other.diagnostics().unwrap().resync_required);
    assert_eq!(
        moved.resume_stream(1).unwrap().checkpoint.unwrap().text,
        "ab"
    );
    assert!(!moved.diagnostics().unwrap().resync_required);
    assert!(moved.next_event().unwrap().is_none());
    assert!(other.next_event().unwrap().is_some());
}
#[derive(Default)]
struct HostState {
    withdrawn: bool,
    operations: BTreeMap<String, ([u8; 32], HostOperationStatus)>,
    handoffs: usize,
    block_cancel: bool,
    cancel_entered: bool,
    cancel_released: bool,
}
#[derive(Default)]
struct Host {
    state: Mutex<HostState>,
    changed: Condvar,
}
impl GatewayHostAdapter for Host {
    fn configured_capabilities(&self) -> BTreeSet<RuntimeCapability> {
        BTreeSet::from([RuntimeCapability::ConversationExecution])
    }
    fn connect(&self) -> Result<HostConnection, String> {
        let state = self.state.lock().unwrap();
        Ok(HostConnection {
            capabilities: BTreeMap::from([(
                RuntimeCapability::ConversationExecution,
                HostCapabilityContract {
                    host_executes_stage: !state.withdrawn,
                    queue: HostQueueGuarantees {
                        durable_acceptance: true,
                        stable_submission_ids: true,
                        linearizable_cancel_dequeue: true,
                        queryable_operation_status: true,
                        idempotent_handoff: true,
                    },
                },
            )]),
        })
    }
    fn reconcile(&self, _: RuntimeCapability) -> Result<HostReconcileStatus, String> {
        Ok(
            if self
                .state
                .lock()
                .unwrap()
                .operations
                .values()
                .any(|(_, s)| {
                    !matches!(
                        s,
                        HostOperationStatus::Completed | HostOperationStatus::Cancelled
                    )
                })
            {
                HostReconcileStatus::Ready
            } else {
                HostReconcileStatus::NoOutstandingWork
            },
        )
    }
    fn handoff(
        &self,
        _: RuntimeCapability,
        operation: &HostOperation,
    ) -> Result<HandoffReceipt, String> {
        operation.validate().map_err(|e| e.to_string())?;
        let mut state = self.state.lock().unwrap();
        if let Some((digest, _)) = state.operations.get(&operation.stable_id) {
            if digest != &operation.payload_digest {
                return Err("digest conflict".into());
            }
        } else {
            state.operations.insert(
                operation.stable_id.clone(),
                (operation.payload_digest, HostOperationStatus::Pending),
            );
        }
        state.handoffs += 1;
        Ok(HandoffReceipt {
            stable_id: operation.stable_id.clone(),
            payload_digest: operation.payload_digest,
            durably_accepted: true,
        })
    }
    fn cancel(&self, _: RuntimeCapability, id: &str) -> Result<HostCancelOutcome, String> {
        let mut state = self.state.lock().unwrap();
        state.cancel_entered = true;
        self.changed.notify_all();
        while state.block_cancel && !state.cancel_released {
            let (next, timeout) = self
                .changed
                .wait_timeout(state, Duration::from_secs(5))
                .unwrap();
            state = next;
            if timeout.timed_out() {
                return Err("test cancel timeout".into());
            }
        }
        let Some((_, status)) = state.operations.get_mut(id) else {
            return Ok(HostCancelOutcome::Unknown);
        };
        Ok(match *status {
            HostOperationStatus::Pending => {
                *status = HostOperationStatus::Cancelled;
                HostCancelOutcome::CancelledBeforeStart
            }
            HostOperationStatus::Running => HostCancelOutcome::RunningCancellationRequested,
            HostOperationStatus::Unknown => HostCancelOutcome::Unknown,
            terminal => HostCancelOutcome::AlreadyTerminal(terminal),
        })
    }
    fn status(&self, _: RuntimeCapability, id: &str) -> Result<HostOperationStatus, String> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .operations
            .get(id)
            .map(|(_, s)| *s)
            .unwrap_or(HostOperationStatus::Unknown))
    }
}
fn host_setup(label: &str) -> (Arc<GatewayRuntime>, String, Arc<InstanceRuntime>, Arc<Host>) {
    let host = Arc::new(Host::default());
    let g = Arc::new(GatewayRuntime::new_with_host(
        test_path(&format!("{label}-state")),
        Default::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        Default::default(),
        host.clone(),
    ));
    let mut cva = Cva::create_project(test_path(&format!("{label}.rel"))).unwrap();
    cva.append_node(
        "root".into(),
        "session".into(),
        None,
        "user".into(),
        1,
        "seed",
    )
    .unwrap();
    cva.append_branch(Branch {
        id: "main".into(),
        conversation_id: "session".into(),
        leaf_node_id: "root".into(),
        canonical: true,
    })
    .unwrap();
    cva.sync().unwrap();
    let owner = g.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let instance = attach(&g, &owner, "host-principal");
    (g, owner, instance, host)
}
fn dispatch_host(
    g: &GatewayRuntime,
    owner: &str,
    instance: &InstanceRuntime,
    session: &LiveSession,
    id: &str,
) -> String {
    session.submit_turn(request(id)).unwrap();
    let receipt = g
        .process_next_conversation_submission(instance, owner, "session")
        .unwrap()
        .unwrap();
    receipt.status.host_operations[&RuntimeCapability::ConversationExecution]
        .stable_id
        .clone()
}
#[test]
fn host_pending_cancel_and_running_request_have_truthful_distinct_receipts() {
    for running in [false, true] {
        let (g, owner, instance, host) = host_setup(&format!("e5-host-cancel-{running}"));
        let session = g
            .live_runtime_adapter(instance.clone())
            .open_session(&owner, "session", "main")
            .unwrap();
        let id = dispatch_host(&g, &owner, &instance, &session, "host-turn");
        if running {
            host.state
                .lock()
                .unwrap()
                .operations
                .get_mut(&id)
                .unwrap()
                .1 = HostOperationStatus::Running;
        }
        let receipt = session.cancel_generation(&id).unwrap();
        let operation = &receipt.status.host_operations[&RuntimeCapability::ConversationExecution];
        if running {
            assert_eq!(
                operation.phase,
                DurableHostOperationPhase::RunningCancellationRequested
            );
            assert_eq!(receipt.status.state, SubmissionState::Pending);
            assert_eq!(session.diagnostics().unwrap().cancellation_requests, 1);
        } else {
            assert_eq!(operation.phase, DurableHostOperationPhase::Cancelled);
            assert_eq!(receipt.status.state, SubmissionState::Cancelled);
        }
        assert!(
            session.tool_events(&id).is_err(),
            "host payload cannot masquerade as a native tool receipt"
        );
    }
}
#[test]
fn downgrade_and_unknown_reconnect_hold_existing_work_without_native_replay() {
    let (g, owner, instance, host) = host_setup("e5-host-held");
    let session = g
        .live_runtime_adapter(instance.clone())
        .open_session(&owner, "session", "main")
        .unwrap();
    let id = dispatch_host(&g, &owner, &instance, &session, "held");
    {
        let mut state = host.state.lock().unwrap();
        state.operations.get_mut(&id).unwrap().1 = HostOperationStatus::Unknown;
        state.withdrawn = true;
    }
    g.reconnect_host();
    assert_eq!(
        session.diagnostics().unwrap().conversation_execution,
        LiveCapability::UnknownStatusHeld
    );
    let _ = g.resume_conversation_submission(&instance, &owner, "session", "held");
    assert_eq!(host.state.lock().unwrap().handoffs, 1);
    let held = session.turn_status("held").unwrap();
    assert_eq!(held.status.state, SubmissionState::Pending);
    assert_eq!(session.diagnostics().unwrap().held_host_operations, 1);
    assert!(
        g.conversation_transcript_for(&instance, &owner, "session", "root")
            .unwrap()
            .iter()
            .all(|t| t.message_id != "held")
    );
}
#[test]
fn revocation_during_host_cancel_does_not_hold_leases_or_accept_old_token_ack() {
    let (g, owner, instance, host) = host_setup("e5-host-token-race");
    let session = g
        .live_runtime_adapter(instance.clone())
        .open_session(&owner, "session", "main")
        .unwrap();
    let id = dispatch_host(&g, &owner, &instance, &session, "race");
    host.state.lock().unwrap().block_cancel = true;
    let cancel_id = id.clone();
    let worker = std::thread::spawn(move || session.cancel_generation(&cancel_id));
    {
        let mut state = host.state.lock().unwrap();
        while !state.cancel_entered {
            let (next, timeout) = host
                .changed
                .wait_timeout(state, Duration::from_secs(5))
                .unwrap();
            state = next;
            assert!(!timeout.timed_out(), "cancel callback was not entered");
        }
    }
    // Regrant would deadlock if E retained admission during external callback.
    g.replace_instance_grants(
        &instance,
        BTreeMap::from([(
            owner.clone(),
            BTreeSet::from([
                OwnerGrant::ViewRel,
                OwnerGrant::SubmitConversation,
                OwnerGrant::CancelSharedGeneration,
            ]),
        )]),
    )
    .unwrap();
    {
        let mut state = host.state.lock().unwrap();
        state.cancel_released = true;
        host.changed.notify_all();
    }
    assert!(worker.join().unwrap().is_err());
    let receipt = g
        .conversation_submission_status(&instance, &owner, "session", "race")
        .unwrap();
    let operation = &receipt.status.host_operations[&RuntimeCapability::ConversationExecution];
    assert!(operation.cancel_requested);
    assert_ne!(operation.phase, DurableHostOperationPhase::Cancelled);
    let fresh = g
        .live_runtime_adapter(instance)
        .open_session(&owner, "session", "main")
        .unwrap();
    let retry = fresh.cancel_generation(&id).unwrap();
    assert_eq!(retry.status.state, SubmissionState::Cancelled);
}

#[test]
fn replacement_resume_hints_validate_identity_epoch_and_future_positions() {
    let (g, owner, instance) = setup("live-resume-hints");
    let adapter = g.live_runtime_adapter(instance.clone());
    let mut first = adapter.open_session(&owner, "session", "main").unwrap();
    first.submit_turn(request("resume")).unwrap();
    g.process_next_conversation_submission(&instance, &owner, "session")
        .unwrap();
    let generation = g
        .start_conversation_generation(&owner, "session", "resume")
        .unwrap()
        .status
        .generation
        .unwrap();
    g.checkpoint_conversation_generation(&owner, "session", &generation.operation_id, 1, "one")
        .unwrap();
    let snapshot = first.watch_generation(&generation.operation_id, 4).unwrap();
    let hint = first.resume_token(&snapshot).unwrap();
    first.close().unwrap();
    g.checkpoint_conversation_generation(&owner, "session", &generation.operation_id, 2, "one two")
        .unwrap();
    let replacement = g.live_runtime_adapter(instance.clone());
    let mut session = replacement.open_session(&owner, "session", "main").unwrap();
    for field in 0..6 {
        let mut forged = hint.clone();
        match field {
            0 => forged.owner_id.push_str("-other"),
            1 => forged.conversation_id.push_str("-other"),
            2 => forged.branch_id.push_str("-other"),
            3 => forged.message_id.push_str("-other"),
            4 => forged.checkpoint_sequence = Some(u64::MAX),
            _ => forged.cursor.delivery_sequence = u64::MAX,
        }
        assert!(session.resume_generation(&forged, 4).is_err());
    }
    let current = session.resume_generation(&hint, 4).unwrap();
    assert_eq!(current.checkpoint.unwrap().text, "one two");
    assert!(session.next_event().unwrap().is_none());
    session.close().unwrap();
    g.quiesce_rel(&owner).unwrap();
    g.reopen_rel(&owner).unwrap();
    let mut fresh = replacement.open_session(&owner, "session", "main").unwrap();
    assert_eq!(
        fresh.resume_generation(&hint, 4).unwrap_err(),
        LiveRuntimeError::FreshSessionRequired
    );
    fresh.watch_generation(&generation.operation_id, 4).unwrap();
}

#[test]
fn owner_receipt_bounds_reject_new_work_without_rejecting_exact_retry() {
    let (g, owner, instance) = setup("e5-bounds");
    let session = g
        .live_runtime_adapter(instance.clone())
        .open_session(&owner, "session", "main")
        .unwrap();
    let op = start_native(&g, &owner, &instance, &session, "bounds");
    let message = g
        .conversation_generation_status(&instance, &owner, "session", &op)
        .unwrap()
        .status
        .generation
        .unwrap()
        .generation_id;
    let mut call = tool(&message, 0, false);
    call.content = "x".repeat(1024 * 1024 + 1);
    assert!(
        g.record_conversation_generation_tool_event(&owner, "session", &op, call.clone())
            .is_err()
    );
    assert!(session.tool_events(&op).unwrap().is_empty());
    call.content.pop();
    assert!(
        g.record_conversation_generation_tool_event(&owner, "session", &op, call.clone())
            .unwrap()
    );
    assert!(
        !g.record_conversation_generation_tool_event(&owner, "session", &op, call)
            .unwrap()
    );
    assert!(
        g.record_conversation_generation_tool_event(
            &owner,
            "session",
            &op,
            tool(&message, 1, true)
        )
        .is_err()
    );
    assert_eq!(session.tool_events(&op).unwrap().len(), 1);
    g.checkpoint_conversation_generation(&owner, "session", &op, 1, "answer")
        .unwrap();
    let empty = crate::IncomingAttachment {
        filename: "empty".into(),
        mime_type: None,
        bytes: vec![],
    };
    assert!(
        g.complete_conversation_generation_with_artifacts(&owner, "session", &op, vec![empty; 65])
            .is_err()
    );
    let huge = crate::IncomingAttachment {
        filename: "huge".into(),
        mime_type: None,
        bytes: vec![0; 8 * 1024 * 1024 + 1],
    };
    assert!(
        g.complete_conversation_generation_with_artifacts(&owner, "session", &op, vec![huge])
            .is_err()
    );
    assert!(session.generated_artifacts(&op).unwrap().is_empty());
    g.complete_conversation_generation(&owner, "session", &op)
        .unwrap();
}

#[test]
fn historical_echo_cannot_bypass_native_tool_order_validation() {
    let (g, owner, instance) = setup("e5-malformed-echo");
    let session = g
        .live_runtime_adapter(instance.clone())
        .open_session(&owner, "session", "main")
        .unwrap();
    let op = start_native(&g, &owner, &instance, &session, "malformed");
    let message = g
        .conversation_generation_status(&instance, &owner, "session", &op)
        .unwrap()
        .status
        .generation
        .unwrap()
        .generation_id;
    let result = tool(&message, 0, true);
    g.put_echo_event_for(&instance, &owner, result).unwrap();
    assert_eq!(
        session.tool_events(&op).unwrap_err(),
        LiveRuntimeError::OperationUnavailable
    );
    let mut call = tool(&message, 1, false);
    call.correlation_id = Some("other".into());
    assert!(
        g.record_conversation_generation_tool_event(&owner, "session", &op, call)
            .is_err()
    );
}
