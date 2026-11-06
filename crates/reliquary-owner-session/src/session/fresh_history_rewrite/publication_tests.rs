use super::*;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Destination(PathBuf);

impl Destination {
    fn new(name: &str) -> Self {
        let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join(".owner-session-test-tmp");
        fs::create_dir_all(&parent).unwrap();
        Self(parent.join(format!(
            "rewrite-publication-{name}-{}-{}.phy",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for Destination {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn intervening_destination_is_not_replaced_by_verified_rewrite() {
    let destination = Destination::new("collision");
    let candidate = StagedCopy::create(&destination.0).unwrap();
    fs::write(&candidate.path, b"verified rewritten owner").unwrap();
    // Simulate another writer winning destination publication after staging.
    fs::write(&destination.0, b"existing unrelated owner").unwrap();

    let error = publish_rewrite_candidate(&candidate, &destination.0).unwrap_err();

    assert_eq!(error, "rewrite destination already exists");
    assert_eq!(
        fs::read(&destination.0).unwrap(),
        b"existing unrelated owner"
    );
    assert_eq!(
        fs::read(&candidate.path).unwrap(),
        b"verified rewritten owner"
    );
}

#[test]
fn verified_rewrite_publishes_without_replacing_any_existing_file() {
    let destination = Destination::new("success");
    let candidate = StagedCopy::create(&destination.0).unwrap();
    fs::write(&candidate.path, b"verified rewritten owner").unwrap();

    publish_rewrite_candidate(&candidate, &destination.0).unwrap();

    assert_eq!(
        fs::read(&destination.0).unwrap(),
        b"verified rewritten owner"
    );
    assert_eq!(
        publish_rewrite_candidate(&candidate, &destination.0).unwrap_err(),
        "rewrite destination already exists"
    );
    assert_eq!(
        fs::read(&destination.0).unwrap(),
        b"verified rewritten owner"
    );
}
