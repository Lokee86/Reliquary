use super::*;
use std::{
    fs::{self, OpenOptions},
    path::PathBuf,
    sync::mpsc,
    thread,
    time::Duration,
};

#[test]
fn per_path_config_lock_excludes_a_contender_until_release() {
    let parent = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/oc02-fixtures");
    fs::create_dir_all(&parent).unwrap();
    let directory = tempfile::tempdir_in(parent).unwrap();
    let config = directory.path().join("settings.cfg");
    let lock_path = config.with_file_name(format!(
        ".reliquary-config-{}.lock",
        super::super::master_key::account(&config)
    ));

    let first = super::super::key_lifecycle::lock(&config).unwrap();
    let probe = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lock_path)
        .unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (acquired_tx, acquired_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let contender_path = config.clone();
    let contender = thread::spawn(move || {
        started_tx.send(()).unwrap();
        let lock = super::super::key_lifecycle::lock(&contender_path).unwrap();
        acquired_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        drop(lock);
        finished_tx.send(()).unwrap();
    });

    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(
        probe.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));

    drop(first);
    acquired_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(
        probe.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));

    release_tx.send(()).unwrap();
    finished_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    contender.join().unwrap();
    assert!(probe.try_lock().is_ok());
}
