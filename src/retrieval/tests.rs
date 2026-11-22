use super::*;
use crate::{
    ConfiguredRuntime, EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingNormalization, Gateway,
    GeneralEndpoint, GeneralEndpointError, HostCredentialBinding, HostedEmbeddingInputSemantics,
    HostedInferenceIdentity, Rel, SwitchboardEmbeddingMode, TrustedHostAdmission,
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
fn archive_hybrid_search_keeps_lexical_hits_outside_vector_top_k() {
    use crate::ArchiveSearchOptions;
    use reliquary_owner_session::{
        ArchiveVectorRow, RelWriter,
        model::{Branch, TurnInput},
    };

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join(format!("root-archive-hybrid-{}.rel", uuid::Uuid::new_v4()));
    let rel = Rel::create(&path).unwrap();
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
    let generation = {
        let mut writer = RelWriter::open_path_current(&path).unwrap();
        for (name, text, timestamp_ns) in [
            ("keyword", "quasar handbook", 1),
            ("semantic", "vector evidence", 2),
        ] {
            writer
                .accept_turn_and_advance_branch(
                    TurnInput {
                        id: "source".into(),
                        conversation_id: name.into(),
                        parent_id: None,
                        role: "user".into(),
                        principal_id: None,
                        timestamp_ns,
                        content: text.into(),
                        attachments: vec![],
                        project_attachments: vec![],
                    },
                    Branch {
                        id: "main".into(),
                        conversation_id: name.into(),
                        leaf_node_id: "source".into(),
                        canonical: true,
                    },
                )
                .unwrap();
        }
        let profile = runtime.probe_embedding_profile().unwrap().unwrap();
        let profile = writer.publish_compatibility_profile(profile).unwrap();
        let input = writer.archive_vector_input(profile).unwrap();
        let rows = input
            .sources()
            .iter()
            .map(|source| ArchiveVectorRow {
                fragment_id: source.fragment_id,
                max_archive_version: source.max_archive_version,
                vector: if source.text.contains("quasar") {
                    vec![0.0, 1.0]
                } else {
                    vec![0.8, 0.6]
                },
            })
            .collect::<Vec<_>>();
        writer
            .publish_archive_vector_batch(
                input.owner_id(),
                profile,
                input.source_archive_version(),
                &rows,
            )
            .unwrap()
            .unwrap()
    };

    let before = std::fs::read(&path).unwrap();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, false).unwrap();
    let token = "v".repeat(48);
    let (admission, control) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "archive-reader",
        &token,
        [OwnerGrant::ViewRel(owner)],
    )])
    .install(&gateway)
    .unwrap();
    let viewer = admission.authenticate(&token).unwrap();
    let profile = generation.compatibility_profile_id;
    let vector_only = viewer
        .search_archive_vectors(owner, profile, &[1.0, 0.0], 1)
        .unwrap();
    let hybrid = viewer
        .search_archive(
            owner,
            "quasar",
            ArchiveSearchOptions {
                limit: Some(1),
                ..Default::default()
            },
            &runtime,
        )
        .unwrap();
    assert_eq!(
        hybrid,
        viewer
            .search_archive_hybrid(owner, profile, "quasar", &[1.0, 0.0], 1)
            .unwrap()
    );
    assert_eq!(hybrid.generation_id, generation.id.0);
    assert_eq!(
        hybrid.source_archive_version,
        generation.source_archive_version
    );
    assert_eq!(hybrid.fragments_scored, 2);
    assert_eq!(hybrid.fragments_with_text, 2);
    assert!(hybrid.truncated);
    assert_eq!(hybrid.hits.len(), 1);
    let hit = &hybrid.hits[0];
    assert_ne!(hit.fragment_id, vector_only.hits[0].fragment_id);
    assert_eq!(hit.conversation_id, "keyword");
    assert_eq!(hit.start_node_id, "source");
    assert_eq!(hit.end_node_id, "source");
    assert_eq!(hit.lexical_score, Some(hit.score));
    assert!(hit.semantic_score.is_none());
    assert_eq!(
        viewer
            .search_archive_hybrid(owner, profile, "quasar", &[1.0], 1)
            .unwrap_err()
            .kind(),
        ProductErrorKind::InvalidInput,
    );
    control.revoke(&token).unwrap();
    assert_eq!(
        viewer
            .search_archive(owner, "quasar", ArchiveSearchOptions::default(), &runtime)
            .unwrap_err()
            .kind(),
        ProductErrorKind::Revoked,
    );
    drop((viewer, control, admission, gateway, rel));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn root_lexical_search_requires_only_view_and_no_runtime() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join(format!("root-retrieval-{}.rel", uuid::Uuid::new_v4()));
    let rel = Rel::create(&path).unwrap();
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
