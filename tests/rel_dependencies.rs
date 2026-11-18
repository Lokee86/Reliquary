use reliquary_container::Container;
use reliquary_ego_store::EgoStore;
use reliquary_memory::{
    AuthorizedContextWorkflow, CompactionBudget, ContextMessage, ContextRole, ContextTokenCounter,
    ContextTurn, ContextView, Gateway, HostCredentialBinding, Instance, OwnerGrant, OwnerId,
    ProductError, ProductErrorKind, Rel, TrustedHostAdmission,
};
use reliquary_owner_session::RelWriter;
use std::{fs, path::PathBuf, sync::Barrier};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("rel-dependencies-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn mount(
        &self,
        gateway: &Gateway,
        name: &str,
        id: u8,
        synthesis: Option<&str>,
        writable: bool,
    ) -> OwnerId {
        let path = self.0.join(format!("{name}.rel"));
        let owner_id = [id; 16];
        drop(RelWriter::create_current_path(&path, owner_id).unwrap());
        if let Some(text) = synthesis {
            let mut container = Container::open_writer(&path).unwrap();
            EgoStore::open(&container)
                .unwrap()
                .put_synthesis(&mut container, 0, 0, text.into())
                .unwrap();
        }
        gateway
            .mount_rel(&Rel::open(&path).unwrap(), writable)
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Words;

impl ContextTokenCounter for Words {
    fn count_text(&self, text: &str) -> u64 {
        text.split_whitespace().count() as u64
    }

    fn count_message(&self, message: &ContextMessage) -> u64 {
        self.count_text(&message.content) + 1
    }
}

fn prepare(
    instance: &Instance,
    owner: OwnerId,
    budget: CompactionBudget,
) -> Result<AuthorizedContextWorkflow, ProductError> {
    let native = ContextView {
        turns: ["earlier", "middle", "primary"]
            .into_iter()
            .map(|id| ContextTurn {
                message_id: id.into(),
                role: ContextRole::User,
                content: format!("{id} discussion item"),
            })
            .collect(),
    };
    instance.prepare_context_workflow(
        owner,
        "conversation",
        native,
        &[],
        &[],
        &[],
        None,
        220,
        budget,
        &Words,
    )
}

#[test]
fn rel_dependency_graph_inherits_only_current_authorized_context() {
    let fixture = Fixture::new();
    let gateway = Gateway::new();
    let root = fixture.mount(&gateway, "primary", 50, Some("primary Ego"), true);
    let left = fixture.mount(&gateway, "left", 20, None, true);
    let right = fixture.mount(&gateway, "right", 30, Some("right Ego"), true);
    let shared = fixture.mount(&gateway, "shared", 10, Some("shared Ego"), true);
    let deep = fixture.mount(&gateway, "deep", 5, Some("deep Ego"), true);
    let behind_left = fixture.mount(&gateway, "behind-left", 40, Some("hidden path Ego"), true);
    let sibling = fixture.mount(&gateway, "sibling", 80, Some("sibling Ego"), false);
    let manager_token = "rel-dependency-manager-000000000000";
    let viewer_token = "rel-dependency-viewer-0000000000000";
    let limited_token = "rel-dependency-limited-000000000000";
    let (adapter, control) = TrustedHostAdmission::new([
        HostCredentialBinding::new(
            "management-test",
            manager_token,
            [root, left, right, shared, deep, behind_left, sibling].map(OwnerGrant::ManageRel),
        ),
        HostCredentialBinding::new(
            "viewer-test",
            viewer_token,
            [root, right, shared, deep, behind_left, sibling].map(OwnerGrant::ViewRel),
        ),
        HostCredentialBinding::new(
            "limited-test",
            limited_token,
            [OwnerGrant::ManageRel(root), OwnerGrant::ViewRel(right)],
        ),
    ])
    .install(&gateway)
    .unwrap();
    let manager = adapter.authenticate(manager_token).unwrap();
    let viewer = adapter.authenticate(viewer_token).unwrap();
    let limited = adapter
        .authenticate(limited_token)
        .unwrap()
        .workspace_management(root)
        .unwrap();
    let primary = manager.workspace_management(root).unwrap();
    let left_owner = manager.workspace_management(left).unwrap();
    let deep_owner = manager.workspace_management(deep).unwrap();
    manager
        .workspace_management(shared)
        .unwrap()
        .replace_rel_dependencies(0, &[deep])
        .unwrap();
    manager
        .workspace_management(right)
        .unwrap()
        .replace_rel_dependencies(0, &[shared])
        .unwrap();
    left_owner
        .replace_rel_dependencies(0, &[behind_left, shared])
        .unwrap();
    let accepted = primary
        .replace_rel_dependencies(0, &[right, left, left])
        .unwrap();
    assert_eq!(accepted.declaration.dependencies, [left, right]);
    assert!(accepted.changed);
    assert_eq!(accepted.declaration.generation, 1);
    assert_eq!(
        limited
            .replace_rel_dependencies(1, &[left])
            .unwrap_err()
            .kind(),
        ProductErrorKind::Unauthorized
    );
    assert!(
        !primary
            .replace_rel_dependencies(1, &[left, right])
            .unwrap()
            .changed
    );
    assert_eq!(
        primary
            .replace_rel_dependencies(0, &[left, right])
            .unwrap_err()
            .kind(),
        ProductErrorKind::Stale
    );
    assert_eq!(
        manager
            .workspace_management(sibling)
            .unwrap()
            .replace_rel_dependencies(0, &[])
            .unwrap_err()
            .kind(),
        ProductErrorKind::WriterUnavailable
    );

    assert_eq!(
        primary.resolve_rel_dependencies().unwrap(),
        [deep, shared, behind_left, left, right, root]
    );
    let view = viewer.workspace_management(root).unwrap();
    assert_eq!(view.rel_dependencies().unwrap(), accepted.declaration);
    assert_eq!(
        view.replace_rel_dependencies(1, &[]).unwrap_err().kind(),
        ProductErrorKind::Unauthorized
    );
    assert_eq!(
        view.resolve_rel_dependencies().unwrap(),
        [deep, shared, right, root]
    );

    let budget = CompactionBudget {
        trigger_tokens: 200,
        target_tokens: 160,
        raw_tail_tokens: 9,
        summary_tokens: 16,
        compaction_input_tokens: 240,
    };
    let prepared = prepare(&viewer, root, budget).unwrap();
    let request = viewer.context_request(&prepared).unwrap();
    assert_eq!(request.messages.len(), 6);
    for (message, body) in request.messages.iter().zip([
        "deep Ego",
        "shared Ego",
        "right Ego",
        "earlier discussion item",
    ]) {
        assert!(message.content.ends_with(body), "{}", message.content);
    }
    assert!(
        request
            .messages
            .iter()
            .all(|message| !message.content.contains("hidden path Ego"))
    );
    let plan = viewer.context_compaction_plan(&prepared).unwrap().unwrap();
    assert!(
        plan.messages
            .iter()
            .all(|message| !message.content.contains("Ego"))
    );
    assert_eq!(
        prepare(
            &viewer,
            root,
            CompactionBudget {
                target_tokens: 12,
                ..budget
            }
        )
        .err()
        .unwrap()
        .kind(),
        ProductErrorKind::InvalidInput
    );

    deep_owner.quiesce().unwrap();
    assert_eq!(
        limited
            .replace_rel_dependencies(1, &[right])
            .unwrap_err()
            .kind(),
        ProductErrorKind::Unavailable
    );
    assert_eq!(primary.rel_dependencies().unwrap(), accepted.declaration);
    assert_eq!(
        view.resolve_rel_dependencies().unwrap_err().kind(),
        ProductErrorKind::Unavailable
    );
    assert_eq!(
        viewer.context_request(&prepared).unwrap_err().kind(),
        ProductErrorKind::Stale
    );
    deep_owner.reopen().unwrap();
    assert_eq!(
        viewer.context_request(&prepared).unwrap_err().kind(),
        ProductErrorKind::Stale
    );
    let prepared = prepare(&viewer, root, budget).unwrap();
    deep_owner
        .create_memory(
            "new fact".into(),
            "new content".into(),
            "fact".into(),
            "project".into(),
            100,
        )
        .unwrap();
    assert_eq!(
        viewer.context_request(&prepared).unwrap_err().kind(),
        ProductErrorKind::Stale
    );
    let prepared = prepare(&viewer, root, budget).unwrap();
    let request = viewer.context_request(&prepared).unwrap();
    assert!(
        request
            .messages
            .iter()
            .all(|message| !message.content.contains("deep Ego"))
    );

    let stale = viewer.compact_context_with(
        &prepared,
        |_| {
            primary
                .replace_rel_dependencies(1, &[])
                .map_err(|error| error.to_string())?;
            Ok("unused summary".into())
        },
        &Words,
    );
    assert_eq!(stale.err().unwrap().kind(), ProductErrorKind::Stale);
    assert_eq!(primary.resolve_rel_dependencies().unwrap(), [root]);
    let prepared = prepare(&viewer, root, budget).unwrap();
    control.revoke(viewer_token).unwrap();
    assert_eq!(
        viewer.context_request(&prepared).unwrap_err().kind(),
        ProductErrorKind::Revoked
    );

    let barrier = Barrier::new(2);
    let (a, b) = std::thread::scope(|scope| {
        let forward = scope.spawn(|| {
            barrier.wait();
            primary.replace_rel_dependencies(2, &[left])
        });
        let reverse = scope.spawn(|| {
            barrier.wait();
            left_owner.replace_rel_dependencies(1, &[root])
        });
        (forward.join().unwrap(), reverse.join().unwrap())
    });
    assert!(
        matches!((&a, &b), (Ok(update), Err(error)) | (Err(error), Ok(update))
        if update.changed && error.kind() == ProductErrorKind::Conflict)
    );
    assert_eq!(
        primary.rel_dependencies().unwrap().generation
            + left_owner.rel_dependencies().unwrap().generation,
        4
    );
}
