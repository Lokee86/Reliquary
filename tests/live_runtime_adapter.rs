use reliquary_memory::{
    Branch, Cva, GatewayAdmissionAdapter, GatewayConversationSubmission, GatewayRuntime,
    InsomniaWorkerConfig, OwnerGrant, VerifiedInstanceAdmission,
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
    let adapter = gateway.live_runtime_adapter(instance);
    let mut session = adapter
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
