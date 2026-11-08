use reliquary_memory::{
    CreateMemoryReceipt, CreateMemoryRequest, Gateway, HostAdmissionAdapter, HostAdmissionControl,
    HostCredentialBinding, InspectionCut, InspectionQuery, OwnerGrant, OwnerId, OwnerScope,
    ProductErrorKind, Rel, TrustedHostAdmission, WorkspaceManagement,
};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const MANAGER: &str = "fixture-checked-memory-manager-admission-token";
const VIEWER: &str = "fixture-checked-memory-viewer-admission-token";
static NEXT_TEST: AtomicUsize = AtomicUsize::new(0);

struct OwnerFile(PathBuf);

impl OwnerFile {
    fn create() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        std::fs::create_dir_all(&root).unwrap();
        let epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let path = root.join(format!(
            "checked-memory-{}-{}-{}.rel",
            std::process::id(),
            epoch.as_nanos(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        Rel::create(&path, OwnerScope::Project).unwrap();
        Self(path)
    }

    fn mount(&self) -> (HostAdmissionAdapter, HostAdmissionControl, OwnerId) {
        let gateway = Gateway::new();
        let rel = Rel::open(&self.0).unwrap();
        let owner = gateway.mount_rel(&rel, true).unwrap();
        let (admission, control) = TrustedHostAdmission::new([
            HostCredentialBinding::new("operator", MANAGER, [OwnerGrant::ManageRel(owner)]),
            HostCredentialBinding::new("viewer", VIEWER, [OwnerGrant::ViewRel(owner)]),
        ])
        .install(&gateway)
        .unwrap();
        (admission, control, owner)
    }
}

impl Drop for OwnerFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn request(id: &str) -> CreateMemoryRequest {
    CreateMemoryRequest {
        request_id: id.into(),
        title: "Manual note".into(),
        content: "Operator-provided content".into(),
        category: "fact".into(),
        memory_type: "note".into(),
        created_at_ns: 42,
    }
}

fn cut(workspace: &WorkspaceManagement) -> InspectionCut {
    workspace
        .inspect(&InspectionQuery::Memories, None, 1)
        .unwrap()
        .cut
}

#[test]
fn accepted_create_replays_after_reopen_and_new_stale_request_fails() {
    let fixture = OwnerFile::create();
    let (initial, accepted) = {
        let (admission, control, owner) = fixture.mount();
        let manager = admission
            .authenticate(MANAGER)
            .unwrap()
            .workspace_management(owner)
            .unwrap();
        let viewer = admission
            .authenticate(VIEWER)
            .unwrap()
            .workspace_management(owner)
            .unwrap();
        let initial = cut(&viewer);
        assert_eq!(
            viewer
                .create_memory_at_cut(initial, request("denied"))
                .unwrap_err()
                .kind(),
            ProductErrorKind::Unauthorized
        );
        let accepted: CreateMemoryReceipt = manager
            .create_memory_at_cut(initial, request("new-note"))
            .unwrap();
        assert!(accepted.created);
        assert_eq!(accepted.memory.revision, 1);
        assert_eq!(accepted.memory.content, "Operator-provided content");
        assert_eq!(accepted.cut, cut(&manager));
        assert!(accepted.cut.memory > initial.memory);
        let mut wrong_generation = initial;
        wrong_generation.generation[0] ^= 1;
        assert_eq!(
            manager
                .create_memory_at_cut(wrong_generation, request("new-note"))
                .unwrap_err()
                .kind(),
            ProductErrorKind::Stale
        );
        let mut changed = request("new-note");
        changed.content = "Changed payload".into();
        assert_eq!(
            manager
                .create_memory_at_cut(initial, changed)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Conflict
        );
        assert_eq!(
            manager
                .create_memory_at_cut(initial, request("another-note"))
                .unwrap_err()
                .kind(),
            ProductErrorKind::Stale
        );
        control.revoke(MANAGER).unwrap();
        assert_eq!(
            manager
                .create_memory_at_cut(initial, request("new-note"))
                .unwrap_err()
                .kind(),
            ProductErrorKind::Revoked
        );
        (initial, accepted)
    };

    let accepted_bytes = std::fs::read(&fixture.0).unwrap();
    {
        let (admission, _, owner) = fixture.mount();
        let manager = admission
            .authenticate(MANAGER)
            .unwrap()
            .workspace_management(owner)
            .unwrap();
        let replay = manager
            .create_memory_at_cut(initial, request("new-note"))
            .unwrap();
        assert!(!replay.created);
        assert_eq!(replay.memory, accepted.memory);
        assert_eq!(replay.cut, accepted.cut);
    }
    assert_eq!(std::fs::read(&fixture.0).unwrap(), accepted_bytes);
}

#[test]
fn competing_fresh_requests_at_one_cut_commit_only_one_memory() {
    let fixture = OwnerFile::create();
    let (admission, _, owner) = fixture.mount();
    let instance = admission.authenticate(MANAGER).unwrap();
    let first = instance.workspace_management(owner).unwrap();
    let second = instance.workspace_management(owner).unwrap();
    let initial = cut(&first);
    let (a, b) = std::thread::scope(|scope| {
        let concurrent = scope.spawn(|| first.create_memory_at_cut(initial, request("first")));
        let other = second.create_memory_at_cut(initial, request("second"));
        (concurrent.join().unwrap(), other)
    });
    let (receipt, rejected) = match (a, b) {
        (Ok(receipt), Err(rejected)) | (Err(rejected), Ok(receipt)) => (receipt, rejected),
        result => panic!("unexpected competing create outcomes: {result:?}"),
    };
    assert!(receipt.created);
    assert_eq!(rejected.kind(), ProductErrorKind::Stale);
    assert_eq!(instance.rel_info(owner).unwrap().memory_count, 1);
    assert_eq!(cut(&first), receipt.cut);
}
