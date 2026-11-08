use reliquary_memory::{
    CreateMemoryRequest, Gateway, GraphNode, HostCredentialBinding, InspectionCut, InspectionItem,
    InspectionQuery, MemoryAssociationAction, MemoryAssociationRequest, MemoryId, OwnerGrant,
    OwnerScope, ProductErrorKind, Rel, RelationKind, RelationOrigin, TrustedHostAdmission,
    WorkspaceManagement,
};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const USER: &str = "fixture-memory-association-manager-credential";
const VIEWER: &str = "fixture-memory-association-viewer-credential";

struct OwnerFile(PathBuf);

impl OwnerFile {
    fn create() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        std::fs::create_dir_all(&root).unwrap();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = root.join(format!("association-{}-{stamp}.rel", std::process::id()));
        Rel::create(&path, OwnerScope::Project).unwrap();
        Self(path)
    }

    fn mount(&self) -> (WorkspaceManagement, WorkspaceManagement) {
        let gateway = Gateway::new();
        let rel = Rel::open(&self.0).unwrap();
        let id = gateway.mount_rel(&rel, true).unwrap();
        let (host, _) = TrustedHostAdmission::new([
            HostCredentialBinding::new("manager", USER, [OwnerGrant::ManageRel(id)]),
            HostCredentialBinding::new("viewer", VIEWER, [OwnerGrant::ViewRel(id)]),
        ])
        .install(&gateway)
        .unwrap();
        (
            host.authenticate(USER)
                .unwrap()
                .workspace_management(id)
                .unwrap(),
            host.authenticate(VIEWER)
                .unwrap()
                .workspace_management(id)
                .unwrap(),
        )
    }
}

impl Drop for OwnerFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn create(owner: &WorkspaceManagement, key: &str) -> MemoryId {
    let memory = owner
        .create_memory_with_request(CreateMemoryRequest {
            request_id: key.into(),
            title: key.into(),
            content: format!("Content for {key}"),
            category: "fact".into(),
            memory_type: "note".into(),
            created_at_ns: 42,
        })
        .unwrap();
    MemoryId(std::array::from_fn(|i| {
        u8::from_str_radix(&memory.id[2 * i..2 * i + 2], 16).unwrap()
    }))
}

fn cut(owner: &WorkspaceManagement) -> InspectionCut {
    owner
        .inspect(&InspectionQuery::Memories, None, 1)
        .unwrap()
        .cut
}

#[test]
fn checked_user_link_unlink_and_reopen_replay() {
    let file = OwnerFile::create();
    let (original, unlink_cut, link, unlink, original_effect, terminal_effect) = {
        let (owner, viewer) = file.mount();
        let source = create(&owner, "first");
        let target = create(&owner, "second");
        let original = cut(&owner);
        let link = MemoryAssociationRequest {
            request_id: "add".into(),
            source_id: source,
            target_id: target,
            source_revision: 1,
            target_revision: 1,
            expected_edge_id: None,
            expected_edge_revision: None,
            action: MemoryAssociationAction::Link,
        };
        assert_eq!(
            viewer
                .change_memory_association(original, link.clone())
                .unwrap_err()
                .kind(),
            ProductErrorKind::Unauthorized
        );
        let mut stale = link.clone();
        stale.target_revision = 2;
        assert_eq!(
            owner
                .change_memory_association(original, stale)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Stale
        );

        let linked = owner
            .change_memory_association(original, link.clone())
            .unwrap();
        assert!(!linked.replayed);
        assert!(linked.edge.edge.active);
        assert_eq!(linked.edge.edge.kind, RelationKind::Topical);
        assert_eq!(linked.edge.edge.origin, RelationOrigin::User);
        assert_eq!(linked.cut.graph, original.graph + 1);
        let graph = owner
            .inspect(
                &InspectionQuery::MemoryGraph(reliquary_memory::MemoryGraphQuery {
                    seed_ids: vec![source],
                    community_id: None,
                    depth: 1,
                    node_limit: 4,
                    edge_limit: 4,
                    expected_cut: Some(linked.cut),
                }),
                None,
                1,
            )
            .unwrap();
        let [InspectionItem::MemoryGraphPage(page)] = graph.items.as_slice() else {
            panic!("missing graph")
        };
        assert_eq!(page.edges, vec![linked.edge.clone()]);

        let unlink = MemoryAssociationRequest {
            request_id: "remove".into(),
            expected_edge_id: Some(linked.edge.id.clone()),
            expected_edge_revision: Some(linked.edge.edge.graph_version),
            action: MemoryAssociationAction::Unlink,
            ..link.clone()
        };
        let unlinked = owner
            .change_memory_association(linked.cut, unlink.clone())
            .unwrap();
        assert!(!unlinked.edge.edge.active);
        assert_eq!(unlinked.edge.id, linked.edge.id);
        assert_eq!(unlinked.cut.graph, linked.cut.graph + 1);
        let mut mismatch = link.clone();
        mismatch.source_revision = 2;
        assert_eq!(
            owner
                .change_memory_association(original, mismatch)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Conflict
        );
        (
            original,
            linked.cut,
            link,
            unlink,
            linked.edge,
            unlinked.edge,
        )
    };

    let before = std::fs::read(&file.0).unwrap();
    {
        let (owner, _) = file.mount();
        let replay = owner
            .change_memory_association(original, link.clone())
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.edge, original_effect);
        let replay = owner.change_memory_association(unlink_cut, unlink).unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.edge, terminal_effect);
        let graph = owner
            .inspect(
                &InspectionQuery::Edge {
                    source: GraphNode::Memory(link.source_id.0),
                    target: GraphNode::Memory(link.target_id.0),
                    kind: RelationKind::Topical,
                },
                None,
                1,
            )
            .unwrap();
        let [InspectionItem::Edge(edge)] = graph.items.as_slice() else {
            panic!("missing terminal edge")
        };
        assert_eq!(edge, &terminal_effect.edge);
    }
    assert_eq!(std::fs::read(&file.0).unwrap(), before);
}
