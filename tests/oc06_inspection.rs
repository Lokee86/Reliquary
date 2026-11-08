use reliquary_memory::{
    ConversationSubmissionId, CreateMemoryRequest, CreateRelationshipRequest, EntityId,
    FinalizeCapture, Gateway, GraphNode, HostCredentialBinding, InspectionItem, InspectionQuery,
    MAX_MANAGEMENT_PAGE_BYTES, MAX_MANAGEMENT_PAGE_ITEMS, MemoryBodyId, MemoryId, MemoryTextField,
    OwnerGrant, OwnerKind, OwnerScope, Phylactery, ProductErrorKind, Rel,
    RelationshipParticipantEvidence, SourceAvailability, SubmissionId, TrustedHostAdmission,
};
use std::path::PathBuf;

struct TestDirectory(PathBuf);
impl TestDirectory {
    fn create() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("oc06-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn id(value: &str) -> [u8; 32] {
    std::array::from_fn(|index| u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap())
}

fn draft(request_id: &str, content: String) -> CreateMemoryRequest {
    CreateMemoryRequest {
        request_id: request_id.into(),
        title: "Accepted title".into(),
        content,
        category: "fact".into(),
        memory_type: "note".into(),
        created_at_ns: 42,
    }
}

#[test]
fn oc06_mounted_inspection_enforces_grants_cuts_and_byte_budget() {
    let dir = TestDirectory::create();
    let rel = Rel::create(dir.0.join("project.rel"), OwnerScope::Project).unwrap();
    let phy = Phylactery::create(dir.0.join("private.phy")).unwrap();
    let other = Rel::create(dir.0.join("readonly.rel"), OwnerScope::Project).unwrap();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true).unwrap();
    let private = gateway.mount_phylactery(&phy, true).unwrap();
    let readonly = gateway.mount_rel(&other, false).unwrap();
    let manager_token = "m".repeat(48);
    let viewer_token = "v".repeat(48);
    let operator_token = "o".repeat(48);
    let (adapter, control) = TrustedHostAdmission::new([
        HostCredentialBinding::new(
            "manager",
            &manager_token,
            [
                OwnerGrant::ManageRel(owner),
                OwnerGrant::ManageRel(readonly),
                OwnerGrant::ViewPhylactery(private),
            ],
        ),
        HostCredentialBinding::new("viewer", &viewer_token, [OwnerGrant::ViewRel(owner)]),
        HostCredentialBinding::new("operator", &operator_token, [OwnerGrant::OperateRel(owner)]),
    ])
    .install(&gateway)
    .unwrap();
    let manager = adapter.authenticate(&manager_token).unwrap();
    let viewer = adapter.authenticate(&viewer_token).unwrap();
    let operator = adapter.authenticate(&operator_token).unwrap();
    let work = manager.workspace_management(owner).unwrap();
    let view = viewer.workspace_management(owner).unwrap();
    let operate = operator.workspace_management(owner).unwrap();
    let read = manager.workspace_management(readonly).unwrap();

