use reliquary_memory::{
    CorrectMemoryRequest, CreateMemoryRequest, Gateway, GraphNode, HostAdmissionAdapter,
    HostCredentialBinding, InspectionCut, InspectionItem, InspectionQuery, MemoryBodyId, MemoryId,
    MemoryRetirementRequest, OwnerGrant, OwnerId, OwnerScope, ProductErrorKind, Rel, RelationKind,
    RelationOrigin, TrustedHostAdmission, WorkspaceManagement,
};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const MANAGER: &str = "fixture-correction-manager-admission-credential";
const OTHER: &str = "fixture-correction-other-admission-credential";
const VIEWER: &str = "fixture-correction-viewer-admission-credential";
static NEXT_TEST: AtomicUsize = AtomicUsize::new(0);

struct OwnerFile(PathBuf);
impl OwnerFile {
    fn create() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        std::fs::create_dir_all(&root).unwrap();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = root.join(format!(
            "user-memory-{}-{stamp}-{}.rel",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        Rel::create(&path, OwnerScope::Project).unwrap();
        Self(path)
    }

    fn mount(&self) -> (HostAdmissionAdapter, OwnerId) {
        let gateway = Gateway::new();
        let rel = Rel::open(&self.0).unwrap();
        let owner = gateway.mount_rel(&rel, true).unwrap();
        let (admission, _) = TrustedHostAdmission::new([
            HostCredentialBinding::new("operator", MANAGER, [OwnerGrant::ManageRel(owner)]),
            HostCredentialBinding::new("other", OTHER, [OwnerGrant::ManageRel(owner)]),
            HostCredentialBinding::new("viewer", VIEWER, [OwnerGrant::ViewRel(owner)]),
        ])
        .install(&gateway)
        .unwrap();
        (admission, owner)
    }
}

impl Drop for OwnerFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn management(
    admission: &HostAdmissionAdapter,
    token: &str,
    owner: OwnerId,
) -> WorkspaceManagement {
    admission
        .authenticate(token)
        .unwrap()
        .workspace_management(owner)
        .unwrap()
}

fn cut(manager: &WorkspaceManagement) -> InspectionCut {
    manager
        .inspect(&InspectionQuery::Memories, None, 1)
        .unwrap()
        .cut
}

fn id(value: &str) -> [u8; 32] {
    std::array::from_fn(|i| u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).unwrap())
}

fn create(manager: &WorkspaceManagement) -> CorrectMemoryRequest {
    let original = manager
        .create_memory_with_request(CreateMemoryRequest {
            request_id: "seed".into(),
            title: "Original title".into(),
            content: "The original accepted body".into(),
            category: "fact".into(),
            memory_type: "note".into(),
            created_at_ns: 10,
        })
        .unwrap();
    CorrectMemoryRequest {
        request_id: "correct".into(),
        memory_id: MemoryId(id(&original.id)),
        expected_revision: original.revision,
        expected_body_id: MemoryBodyId(id(&original.body_id)),
        title: "Successor title".into(),
        content: "The user corrected body".into(),
        created_at_ns: 11,
    }
}

