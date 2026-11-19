use super::*;
use reliquary_container::ContainerError;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

#[test]
fn observer_holds_the_shared_read_only_lease_until_finalize() {
    let path = std::env::temp_dir().join(format!(
        "d02-held-observer-{}-{}.rel",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    drop(crate::RelWriter::create_current_path(&path, OwnerScope::Project, [0xA1; 16]).unwrap());

    let observer = ReadOnlyCurrentOwnerObserver::open(&path).unwrap();
    assert!(matches!(
        Container::open_writer(&path),
        Err(ContainerError::WriterConflict)
    ));
    let finalization = observer.finalize().unwrap();
    assert!(finalization.candidate_unchanged);

    let writer = Container::open_writer(&path).unwrap();
    drop(writer);
    fs::remove_file(path).unwrap();
}

#[test]
fn finalize_rejects_path_replacement_during_held_handle_hash() {
    let stem = format!(
        "d02-replaced-observer-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let directory = std::env::temp_dir();
    let path = directory.join(format!("{stem}.rel"));
    let replacement = directory.join(format!("{stem}-replacement.rel"));
    let displaced = directory.join(format!("{stem}-displaced.rel"));
    drop(crate::RelWriter::create_current_path(&path, OwnerScope::Project, [0xA2; 16]).unwrap());
    drop(
        crate::RelWriter::create_current_path(&replacement, OwnerScope::Project, [0xA3; 16])
            .unwrap(),
    );

    let observer = ReadOnlyCurrentOwnerObserver::open(&path).unwrap();
    let finalization = observer
        .finalize_after_hash(|| {
            fs::rename(&path, &displaced).unwrap();
            fs::rename(&replacement, &path).unwrap();
        })
        .unwrap();

    assert!(!finalization.path_matches_open_file_after);
    assert!(finalization.physical_file_identity_unchanged);
    assert!(!finalization.candidate_unchanged);
    fs::remove_file(displaced).unwrap();
    fs::remove_file(path).unwrap();
}
