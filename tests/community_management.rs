use reliquary_container::{Container, FileKind};
use reliquary_graph::{
    Community, CommunitySnapshot, CommunityStore, GraphAdmission, GraphStore, NodeRef,
    RelationChange, RelationKind, RelationOrigin, community_id,
};
use reliquary_memory::{
    CommunityNameOrigin, CommunityRenameRequest, Gateway, HostAdmissionAdapter,
    HostCredentialBinding, InspectionItem, InspectionQuery, OwnerGrant, OwnerId, OwnerKind,
    OwnerScope, ProductErrorKind, Rel, TrustedHostAdmission,
};
use std::path::{Path, PathBuf};

const MANAGER: &str = "community-proof-manager-0123456789";
const VIEWER: &str = "community-proof-viewer-0123456789";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("community-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn rel(&self) -> PathBuf {
        self.0.join("proof.rel")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.rel());
        let _ = std::fs::remove_file(self.0.join("forged.rel"));
        let _ = std::fs::remove_file(self.0.join("reclaimed.rel"));
        let _ = std::fs::remove_dir(&self.0);
    }
}

fn admit(gateway: &Gateway, path: &Path) -> (OwnerId, HostAdmissionAdapter) {
    let owner = gateway.mount_rel(&Rel::open(path).unwrap(), true).unwrap();
    let (adapter, _) = TrustedHostAdmission::new([
        HostCredentialBinding::new("manager", MANAGER, [OwnerGrant::ManageRel(owner)]),
        HostCredentialBinding::new("viewer", VIEWER, [OwnerGrant::ViewRel(owner)]),
    ])
    .install(gateway)
    .unwrap();
    (owner, adapter)
}

// The fixture starts with an unnamed, derived Community. Every tested user
// operation below goes through public Root and the mounted owner's writer.
fn seed(path: &Path) -> ([u8; 32], [[u8; 32]; 2]) {
    Rel::create(path, OwnerScope::Project).unwrap();
    let (owner, mut members) = {
        let gateway = Gateway::new();
        let (owner, adapter) = admit(&gateway, path);
        let instance = adapter.authenticate(MANAGER).unwrap();
        let management = instance.workspace_management(owner).unwrap();
        let ids = ["first", "second"].map(|name| {
            let memory = management
                .create_memory(
                    name.into(),
                    format!("{name} Community member"),
                    "fact".into(),
                    "note".into(),
                    1,
                )
                .unwrap();
            std::array::from_fn(|index| {
                u8::from_str_radix(&memory.id[index * 2..index * 2 + 2], 16).unwrap()
            })
        });
        (owner, ids)
    };
    members.sort_unstable();
    let mut file = Container::open_writer_as(path, FileKind::Reliquary).unwrap();
    let admission = GraphAdmission::for_changes(owner.as_bytes(), members, []);
    let mut graph = GraphStore::open(&file, &admission).unwrap();
    graph
        .put(
            &mut file,
            &admission,
            &[RelationChange {
                source: NodeRef::Memory(members[0]),
                target: NodeRef::Memory(members[1]),
                kind: RelationKind::Topical,
                active: true,
                origin: RelationOrigin::Dream,
            }],
        )
        .unwrap();
    let mut communities = CommunityStore::open(&file, &graph).unwrap();
    let id = community_id(owner.as_bytes(), &members);
    communities
        .publish(
            &mut file,
            &graph,
            CommunitySnapshot {
                generation: 1,
                graph_version: graph.graph_version(),
                communities: vec![Community {
                    id,
                    members: members.into(),
                }],
            },
        )
        .unwrap();
    (id.0, members)
}

