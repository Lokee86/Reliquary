#[path = "ego_anchor_tests.rs"]
mod anchors;
#[path = "ego_identity_tests.rs"]
mod identities;
#[path = "ego_lifecycle_tests.rs"]
mod lifecycle;
#[path = "ego_persona_tests.rs"]
mod persona;

use std::fs;
use std::path::PathBuf;

pub(super) fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("reliquary-ego-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}
