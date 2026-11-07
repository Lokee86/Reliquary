use reliquary_memory::{
    Gateway, HostCredentialBinding, OwnerGrant, OwnerScope, Phylactery, ProductErrorKind, Rel,
    TrustedHostAdmission,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct TestPaths {
    root: PathBuf,
}
impl TestPaths {
    fn new(tag: &str) -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "public-{tag}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }
    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}
impl Drop for TestPaths {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn current_rel_and_phylactery_reopen_with_stable_identity_and_kind_checks() {
    let temp = TestPaths::new("owners");
    let rel_path = temp.path("owner.rel");
    let phy_path = temp.path("owner.phy");
    let rel = Rel::create(&rel_path, OwnerScope::Connection).unwrap();
    let phy = Phylactery::create(&phy_path).unwrap();
    assert_eq!(Rel::open(&rel_path).unwrap().owner_id(), rel.owner_id());
    assert_eq!(
        Phylactery::open(&phy_path).unwrap().owner_id(),
        phy.owner_id()
    );
    assert_eq!(rel.info().unwrap().format, "2.1");
    assert_eq!(
        Rel::open(&phy_path).err().unwrap().kind(),
        ProductErrorKind::WrongFormat
    );
    assert_eq!(
        Phylactery::open(&rel_path).err().unwrap().kind(),
        ProductErrorKind::WrongFormat
    );
}

#[test]
fn trusted_management_requires_manage_grant_and_revocation_fences_reads() {
    let temp = TestPaths::new("management");
    let rel = Rel::create(temp.path("owner.rel"), OwnerScope::Project).unwrap();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true).unwrap();
    let manage_token = "m".repeat(48);
    let view_token = "v".repeat(48);
    let admission = TrustedHostAdmission::new([
        HostCredentialBinding::new("manager", &manage_token, [OwnerGrant::ManageRel(owner)]),
        HostCredentialBinding::new("viewer", &view_token, [OwnerGrant::ViewRel(owner)]),
    ]);
    let (adapter, control) = admission.install(&gateway).unwrap();
    let manager = adapter.authenticate(&manage_token).unwrap();
    let viewer = adapter.authenticate(&view_token).unwrap();
    let view = viewer.workspace_management(owner).unwrap();
    assert_eq!(
        view.create_memory(
            "title".into(),
            "body".into(),
            "fact".into(),
            "note".into(),
            1
        )
        .unwrap_err()
        .kind(),
        ProductErrorKind::Unauthorized
    );
    let management = manager.workspace_management(owner).unwrap();
    let memory = management
        .create_memory(
            "title".into(),
            "body".into(),
            "fact".into(),
            "note".into(),
            1,
        )
        .unwrap();
    assert_eq!(management.statistics().unwrap().memories.memories, 1);
    assert_eq!(
        management
            .provenance(reliquary_memory::MemoryId(parse_hex(&memory.id)))
            .unwrap()
            .principal_id
            .as_deref(),
        Some("manager")
    );
    management.quiesce().unwrap();
    assert_eq!(management.reconcile_quiesced().unwrap().owner_id, owner);
    management.reopen().unwrap();
    assert_eq!(
        management
            .memory(reliquary_memory::MemoryId(parse_hex(&memory.id)))
            .unwrap()
            .content,
        "body"
    );
    control.revoke(&manage_token).unwrap();
    assert_eq!(
        management.health().unwrap_err().kind(),
        ProductErrorKind::Revoked
    );
    assert_eq!(
        management.quiesce().unwrap_err().kind(),
        ProductErrorKind::Revoked
    );
}

#[test]
fn host_grants_do_not_cross_rel_and_phylactery_boundaries() {
    let temp = TestPaths::new("privacy");
    let rel = Rel::create(temp.path("owner.rel"), OwnerScope::Project).unwrap();
    let phy = Phylactery::create(temp.path("owner.phy")).unwrap();
    let gateway = Gateway::new();
    let rel_owner = gateway.mount_rel(&rel, false).unwrap();
    let phy_owner = gateway.mount_phylactery(&phy, false).unwrap();
    let token = "r".repeat(48);
    let (adapter, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "rel-only",
        &token,
        [OwnerGrant::ViewRel(rel_owner)],
    )])
    .install(&gateway)
    .unwrap();
    let instance = adapter.authenticate(&token).unwrap();
    assert_eq!(instance.rel_info(rel_owner).unwrap().owner_id, rel_owner);
    assert_eq!(
        instance.phylactery_info(phy_owner).unwrap_err().kind(),
        ProductErrorKind::Unauthorized
    );
    assert_eq!(
        instance
            .open_conversation_queue(temp.path("queue"), rel_owner)
            .err()
            .unwrap()
            .kind(),
        ProductErrorKind::Unauthorized
    );
}