#[test]
fn correction_and_checked_retirement_replay_exactly_after_reopen() {
    let fixture = OwnerFile::create();
    let (expected, request, correction, retirement_request, retirement) = {
        let (admission, owner) = fixture.mount();
        let manager = management(&admission, MANAGER, owner);
        let viewer = management(&admission, VIEWER, owner);
        let other = management(&admission, OTHER, owner);
        let request = create(&manager);
        let expected = cut(&manager);
        assert_eq!(
            viewer
                .correct_memory_at_cut(expected, request.clone())
                .unwrap_err()
                .kind(),
            ProductErrorKind::Unauthorized
        );
        let mut wrong_body = request.clone();
        wrong_body.expected_body_id.0[0] ^= 1;
        assert_eq!(
            manager
                .correct_memory_at_cut(expected, wrong_body)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Stale
        );
        assert_eq!(cut(&manager), expected);
        let correction = manager
            .correct_memory_at_cut(expected, request.clone())
            .unwrap();
        assert!(correction.created);
        assert_eq!(correction.original_revision, request.expected_revision + 1);
        assert_eq!(correction.cut.memory, expected.memory + 2);
        assert_eq!(correction.cut.graph, expected.graph + 1);
        assert_ne!(correction.replacement.id, correction.original_memory_id);
        assert_ne!(
            id(&correction.replacement.body_id),
            request.expected_body_id.0
        );
        assert_eq!(correction.replacement.content, request.content);
        assert_eq!(correction.replacement.category, "fact");
        let successor = MemoryId(id(&correction.replacement.id));
        assert_eq!(
            manager
                .provenance(successor)
                .unwrap()
                .principal_id
                .as_deref(),
            Some("operator")
        );
        assert_eq!(
            manager.memory(request.memory_id).unwrap().content,
            "The original accepted body"
        );
        assert!(manager.memory(request.memory_id).unwrap().archived);
        let edge = manager
            .inspect(
                &InspectionQuery::Edge {
                    source: GraphNode::Memory(successor.0),
                    target: GraphNode::Memory(request.memory_id.0),
                    kind: RelationKind::Supersedes,
                },
                None,
                1,
            )
            .unwrap();
        let [InspectionItem::Edge(edge)] = edge.items.as_slice() else {
            panic!("missing User edge");
        };
        assert_eq!(edge.origin, RelationOrigin::User);
        assert_eq!(
            other
                .correct_memory_at_cut(expected, request.clone())
                .unwrap_err()
                .kind(),
            ProductErrorKind::Stale
        );
        let mut changed = request.clone();
        changed.content = "Different input under one request ID".into();
        assert_eq!(
            manager
                .correct_memory_at_cut(expected, changed)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Conflict
        );
        let retirement_request = MemoryRetirementRequest {
            owner_id: owner.as_bytes(),
            memory_id: request.memory_id,
            memory_revision: correction.original_revision,
            body_id: request.expected_body_id,
            superseded_by: successor,
            successor_revision: correction.replacement.revision,
            successor_body_id: MemoryBodyId(id(&correction.replacement.body_id)),
        };
        let retirement = manager
            .retire_memory_at_cut(correction.cut, "retire", retirement_request.clone())
            .unwrap();
        assert!(retirement.retired);
        assert_eq!(retirement.revision, correction.original_revision + 1);
        assert_eq!(retirement.cut.memory, correction.cut.memory + 1);
        (
            expected,
            request,
            correction,
            retirement_request,
            retirement,
        )
    };
    let bytes = std::fs::read(&fixture.0).unwrap();
    {
        let (admission, owner) = fixture.mount();
        let manager = management(&admission, MANAGER, owner);
        let replay = manager.correct_memory_at_cut(expected, request).unwrap();
        assert!(!replay.created);
        assert_eq!(replay.original_revision, correction.original_revision);
        assert_eq!(replay.replacement, correction.replacement);
        let replay = manager
            .retire_memory_at_cut(correction.cut, "retire", retirement_request.clone())
            .unwrap();
        assert!(!replay.retired);
        assert_eq!(replay.revision, retirement.revision);
        let mut changed = retirement_request;
        changed.successor_revision += 1;
        assert_eq!(
            manager
                .retire_memory_at_cut(correction.cut, "retire", changed)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Conflict
        );
    }
    assert_eq!(std::fs::read(&fixture.0).unwrap(), bytes);
}

#[test]
fn competing_corrections_publish_only_one_successor() {
    let fixture = OwnerFile::create();
    let (admission, owner) = fixture.mount();
    let one = management(&admission, MANAGER, owner);
    let two = management(&admission, MANAGER, owner);
    let first = create(&one);
    let expected = cut(&one);
    let mut second = first.clone();
    second.request_id = "another-correction".into();
    second.content = "Competing semantic body".into();
    let (a, b) = std::thread::scope(|scope| {
        let left = scope.spawn(|| one.correct_memory_at_cut(expected, first));
        let right = two.correct_memory_at_cut(expected, second);
        (left.join().unwrap(), right)
    });
    let (accepted, denied) = match (a, b) {
        (Ok(accepted), Err(denied)) | (Err(denied), Ok(accepted)) => (accepted, denied),
        result => panic!("unexpected results {result:?}"),
    };
    assert!(accepted.created);
    assert_eq!(denied.kind(), ProductErrorKind::Stale);
    assert_eq!(cut(&one), accepted.cut);
    assert_eq!(
        admission
            .authenticate(MANAGER)
            .unwrap()
            .rel_info(owner)
            .unwrap()
            .memory_count,
        2
    );
}