    let large = draft("first", "a".repeat(140_000));
    let memory = work.create_memory_with_request(large.clone()).unwrap();
    assert_eq!(
        work.create_memory_with_request(large.clone()).unwrap(),
        memory
    );
    assert_eq!(
        work.create_memory_with_request(draft("first", "different".into()))
            .unwrap_err()
            .kind(),
        ProductErrorKind::Conflict
    );
    for denied in [&view, &operate] {
        assert_eq!(
            denied
                .create_memory_with_request(large.clone())
                .unwrap_err()
                .kind(),
            ProductErrorKind::Unauthorized
        );
    }
    assert_eq!(
        read.create_memory_with_request(large).unwrap_err().kind(),
        ProductErrorKind::WriterUnavailable
    );
    work.create_memory_with_request(draft("second", "b".repeat(140_000)))
        .unwrap();
    assert_eq!(manager.rel_info(owner).unwrap().memory_count, 2);
    assert_eq!(manager.phylactery_info(private).unwrap().memory_count, 0);
    assert_eq!(
        viewer
            .inspect_owner(
                private,
                OwnerKind::Phylactery,
                &InspectionQuery::Memories,
                None,
                0
            )
            .unwrap_err()
            .kind(),
        ProductErrorKind::Unauthorized
    );
    assert!(
        manager
            .inspect_owner(
                private,
                OwnerKind::Phylactery,
                &InspectionQuery::Memories,
                None,
                1
            )
            .unwrap()
            .items
            .is_empty()
    );
    let query = InspectionQuery::Memories;
    let page = view
        .inspect(&query, None, MAX_MANAGEMENT_PAGE_ITEMS)
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(serde_json::to_vec(&page).unwrap().len() <= MAX_MANAGEMENT_PAGE_BYTES);
    let cursor = page.next.as_ref().unwrap();
    let next = view
        .inspect(&query, Some(cursor), MAX_MANAGEMENT_PAGE_ITEMS)
        .unwrap();
    assert_eq!(next.cut, page.cut);
    assert_eq!(next.items.len(), 1);
    assert_ne!(next.items, page.items);
    assert!(next.next.is_none());
    assert_eq!(
        read.inspect(&query, Some(cursor), 1).unwrap_err().kind(),
        ProductErrorKind::Stale
    );
    assert_eq!(
        view.inspect(&InspectionQuery::Entities, Some(cursor), 1)
            .unwrap_err()
            .kind(),
        ProductErrorKind::Stale
    );
    assert_eq!(
        view.inspect(&query, None, MAX_MANAGEMENT_PAGE_ITEMS + 1)
            .unwrap_err()
            .kind(),
        ProductErrorKind::InvalidInput
    );

    let memory_id = MemoryId(id(&memory.id));
    let history = InspectionQuery::MemoryHistory {
        id: memory_id,
        memory_cut: Some(memory.memory_version),
    };
    assert_eq!(
        view.inspect(&history, None, 1).unwrap().items,
        vec![InspectionItem::Memory(memory.clone())]
    );
    let history = InspectionQuery::MemoryHistory {
        id: memory_id,
        memory_cut: Some(u64::MAX),
    };
    assert_eq!(
        view.inspect(&history, None, 1).unwrap_err().kind(),
        ProductErrorKind::Stale
    );
    let source = view
        .inspect(
            &InspectionQuery::Provenance {
                id: memory_id,
                revision: Some(memory.revision),
            },
            None,
            1,
        )
        .unwrap();
    let [InspectionItem::Provenance(source)] = source.items.as_slice() else {
        panic!("expected provenance")
    };
    assert_eq!(source.availability, SourceAvailability::Direct);
    assert_eq!(source.body_id, memory.body_id);
    assert_eq!(source.principal_id.as_deref(), Some("manager"));
    assert_eq!(source.source_time_ns, Some(42));
    assert!(source.source_turn.is_none());

    let relationship = CreateRelationshipRequest {
        owner_id: owner.as_bytes(),
        kind: "example".into(),
        participants: vec![RelationshipParticipantEvidence {
            entity_id: EntityId([0; 32]),
            entity_revision: 1,
            role: None,
        }],
        source_memory_id: memory_id,
        source_memory_revision: memory.revision,
        source_body_id: MemoryBodyId(id(&memory.body_id)),
        source_field: MemoryTextField::Content,
        start_byte: 0,
        end_byte: 1,
        source_quote: "z".into(),
        mutation_id: "forged-evidence".into(),
        updated_at_ns: 43,
    };
    assert_eq!(
        operate
            .create_relationship(relationship.clone())
            .unwrap_err()
            .kind(),
        ProductErrorKind::Unauthorized
    );
    assert_eq!(
        work.create_relationship(relationship.clone())
            .unwrap_err()
            .kind(),
        ProductErrorKind::InvalidInput
    );
    assert_eq!(
        work.create_relationship(CreateRelationshipRequest {
            source_memory_revision: memory.revision + 1,
            ..relationship
        })
        .unwrap_err()
        .kind(),
        ProductErrorKind::Stale
    );
    assert!(
        view.inspect(&InspectionQuery::Relationships, None, 1)
            .unwrap()
            .items
            .is_empty()
    );