fn parse_hex(value: &str) -> [u8; 32] {
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap();
    }
    bytes
}
#[test]
fn public_copy_workflows_reopen_and_do_not_overwrite() {
    use reliquary_memory::{ReconciliationMode, RewritePolicy, execute_rewrite, reconcile_rel};

    let temp = TestPaths::new("copies");
    let source = temp.path("original.rel");
    let identical = temp.path("identical.rel");
    let rewritten = temp.path("rewritten.rel");
    let merged = temp.path("merged.rel");
    let rel = Rel::create(&source, OwnerScope::Project).unwrap();
    let owner_id = rel.owner_id();
    let plan = rel.plan_rewrite(RewritePolicy::default()).unwrap();
    let before = fs::read(&source).unwrap();
    fs::copy(&source, &identical).unwrap();

    let report = execute_rewrite(&plan, &source, &rewritten).unwrap();
    let fresh = Rel::open(&rewritten).unwrap();
    assert_eq!(fresh.owner_id().as_bytes(), report.destination_owner_id);
    assert_ne!(fresh.owner_id(), owner_id);
    assert_eq!(fs::read(&source).unwrap(), before);
    assert_eq!(
        execute_rewrite(&plan, &source, &rewritten)
            .unwrap_err()
            .kind(),
        ProductErrorKind::PublicationCollision
    );

    let merge = reconcile_rel(&source, &identical, &merged).unwrap();
    assert_eq!(merge.mode, ReconciliationMode::ExactCopy);
    assert_eq!(fs::read(&merged).unwrap(), before);
    assert_eq!(fs::read(&source).unwrap(), before);
    assert_eq!(
        reconcile_rel(&source, &identical, &merged)
            .unwrap_err()
            .kind(),
        ProductErrorKind::PublicationCollision
    );
}

#[test]
fn public_management_memory_history_and_proof_survive_reopen() {
    use reliquary_memory::{MemoryHistoryLookup, MemoryHistoryStatus, MemoryId};
    let temp = TestPaths::new("history");
    let path = temp.path("owner.rel");
    let rel = Rel::create(&path, OwnerScope::Project).unwrap();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true).unwrap();
    let token = "h".repeat(48);
    let (adapter, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "manager",
        &token,
        [OwnerGrant::ManageRel(owner)],
    )])
    .install(&gateway)
    .unwrap();
    let management = adapter
        .authenticate(&token)
        .unwrap()
        .workspace_management(owner)
        .unwrap();
    let memory = management
        .create_memory(
            "A title".into(),
            "A body".into(),
            "fact".into(),
            "project".into(),
            42,
        )
        .unwrap();
    management.quiesce().unwrap();
    let reopened = Rel::open(&path).unwrap();
    let id = MemoryId(parse_hex(&memory.id));
    let history = reopened.memory_history(id).unwrap();
    let MemoryHistoryLookup::Found(history) = history else {
        panic!("missing published history");
    };
    assert!(matches!(history.status, MemoryHistoryStatus::Available));
    assert_eq!(history.history.revisions.len(), 1);
    assert!(
        !reopened
            .current_historical_proof()
            .unwrap()
            .digest
            .is_empty()
    );
}
#[test]
fn public_host_commits_conversation_and_management_reads_bounded_transcript() {
    use reliquary_memory::{ConversationSubmissionId, FinalizeCapture, QueueState, SubmissionId};
    let temp = TestPaths::new("conversation");
    let rel = Rel::create(temp.path("owner.rel"), OwnerScope::Project).unwrap();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true).unwrap();
    let token = "q".repeat(48);
    let (adapter, control) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "operator",
        &token,
        [OwnerGrant::OperateRel(owner), OwnerGrant::ManageRel(owner)],
    )])
    .install(&gateway)
    .unwrap();
    let instance = adapter.authenticate(&token).unwrap();
    let mut queue = instance
        .open_conversation_queue(temp.path("queue"), owner)
        .unwrap();
    let submission = SubmissionId::new("submission-1").unwrap();
    let id = ConversationSubmissionId::new("conversation-1", submission.clone()).unwrap();
    instance
        .submit_conversation(
            &mut queue,
            "conversation-1",
            submission,
            "visible committed content",
            0,
        )
        .unwrap();
    let status = instance
        .finalize_conversation_submission(
            &mut queue,
            &id,
            1,
            FinalizeCapture {
                branch_id: "main".into(),
                parent_node_id: None,
                committed_timestamp_ns: 42,
                verified_principal_id: "operator".into(),
            },
        )
        .unwrap();
    assert_eq!(status.state, QueueState::Committed);
    let management = instance.workspace_management(owner).unwrap();
    let page = management.conversations(0, 4).unwrap();
    assert_eq!(page.items[0].conversation_id, "conversation-1");
    management
        .select_conversation("conversation-1", "submission-1")
        .unwrap();
    let transcript = management.transcript(0, 4).unwrap();
    assert_eq!(transcript.items[0].content, "visible committed content");
    management
        .set_title("conversation-1", "Updated title")
        .unwrap();
    assert_eq!(
        management.conversations(0, 4).unwrap().items[0]
            .title
            .as_deref(),
        Some("Updated title")
    );
    assert!(management.conversations(0, usize::MAX).is_err());
    control.revoke(&token).unwrap();
    assert_eq!(
        management.transcript(0, 4).unwrap_err().kind(),
        ProductErrorKind::Revoked
    );
}
