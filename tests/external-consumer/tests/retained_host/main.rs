use reliquary_memory::{
    Gateway, HostAdmissionControl, HostCredentialBinding, Instance, OwnerGrant, OwnerId,
    OwnerScope, Rel, TrustedHostAdmission,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

mod context;
mod conversation;
mod external_source;
mod project_source;

const OPERATOR: &str = "fixture-credential-operator-00000001";
const VIEWER: &str = "fixture-credential-viewer-0000000002";

struct FixtureDir(PathBuf);

impl FixtureDir {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "retained-host-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn create_rel(&self) -> Rel {
        Rel::create(self.0.join("owner.rel"), OwnerScope::Project).unwrap()
    }
}

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn admission(gateway: &Gateway, owner: OwnerId) -> (Instance, Instance, HostAdmissionControl) {
    let (adapter, control) = admission_config(owner).install(gateway).unwrap();
    (
        adapter.authenticate(OPERATOR).unwrap(),
        adapter.authenticate(VIEWER).unwrap(),
        control,
    )
}

fn admission_config(owner: OwnerId) -> TrustedHostAdmission {
    TrustedHostAdmission::new([
        HostCredentialBinding::new("alice", OPERATOR, [OwnerGrant::OperateRel(owner)]),
        HostCredentialBinding::new("alice", VIEWER, [OwnerGrant::ViewRel(owner)]),
    ])
}
