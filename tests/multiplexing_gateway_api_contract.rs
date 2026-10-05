use reliquary_memory::{
    Cva, EpisodePolicy, GatewayAdmissionAdapter, GatewayConversationSubmission, GatewayRuntime,
    GatewaySubmissionState, InsomniaWorkerConfig, OwnerGrant, ReliquaryRuntimeRoutes,
    VerifiedInstanceAdmission,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

struct Admission(String);
impl GatewayAdmissionAdapter for Admission {
    fn verify(&self, credential: &str) -> Result<VerifiedInstanceAdmission, String> {
        if credential != "verified-credential" {
            return Err("credential rejected".into());
        }
        Ok(VerifiedInstanceAdmission {
            principal_id: "verified-writer".into(),
            phy_id: None,
            grants: BTreeMap::from([(
                self.0.clone(),
                BTreeSet::from([
                    OwnerGrant::ViewRel,
                    OwnerGrant::SubmitConversation,
                    OwnerGrant::CancelOwnPendingSubmission,
                ]),
            )]),
        })
    }
}

#[test]
fn public_admission_and_durable_receipts_need_no_legacy_selection() {
    let dir = std::env::temp_dir().join(format!("rq-api-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("owner.rel");
    drop(Cva::create_project(&path).unwrap());
    let gateway = GatewayRuntime::new(
        dir.join("state"),
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
        EpisodePolicy::default(),
    );
    let owner = gateway.mount_rel_path(&path).unwrap();
    let gateway = gateway.with_admission_adapter(Arc::new(Admission(owner.clone())));
    assert!(gateway.attach_authenticated_instance("forged").is_err());
    let instance = gateway
        .attach_authenticated_instance("verified-credential")
        .unwrap();
    let receipt = gateway
        .submit_conversation(
            &instance,
            &owner,
            GatewayConversationSubmission {
                conversation_id: "conversation".into(),
                submission_id: "submission".into(),
                expected_conversation_revision: 0,
                expected_parent_node_id: None,
                branch_id: "main".into(),
                content: "admitted turn".into(),
                attachments: Vec::new(),
                client_timestamp_ns: None,
            },
        )
        .unwrap();
    assert_eq!(receipt.id.conversation_id.as_str(), "conversation");
    assert_eq!(receipt.id.submission_id.as_str(), "submission");
    assert_eq!(receipt.status.state, GatewaySubmissionState::Pending);
    assert!(receipt.status.host_operations.is_empty());
    let cancelled = gateway
        .cancel_conversation_submission(&instance, &owner, "conversation", "submission")
        .unwrap();
    assert_eq!(cancelled.status.state, GatewaySubmissionState::Cancelled);
    gateway.detach_instance(&instance).unwrap();
    drop(gateway.unmount_rel(&owner).unwrap());
    drop(gateway);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn public_branch_admission_and_finalization_preserve_independent_heads() {
    use reliquary_memory::Branch;
    let dir = std::env::temp_dir().join(format!("rq-branch-api-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("owner.rel");
    let mut cva = Cva::create_project(&path).unwrap();
    for (branch, root) in [("main", "main-root"), ("alternate", "alt-root")] {
        cva.append_node(
            root.into(),
            "conversation".into(),
            None,
            "user".into(),
            1,
            root,
        )
        .unwrap();
        cva.append_branch(Branch {
            id: branch.into(),
            conversation_id: "conversation".into(),
            leaf_node_id: root.into(),
            canonical: branch == "main",
        })
        .unwrap();
    }
    cva.sync().unwrap();
    drop(cva);
    let gateway = GatewayRuntime::new(
        dir.join("state"),
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
        EpisodePolicy::default(),
    );
    let owner = gateway.mount_rel_path(&path).unwrap();
    let gateway = gateway.with_admission_adapter(Arc::new(Admission(owner.clone())));
    let instance = gateway
        .attach_authenticated_instance("verified-credential")
        .unwrap();
    let mut expected_revision = 0;
    for (branch, parent, id) in [
        ("main", "main-root", "main-turn"),
        ("alternate", "alt-root", "alt-turn"),
    ] {
        let session = gateway
            .admit_conversation_writer(&instance, &owner, "conversation", branch)
            .unwrap();
        assert_eq!(session.leaf_message_id.as_deref(), Some(parent));
        let receipt = gateway
            .submit_conversation(
                &instance,
                &owner,
                GatewayConversationSubmission {
                    conversation_id: "conversation".into(),
                    submission_id: id.into(),
                    expected_conversation_revision: expected_revision,
                    expected_parent_node_id: Some(parent.into()),
                    branch_id: branch.into(),
                    content: format!("turn {id}"),
                    attachments: vec![],
                    client_timestamp_ns: None,
                },
            )
            .unwrap();
        expected_revision = receipt.status.transition_revision;
    }
    // The coordinator chooses chronological order across eligible branches.
    for (id, parent) in [("main-turn", "main-root"), ("alt-turn", "alt-root")] {
        let receipt = gateway
            .process_next_conversation_submission(&instance, &owner, "conversation")
            .unwrap()
            .unwrap();
        assert_eq!(receipt.id.submission_id.as_str(), id);
        assert_eq!(receipt.status.state, GatewaySubmissionState::Committed);
        assert_eq!(
            receipt
                .status
                .finalize_capture
                .unwrap()
                .parent_node_id
                .as_deref(),
            Some(parent)
        );
    }
    gateway.detach_instance(&instance).unwrap();
    drop(gateway.unmount_rel(&owner).unwrap());
    drop(gateway);
    let reopened = Cva::open(path).unwrap();
    assert!(
        reopened
            .archive()
            .branches()
            .iter()
            .any(|b| b.id == "main" && b.leaf_node_id == "main-turn")
    );
    assert!(
        reopened
            .archive()
            .branches()
            .iter()
            .any(|b| b.id == "alternate" && b.leaf_node_id == "alt-turn" && !b.canonical)
    );
    drop(reopened);
    std::fs::remove_dir_all(dir).unwrap();
}

struct PolicyAdmission(String);
impl GatewayAdmissionAdapter for PolicyAdmission {
    fn verify(&self, credential: &str) -> Result<VerifiedInstanceAdmission, String> {
        let mut capabilities = BTreeSet::from([OwnerGrant::ViewRel]);
        match credential {
            "manager" => {
                capabilities.insert(OwnerGrant::ManageRel);
            }
            "viewer" => {}
            _ => return Err("credential rejected".into()),
        }
        Ok(VerifiedInstanceAdmission {
            principal_id: format!("verified-{credential}"),
            phy_id: None,
            grants: BTreeMap::from([(self.0.clone(), capabilities)]),
        })
    }
}

#[test]
fn public_policy_management_uses_verified_grants_and_survives_reopen() {
    use reliquary_memory::ConversationPolicy::{SequentialChronological, VisibleAutoBranch};
    let dir = std::env::temp_dir().join(format!("rq-policy-api-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("owner.rel");
    let mut cva = Cva::create_project(&path).unwrap();
    cva.append_node(
        "root".into(),
        "conversation".into(),
        None,
        "user".into(),
        1,
        "root",
    )
    .unwrap();
    cva.sync().unwrap();
    drop(cva);
    let gateway = GatewayRuntime::new(
        dir.join("state"),
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
        EpisodePolicy::default(),
    );
    let owner = gateway.mount_rel_path(&path).unwrap();
    let gateway = gateway.with_admission_adapter(Arc::new(PolicyAdmission(owner.clone())));
    let viewer = gateway.attach_authenticated_instance("viewer").unwrap();
    let manager = gateway.attach_authenticated_instance("manager").unwrap();
    assert_eq!(
        gateway
            .conversation_policy(&viewer, &owner, "conversation")
            .unwrap(),
        (SequentialChronological, 1)
    );
    assert!(
        gateway
            .set_conversation_policy(&viewer, &owner, "conversation", VisibleAutoBranch)
            .is_err()
    );
    assert!(
        gateway
            .set_conversation_policy(&manager, &owner, "conversation", VisibleAutoBranch)
            .unwrap()
    );
    assert_eq!(
        gateway
            .conversation_policy(&viewer, &owner, "conversation")
            .unwrap(),
        (VisibleAutoBranch, 2)
    );
    gateway.detach_instance(&viewer).unwrap();
    gateway.detach_instance(&manager).unwrap();
    drop(gateway.unmount_rel(&owner).unwrap());
    drop(gateway);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened.conversation_policy("conversation"),
        (VisibleAutoBranch, 2)
    );
    drop(reopened);
    std::fs::remove_dir_all(dir).unwrap();
}