    assert!(
        view.inspect(
            &InspectionQuery::Neighbors {
                anchor: GraphNode::Memory(memory_id.0)
            },
            None,
            1
        )
        .unwrap()
        .items
        .is_empty()
    );
    assert_eq!(
        view.inspect(
            &InspectionQuery::Neighbors {
                anchor: GraphNode::Memory([0; 32])
            },
            None,
            1
        )
        .unwrap_err()
        .kind(),
        ProductErrorKind::Unavailable
    );
    assert_eq!(view.inspect(&query, Some(cursor), 1).unwrap().cut, page.cut);
    work.create_memory_with_request(draft("third", "another".into()))
        .unwrap();
    assert_eq!(
        view.inspect(&query, Some(cursor), 1).unwrap_err().kind(),
        ProductErrorKind::Stale
    );
    control.revoke(&viewer_token).unwrap();
    assert_eq!(
        view.inspect(&query, None, 1).unwrap_err().kind(),
        ProductErrorKind::Revoked
    );
}

#[test]
fn oc06_archive_uses_exact_node_and_revision_across_branch_advances() {
    let dir = TestDirectory::create();
    let rel = Rel::create(dir.0.join("source.rel"), OwnerScope::Project).unwrap();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true).unwrap();
    let token = "s".repeat(48);
    let (adapter, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "operator",
        &token,
        [OwnerGrant::OperateRel(owner)],
    )])
    .install(&gateway)
    .unwrap();
    let instance = adapter.authenticate(&token).unwrap();
    let work = instance.workspace_management(owner).unwrap();
    let mut queue = instance
        .open_conversation_queue(dir.0.join("queue"), owner)
        .unwrap();
    for (node, parent, time, text) in [
        ("first", None, 42, "original exact text"),
        ("second", Some("first"), 43, "different current text"),
    ] {
        let submission = SubmissionId::new(node).unwrap();
        let id = ConversationSubmissionId::new("conversation", submission.clone()).unwrap();
        let revision = instance
            .conversation_revision(&queue, "conversation")
            .unwrap();
        instance
            .submit_conversation(&mut queue, "conversation", submission, text, revision)
            .unwrap();
        let revision = instance
            .conversation_revision(&queue, "conversation")
            .unwrap();
        instance
            .finalize_conversation_submission(
                &mut queue,
                &id,
                revision,
                FinalizeCapture {
                    branch_id: "main".into(),
                    parent_node_id: parent.map(str::to_owned),
                    committed_timestamp_ns: time,
                    verified_principal_id: "operator".into(),
                },
            )
            .unwrap();
    }
    let branches = work
        .inspect(
            &InspectionQuery::Branches {
                conversation: "conversation".into(),
            },
            None,
            1,
        )
        .unwrap();
    let [InspectionItem::Branch(branch)] = branches.items.as_slice() else {
        panic!("expected branch")
    };
    assert_eq!(branch.leaf_node_id, "second");
    let transcript = InspectionQuery::Transcript {
        conversation: "conversation".into(),
        leaf: branch.leaf_node_id.clone(),
    };
    let first = work.inspect(&transcript, None, 1).unwrap();
    let second = work.inspect(&transcript, first.next.as_ref(), 1).unwrap();
    let [InspectionItem::Turn(turn)] = second.items.as_slice() else {
        panic!("expected turn")
    };
    assert_eq!(turn.parent_node_id.as_deref(), Some("first"));
    let exact = work
        .inspect(
            &InspectionQuery::SourceTurn {
                conversation: "conversation".into(),
                node: "first".into(),
            },
            None,
            1,
        )
        .unwrap();
    assert_eq!(first.items, exact.items);
    let [InspectionItem::Turn(turn)] = exact.items.as_slice() else {
        panic!("expected exact turn")
    };
    assert_eq!(turn.content.as_deref(), Some("original exact text"));
    assert_eq!(turn.timestamp_ns, 42);
    assert_eq!(turn.availability, SourceAvailability::Available);
    assert!(turn.archive_revision.unwrap() < exact.cut.archive.unwrap());
    assert_eq!(
        work.inspect(
            &InspectionQuery::SourceTurn {
                conversation: "another-conversation".into(),
                node: "first".into()
            },
            None,
            1
        )
        .unwrap_err()
        .kind(),
        ProductErrorKind::Unavailable
    );
}
