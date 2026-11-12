use super::*;
use crate::{
    ConfiguredRuntime, EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingNormalization, Gateway,
    GeneralEndpoint, GeneralEndpointError, HostCredentialBinding, HostedEmbeddingInputSemantics,
    HostedInferenceIdentity, OwnerScope, Rel, SwitchboardEmbeddingMode, TrustedHostAdmission,
};
use serde_json::{Value, json};
use std::sync::Arc;

struct TestGeneral;

impl GeneralEndpoint for TestGeneral {
    fn model(&self) -> &str {
        "general-test"
    }

    fn complete_json(
        &self,
        _system_prompt: &str,
        _user_payload: &str,
        _schema_name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        Ok(json!({"ok": true}))
    }
}

struct TestEmbedding;

impl EmbeddingEndpoint for TestEmbedding {
    fn model(&self) -> Option<&str> {
        Some("embedding-test")
    }

    fn dimensions(&self) -> u32 {
        2
    }

    fn normalization(&self) -> EmbeddingNormalization {
        EmbeddingNormalization::L2
    }

    fn supports_bounded_embedding(&self) -> bool {
        true
    }

    fn embed_bounded(
        &self,
        _mode: SwitchboardEmbeddingMode,
        inputs: &[String],
        _max_response_bytes: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        Ok(inputs.iter().map(|_| vec![1.0, 0.0]).collect())
    }

    fn embed(
        &self,
        mode: SwitchboardEmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.embed_bounded(mode, inputs, usize::MAX)
    }
}

#[test]
fn root_lexical_search_requires_only_view_and_no_runtime() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join(format!("root-retrieval-{}.rel", uuid::Uuid::new_v4()));
    let rel = Rel::create(&path, OwnerScope::Project).unwrap();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true).unwrap();
    let manage_token = "m".repeat(48);
    let view_token = "v".repeat(48);
    let (admission, control) = TrustedHostAdmission::new([
        HostCredentialBinding::new(
            "manager",
            &manage_token,
            [OwnerGrant::ManageRel(owner), OwnerGrant::OperateRel(owner)],
        ),
        HostCredentialBinding::new("viewer", &view_token, [OwnerGrant::ViewRel(owner)]),
    ])
    .install(&gateway)
    .unwrap();
    let management = admission
        .authenticate(&manage_token)
        .unwrap()
        .workspace_management(owner)
        .unwrap();
    let created = management
        .create_memory(
            "Needle".into(),
            "Exact stored body".into(),
            "fact".into(),
            "note".into(),
            1,
        )
        .unwrap();
    let viewer = admission.authenticate(&view_token).unwrap();
    assert!(!viewer.has_active_vectors(owner).unwrap());
    let manager = admission.authenticate(&manage_token).unwrap();
    let runtime = ConfiguredRuntime::hosted(
        Arc::new(TestGeneral),
        Arc::new(TestEmbedding),
        HostedInferenceIdentity::new(
            "test",
            "general-test",
            "test",
            "embedding-test",
            2,
            HostedEmbeddingInputSemantics::Shared,
            EmbeddingNormalization::L2,
        )
        .unwrap(),
    )
    .unwrap();
    manager.build_vectors(owner, &runtime, 1).unwrap();
    assert!(viewer.has_active_vectors(owner).unwrap());
    let result = viewer
        .search(owner, "needle", SearchOptions::default(), None)
        .unwrap();
    assert_eq!(result.cut.owner_id, owner.to_string());
    assert_eq!(result.mode, SearchMode::Lexical);
    assert_eq!(result.memories[0].memory_id, created.id);
    assert_eq!(result.memories[0].content, "Exact stored body");
    assert!(result.memories[0].lexical_score.is_some());
    assert!(result.memories[0].semantic_score.is_none());
    assert!(result.cut.profile_id.is_none());

    for mode in [SearchMode::Hybrid, SearchMode::Semantic] {
        let error = viewer
            .search(
                owner,
                "needle",
                SearchOptions {
                    mode,
                    ..SearchOptions::default()
                },
                None,
            )
            .unwrap_err();
        assert_eq!(error.kind(), ProductErrorKind::Unavailable);
    }

    control.revoke(&view_token).unwrap();
    assert_eq!(
        viewer.has_active_vectors(owner).unwrap_err().kind(),
        ProductErrorKind::Revoked
    );
    assert_eq!(
        viewer
            .search(owner, "needle", SearchOptions::default(), None)
            .unwrap_err()
            .kind(),
        ProductErrorKind::Revoked
    );
    drop((management, viewer, admission, control, gateway, rel));
    std::fs::remove_file(path).unwrap();
}
