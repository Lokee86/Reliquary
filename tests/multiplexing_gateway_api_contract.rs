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