#[test]
fn community_name_is_cut_bound_and_exact_receipt_survives_reopen() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let (id, _) = seed(&path);
    let initial_cut;
    let first_request = CommunityRenameRequest {
        request_id: "give-name".into(),
        community_id: id,
        expected_generation: 1,
        name: "Verified community".into(),
    };
    {
        let gateway = Gateway::new();
        let (owner, adapter) = admit(&gateway, &path);
        let viewer = adapter.authenticate(VIEWER).unwrap();
        let manager = adapter.authenticate(MANAGER).unwrap();
        let read = |query: &InspectionQuery, cursor| {
            viewer
                .inspect_owner(owner, OwnerKind::Rel, query, cursor, 1)
                .unwrap()
        };
        let before = read(&InspectionQuery::Communities, None);
        initial_cut = before.cut;
        let [InspectionItem::Community(unnamed)] = before.items.as_slice() else {
            panic!("expected Community");
        };
        assert_eq!(unnamed.name_origin, CommunityNameOrigin::Unnamed);
        assert_eq!(unnamed.name, None);

        let query = InspectionQuery::CommunityMembers {
            id,
            expected_generation: 1,
        };
        let first = read(&query, None);
        let second = read(&query, first.next.as_ref());
        assert_ne!(first.items, second.items);
        assert_eq!(first.items.len() + second.items.len(), 2);
        assert!(second.next.is_none());
        assert_eq!(first.cut, second.cut);

        assert_eq!(
            viewer
                .workspace_management(owner)
                .unwrap()
                .rename_community_at_cut(initial_cut, first_request.clone())
                .unwrap_err()
                .kind(),
            ProductErrorKind::Unauthorized,
        );
        let management = manager.workspace_management(owner).unwrap();
        let renamed = management
            .rename_community_at_cut(initial_cut, first_request.clone())
            .unwrap();
        assert!(renamed.changed && !renamed.replayed);
        assert_eq!(renamed.community.id, id);
        assert_eq!(renamed.community.generation, 2);
        assert_eq!(renamed.community.graph_version, initial_cut.graph);
        assert_eq!(renamed.community.member_count, 2);
        assert_eq!(renamed.community.name_origin, CommunityNameOrigin::User);
        assert_eq!(renamed.cut.community, Some(2));

        assert_eq!(
            viewer
                .inspect_owner(owner, OwnerKind::Rel, &query, first.next.as_ref(), 1)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Stale,
        );
        let mut stale = first_request.clone();
        stale.request_id = "new-request-at-old-cut".into();
        assert_eq!(
            management
                .rename_community_at_cut(initial_cut, stale)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Stale
        );
        let mut changed_payload = first_request.clone();
        changed_payload.name = "Different payload".into();
        assert_eq!(
            management
                .rename_community_at_cut(initial_cut, changed_payload)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Conflict
        );

        let no_op = CommunityRenameRequest {
            request_id: "same-name".into(),
            expected_generation: 2,
            ..first_request.clone()
        };
        let noop = management
            .rename_community_at_cut(renamed.cut, no_op.clone())
            .unwrap();
        assert!(!noop.changed && !noop.replayed);
        assert_eq!(noop.cut, renamed.cut);
        let replay = management
            .rename_community_at_cut(renamed.cut, no_op)
            .unwrap();
        assert!(!replay.changed && replay.replayed);
    }
    let after = std::fs::read(&path).unwrap();
    {
        let gateway = Gateway::new();
        let (owner, adapter) = admit(&gateway, &path);
        let manager = adapter.authenticate(MANAGER).unwrap();
        let page = manager
            .inspect_owner(
                owner,
                OwnerKind::Rel,
                &InspectionQuery::Communities,
                None,
                1,
            )
            .unwrap();
        let [InspectionItem::Community(named)] = page.items.as_slice() else {
            panic!("expected Community");
        };
        assert_eq!(named.name.as_deref(), Some("Verified community"));
        let replay = manager
            .workspace_management(owner)
            .unwrap()
            .rename_community_at_cut(initial_cut, first_request)
            .unwrap();
        assert!(replay.changed && replay.replayed);
        assert_eq!(&replay.community, named);
    }
    assert_eq!(std::fs::read(&path).unwrap(), after);
    let copy = fixture.0.join("reclaimed.rel");
    let plan = reliquary_memory::VerifiedCopyPlan::prepare(&path).unwrap();
    reliquary_memory::reclaim_storage_copy(&plan, &path, &copy).unwrap();
    let source_proof = Rel::open(&path)
        .unwrap()
        .current_historical_proof()
        .unwrap();
    let copy_proof = Rel::open(&copy)
        .unwrap()
        .current_historical_proof()
        .unwrap();
    assert_eq!(source_proof, copy_proof);
    let gateway = Gateway::new();
    let (owner, adapter) = admit(&gateway, &copy);
    let viewer = adapter.authenticate(VIEWER).unwrap();
    let page = viewer
        .inspect_owner(
            owner,
            OwnerKind::Rel,
            &InspectionQuery::Communities,
            None,
            1,
        )
        .unwrap();
    let [InspectionItem::Community(named)] = page.items.as_slice() else {
        panic!("expected reclaimed Community");
    };
    assert_eq!(named.name.as_deref(), Some("Verified community"));
}

#[test]
fn reopen_rejects_a_rename_relocated_beyond_its_physical_graph_cut() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let (id, members) = seed(&path);
    let owner = {
        let gateway = Gateway::new();
        let (owner, adapter) = admit(&gateway, &path);
        let manager = adapter.authenticate(MANAGER).unwrap();
        let cut = manager
            .inspect_owner(
                owner,
                OwnerKind::Rel,
                &InspectionQuery::Communities,
                None,
                1,
            )
            .unwrap()
            .cut;
        manager
            .workspace_management(owner)
            .unwrap()
            .rename_community_at_cut(
                cut,
                CommunityRenameRequest {
                    request_id: "before-graph-change".into(),
                    community_id: id,
                    expected_generation: 1,
                    name: "Only valid at the earlier graph cut".into(),
                },
            )
            .unwrap();
        owner
    };
    let mut original = Container::open_writer_as(&path, FileKind::Reliquary).unwrap();
    let admission = GraphAdmission::for_changes(owner.as_bytes(), members, []);
    let mut graph = GraphStore::open(&original, &admission).unwrap();
    graph
        .put(
            &mut original,
            &admission,
            &[RelationChange {
                source: NodeRef::Memory(members[0]),
                target: NodeRef::Memory(members[1]),
                kind: RelationKind::Topical,
                active: false,
                origin: RelationOrigin::Dream,
            }],
        )
        .unwrap();

    let path = fixture.0.join("forged.rel");
    let mut forged = Container::create(&path, original.identity(), original.owner_id()).unwrap();
    let mut delayed_rename = None;
    for frame in original.read_frames().unwrap() {
        if frame.payload.starts_with(b"RELCMN01") {
            assert!(delayed_rename.replace(frame.payload).is_none());
        } else {
            forged.append(&frame.payload).unwrap();
        }
    }
    forged.append(&delayed_rename.unwrap()).unwrap();
    forged.sync().unwrap();
    drop(forged);
    let result = Rel::open(&path).and_then(|rel| Gateway::new().mount_rel(&rel, false));
    assert_eq!(result.unwrap_err().kind(), ProductErrorKind::Verification);
}
