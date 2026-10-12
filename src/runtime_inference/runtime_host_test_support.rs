use crate::InsomniaWorkerConfig;
use std::{fs, path::PathBuf};

pub(super) fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("reliquary-runtime-host-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

pub(super) fn one_worker() -> InsomniaWorkerConfig {
    InsomniaWorkerConfig {
        workers: 1,
        poll_interval_ns: 1_000_000,
        ..Default::default()
    }
}
