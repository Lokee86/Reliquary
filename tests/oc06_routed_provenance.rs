use reliquary_archive::{Branch, EpisodeBoundary, EpisodeOrigin, TurnInput};
use reliquary_memory::{
    Gateway, HostCredentialBinding, InspectionItem, InspectionQuery, OwnerGrant, OwnerKind,
    OwnerScope, Phylactery, Rel, SourceAvailability, TrustedHostAdmission,
};
use reliquary_owner_session::model::{MemoryDraft, MemorySourceRef};
use reliquary_owner_session::{PhyWriter, RelWriter};

#[test]
fn oc06_routed_provenance_requires_both_grants_and_does_not_replace_terminal_source() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("oc06-routed-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let rel_path = root.join("source.rel");
    let phy_path = root.join("private.phy");

    let source = Rel::create(&rel_path, OwnerScope::Project).unwrap();
    let source_owner = source.owner_id();
    let private = Phylactery::create(&phy_path).unwrap();
    let private_owner = private.owner_id();
    // Seed a real owner-validated route. All observation below uses Root.
    let mut rel = RelWriter::open_path_current(&rel_path).unwrap();
    let mut phy = PhyWriter::open_path_current(&phy_path).unwrap();
    let text = "original private evidence";
    rel.accept_turn_and_advance_branch(
        TurnInput {
            id: "exact-node".into(),
            conversation_id: "source-conversation".into(),
            parent_id: None,
            role: "user".into(),
            principal_id: Some("alice".into()),
            timestamp_ns: 42,
            content: text.into(),
            attachments: vec![],
            project_attachments: vec![],
        },
        Branch {
            id: "main".into(),
            conversation_id: "source-conversation".into(),
            leaf_node_id: "exact-node".into(),
            canonical: true,
        },
    )
    .unwrap();
    let episode = rel
        .finalize_episode(
            "source-conversation",
            "exact-node",
            "exact-node",
            EpisodeOrigin::Live,
            EpisodeBoundary::Explicit,
            43,
        )
        .unwrap();
    let draft = MemoryDraft {
        title: "Routed private Memory".into(),
        content: text.into(),
        category: "fact".into(),
        memory_type: "note".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "user".into(),
        lifecycle_state: "extracted".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: Some(MemorySourceRef {
            owner_id: source_owner.to_string(),
            principal_id: Some("alice".into()),
            source_episode_id: episode.0,
            source_node_id: "exact-node".into(),
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
        }),
        source_time_ns: Some(42),
        temporal_inference: None,
        mutation_id: "routed-source-fixture".into(),
        created_at_ns: 43,
        updated_at_ns: 43,
    };
    let proof = rel
        .read_session()
        .prove_routed_user_memory(&phy, &draft, text, None)
        .unwrap();
    let (revision, _) = phy.publish_routed_user_memory(proof).unwrap();
    drop((rel, phy));

    let query = InspectionQuery::Provenance {
        id: revision.record.id,
        revision: Some(revision.record.revision),
    };
    for terminal in [false, true] {
        if terminal {
            let mut writer = RelWriter::open_path_current(&rel_path).unwrap();
            writer
                .tombstone_conversation("source-conversation")
                .unwrap();
        }
        let gateway = Gateway::new();
        gateway.mount_rel(&source, true).unwrap();
        gateway.mount_phylactery(&private, true).unwrap();
        let phy_token = "p".repeat(48);
        let both_token = "b".repeat(48);
        let (adapter, _) = TrustedHostAdmission::new([
            HostCredentialBinding::new(
                "alice",
                &phy_token,
                [OwnerGrant::ViewPhylactery(private_owner)],
            ),
            HostCredentialBinding::new(
                "alice",
                &both_token,
                [
                    OwnerGrant::ViewPhylactery(private_owner),
                    OwnerGrant::ViewRel(source_owner),
                ],
            ),
        ])
        .install(&gateway)
        .unwrap();
        let inspect = |token| {
            adapter
                .authenticate(token)
                .unwrap()
                .inspect_owner(private_owner, OwnerKind::Phylactery, &query, None, 1)
                .unwrap()
        };
        let restricted = inspect(&phy_token);
        let [InspectionItem::Provenance(result)] = restricted.items.as_slice() else {
            panic!("expected provenance");
        };
        assert_eq!(result.availability, SourceAvailability::Restricted);
        assert!(result.source_owner_id.is_none());
        assert!(result.source_time_ns.is_none());
        assert!(!format!("{restricted:?}").contains("exact-node"));
        let authorized = inspect(&both_token);
        let [InspectionItem::Provenance(result)] = authorized.items.as_slice() else {
            panic!("expected provenance");
        };
        assert_eq!(result.source_time_ns, Some(42));
        assert_eq!(result.created_at_ns, 43);
        let turn = result.source_turn.as_ref().unwrap();
        assert_eq!(turn.node_id, "exact-node");
        assert_eq!(turn.archive_revision, Some(1));
        if terminal {
            assert_eq!(turn.availability, SourceAvailability::Terminal);
            assert!(turn.content.is_none());
        } else {
            assert_eq!(turn.availability, SourceAvailability::Available);
            assert_eq!(turn.content.as_deref(), Some(text));
        }
    }
    drop((source, private));
    std::fs::remove_dir_all(root).unwrap();
}
