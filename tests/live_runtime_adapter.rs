// Explicit downstream names are part of the live consumer contract.
use reliquary_memory::{
    Branch, Cva, GatewayAdmissionAdapter, GatewayConversationSubmission, GatewayRuntime,
    InsomniaWorkerConfig, OwnerGrant, VerifiedInstanceAdmission,
};
pub use reliquary_memory::{
    LiveCapability, LiveCheckpoint, LiveCheckpointShape, LiveDiagnostics, LiveGeneratedArtifact,
    LiveGeneration, LiveGenerationStatus, LiveResumeToken, LiveRuntimeAdapter, LiveRuntimeError,
    LiveSession, LiveStreamCursor, LiveStreamEvent, LiveStreamSnapshot, LiveToolEvent,
    LiveToolEventKind,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

struct Admission(Mutex<Option<String>>);
impl GatewayAdmissionAdapter for Admission {
    fn verify(&self, credential: &str) -> Result<VerifiedInstanceAdmission, String> {
        if credential != "verified" {
            return Err("rejected".into());
        }
        Ok(VerifiedInstanceAdmission {
            principal_id: "public-test-principal".into(),
            phy_id: None,
            grants: BTreeMap::from([(
                self.0.lock().unwrap().clone().ok_or("owner unavailable")?,
                BTreeSet::from([
                    OwnerGrant::ViewRel,
                    OwnerGrant::SubmitConversation,
                    OwnerGrant::CancelOwnPendingSubmission,
                    OwnerGrant::ManageRel,
                ]),
            )]),
        })
    }
}
#[test]
fn host_uses_adapter_without_receiving_a_store_handle() {
    let root = std::env::temp_dir().join(format!("reliquary-live-public-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("owner.rel");
    let mut cva = Cva::create_project(&path).unwrap();
    cva.append_node(
        "seed".into(),
        "conversation".into(),
        None,
        "user".into(),
        1,
        "seed",
    )
    .unwrap();
    cva.append_branch(Branch {
        id: "main".into(),
        conversation_id: "conversation".into(),
        leaf_node_id: "seed".into(),
        canonical: true,
    })
    .unwrap();
    cva.sync().unwrap();
    drop(cva);
    let admission = Arc::new(Admission(Mutex::new(None)));
    let gateway = Arc::new(
        GatewayRuntime::new(
            root.join("state"),
            Default::default(),
            InsomniaWorkerConfig {
                workers: 0,
                ..Default::default()
            },
            Default::default(),
        )
        .with_admission_adapter(admission.clone()),
    );
    let owner = gateway.mount_rel_path(&path).unwrap();
    *admission.0.lock().unwrap() = Some(owner.clone());
    let instance = gateway.attach_authenticated_instance("verified").unwrap();
    let adapter: LiveRuntimeAdapter = gateway.live_runtime_adapter(instance);
    let mut session: LiveSession = adapter
        .open_session(&owner, "conversation", "main")
        .unwrap();
    let receipt = session
        .submit_turn(GatewayConversationSubmission {
            conversation_id: "conversation".into(),
            submission_id: "turn".into(),
            expected_conversation_revision: 0,
            expected_parent_node_id: Some("seed".into()),
            branch_id: "main".into(),
            content: "hello".into(),
            attachments: vec![],
            client_timestamp_ns: None,
        })
        .unwrap();
    assert_eq!(receipt.id.submission_id.as_str(), "turn");
    assert_eq!(session.turn_status("turn").unwrap(), receipt);
    assert!(session.diagnostics().unwrap().mount_epoch > 0);
    session.close().unwrap();
    assert!(session.turn_status("turn").is_err());
    let replacement = adapter
        .open_session(&owner, "conversation", "main")
        .unwrap();
    assert_eq!(replacement.turn_status("turn").unwrap(), receipt);
    drop(replacement);
    drop(session);
    adapter.quiesce_owner(&owner).unwrap();
    adapter.reconcile_quiesced_owner(&owner).unwrap();
    adapter.reopen_owner(&owner).unwrap();
    gateway.shutdown().unwrap();
    drop(adapter);
    drop(gateway);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_receipts_and_artifacts_survive_runtime_restart_without_duplicate_work() {
    use reliquary_memory::{EchoEvent, EchoEventKind, IncomingAttachment};
    let root = std::env::temp_dir().join(format!("reliquary-e5-restart-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("owner.rel");
    let cva = Cva::create_project(&path).unwrap();
    cva.sync().unwrap();
    drop(cva);
    let admission = Arc::new(Admission(Mutex::new(None)));
    let make_gateway = || {
        Arc::new(
            GatewayRuntime::new(
                root.join("state"),
                Default::default(),
                InsomniaWorkerConfig {
                    workers: 0,
                    ..Default::default()
                },
                Default::default(),
            )
            .with_admission_adapter(admission.clone()),
        )
    };
    let gateway = make_gateway();
    let owner = gateway.mount_rel_path(&path).unwrap();
    *admission.0.lock().unwrap() = Some(owner.clone());
    let instance = gateway.attach_authenticated_instance("verified").unwrap();
    let adapter: LiveRuntimeAdapter = gateway.live_runtime_adapter(instance.clone());
    let (mut session, receipt) = adapter
        .create_session(
            &owner,
            GatewayConversationSubmission {
                conversation_id: "new".into(),
                submission_id: "first".into(),
                expected_conversation_revision: 0,
                expected_parent_node_id: None,
                branch_id: "main".into(),
                content: "hello".into(),
                attachments: vec![],
                client_timestamp_ns: None,
            },
        )
        .unwrap();
    gateway
        .process_next_conversation_submission(&instance, &owner, "new")
        .unwrap()
        .unwrap();
    let operation = gateway
        .start_conversation_generation(&owner, "new", "first")
        .unwrap()
        .status
        .generation
        .unwrap();
    let call = EchoEvent {
        conversation_id: "new".into(),
        message_id: operation.generation_id.clone(),
        sequence: 0,
        timestamp_ns: 2,
        model_round: None,
        kind: EchoEventKind::ToolCall,
        correlation_id: Some("tool".into()),
        name: Some("write".into()),
        content: "create".into(),
    };
    gateway
        .record_conversation_generation_tool_event(
            &owner,
            "new",
            &operation.operation_id,
            call.clone(),
        )
        .unwrap();
    let mut result = call.clone();
    result.sequence = 1;
    result.kind = EchoEventKind::ToolResult;
    result.content = "done".into();
    gateway
        .record_conversation_generation_tool_event(&owner, "new", &operation.operation_id, result)
        .unwrap();
    gateway
        .checkpoint_conversation_generation(&owner, "new", &operation.operation_id, 1, "answer")
        .unwrap();
    let file = IncomingAttachment {
        filename: "answer.txt".into(),
        mime_type: Some("text/plain".into()),
        bytes: b"durable".to_vec(),
    };
    let completed = gateway
        .complete_conversation_generation_with_artifacts(
            &owner,
            "new",
            &operation.operation_id,
            vec![file.clone()],
        )
        .unwrap();
    let events: Vec<LiveToolEvent> = session.tool_events(&operation.operation_id).unwrap();
    let artifacts: Vec<LiveGeneratedArtifact> = session
        .generated_artifacts(&operation.operation_id)
        .unwrap();
    assert_eq!(artifacts.len(), 1);
    assert_eq!(events[0].kind, LiveToolEventKind::Invocation);
    assert_eq!(events[1].kind, LiveToolEventKind::Result);
    assert_eq!(
        artifacts[0].generation.status,
        LiveGenerationStatus::Completed
    );
    let snapshot: LiveStreamSnapshot = session
        .watch_generation(&operation.operation_id, 4)
        .unwrap();
    let hint: LiveResumeToken = session.resume_token(&snapshot).unwrap();
    session.close().unwrap();
    let mut replacement = adapter.open_session(&owner, "new", "main").unwrap();
    assert_eq!(
        replacement
            .resume_generation(&hint, 4)
            .unwrap()
            .checkpoint
            .unwrap()
            .text,
        "answer"
    );
    replacement.close().unwrap();
    drop(replacement);
    drop(session);
    drop(adapter);
    drop(instance);
    gateway.shutdown().unwrap();
    drop(gateway);

    let reopened = make_gateway();
    assert_eq!(reopened.mount_rel_path(&path).unwrap(), owner);
    let instance = reopened.attach_authenticated_instance("verified").unwrap();
    let adapter: LiveRuntimeAdapter = reopened.live_runtime_adapter(instance.clone());
    let mut session: LiveSession = adapter.open_session(&owner, "new", "main").unwrap();
    assert_eq!(
        session.tool_events(&operation.operation_id).unwrap(),
        events
    );
    assert_eq!(
        session
            .generated_artifacts(&operation.operation_id)
            .unwrap(),
        artifacts
    );
    assert_eq!(
        reopened
            .file_bytes_for(&instance, &owner, artifacts[0].file_id)
            .unwrap(),
        b"durable"
    );
    assert!(
        !reopened
            .record_conversation_generation_tool_event(&owner, "new", &operation.operation_id, call)
            .unwrap()
    );
    assert_eq!(
        reopened
            .complete_conversation_generation_with_artifacts(
                &owner,
                "new",
                &operation.operation_id,
                vec![file]
            )
            .unwrap(),
        completed
    );
    assert_eq!(
        session
            .turn_status(receipt.id.submission_id.as_str())
            .unwrap(),
        completed
    );
    assert!(
        reopened
            .process_next_conversation_submission(&instance, &owner, "new")
            .unwrap()
            .is_none()
    );
    session.close().unwrap();
    drop(session);
    drop(adapter);
    drop(instance);
    reopened.shutdown().unwrap();
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}
