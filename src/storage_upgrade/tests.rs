use super::*;
use crate::{Container, ContainerIdentity, Cva, FileKind, Phylactery};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("upgrade-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn request(&self) -> StorageUpgradeRequest {
        StorageUpgradeRequest {
            source: self.0.join("source"),
            destination: self.0.join("output"),
        }
    }
    fn assert_no_partial(&self) {
        assert!(fs::read_dir(&self.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".storage-upgrade-")
        }));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn legacy(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

#[test]
fn minimal_rel_and_phy_shells_preserve_owner_identity_and_abort_cleanly() {
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
        let fixture = Fixture::new();
        let request = fixture.request();
        match kind {
            FileKind::Reliquary => drop(Cva::create(&request.source).unwrap()),
            FileKind::Phylactery => drop(Phylactery::create(&request.source).unwrap()),
        }
        legacy(&request.source);
        let before = fs::read(&request.source).unwrap();
        let plan = plan_storage_upgrade(request.clone()).unwrap();
        let staged = stage_storage_upgrade(&plan).unwrap();
        assert_eq!(staged.evidence.identity.file_kind, kind);
        assert_eq!(staged.evidence.source_bytes, before.len() as u64);
        assert_eq!(staged.evidence.owner_uuid, plan.owner_uuid);
        let container = Container::open(&staged.path).unwrap();
        assert_eq!(
            container.version(),
            crate::FormatVersion { major: 2, minor: 0 }
        );
        assert_eq!(container.owner_uuid(), Some(plan.owner_uuid));
        drop(container);
        match kind {
            FileKind::Reliquary => drop(Cva::open(&staged.path).unwrap()),
            FileKind::Phylactery => drop(Phylactery::open(&staged.path).unwrap()),
        }
        assert!(!request.destination.exists());
        drop(staged);
        fixture.assert_no_partial();
        assert_eq!(fs::read(&request.source).unwrap(), before);
    }
}

#[test]
fn physical_shell_retains_global_tickets_timestamps_and_clock_neutral_order() {
    let fixture = Fixture::new();
    let request = fixture.request();
    let mut container = Container::create_with_identity(
        &request.source,
        ContainerIdentity {
            file_kind: FileKind::Reliquary,
            scope: None,
        },
    )
    .unwrap();
    container.append(b"clock-neutral-before").unwrap();
    for timestamp in [Some(100), None, Some(50), Some(100)] {
        container
            .allocate_version_with_transaction_time(timestamp)
            .unwrap();
        container.append(b"clock-neutral-after").unwrap();
    }
    drop(container);
    legacy(&request.source);
    let before = fs::read(&request.source).unwrap();
    let plan = plan_storage_upgrade(request.clone()).unwrap();
    let staged = stage_storage_upgrade(&plan).unwrap();
    assert_eq!(staged.evidence.global_head, 4);
    let copied = fs::read(&staged.path).unwrap();
    assert_eq!(&copied[12..], &before[12..]);
    let mut source = Container::open_v1_read_only(&request.source).unwrap();
    let mut output = Container::open(&staged.path).unwrap();
    assert_eq!(source.chunks().unwrap(), output.chunks().unwrap());
    for version in 0..=4 {
        assert_eq!(
            source.transaction_time_ns(version),
            output.transaction_time_ns(version)
        );
        assert_eq!(
            crate::historical_global_frontier::frontier_end(&source, version).unwrap(),
            crate::historical_global_frontier::frontier_end(&output, version).unwrap()
        );
    }
    drop(output);
    drop(staged);
    fixture.assert_no_partial();
    assert_eq!(fs::read(&request.source).unwrap(), before);
}

#[test]
fn interleaved_owner_cuts_and_historical_fingerprints_survive_shell_copy() {
    let fixture = Fixture::new();
    let request = fixture.request();
    let mut rel = Cva::create(&request.source).unwrap();
    let (memory, changed) = rel
        .memories
        .publish_legacy_fixture(
            &mut rel.container,
            None,
            0,
            crate::entity_candidate_test_support::memory_draft(
                "upgrade-memory-one",
                "Server",
                "The server accepts requests.",
            ),
        )
        .unwrap();
    assert!(changed);
    rel.publish_entity(
        None,
        0,
        crate::entity_candidate_test_support::entity_draft("Server", &[], "upgrade-entity", 1),
    )
    .unwrap();
    let (second_memory, changed) = rel
        .memories
        .publish_legacy_fixture(
            &mut rel.container,
            None,
            0,
            crate::entity_candidate_test_support::memory_draft(
                "upgrade-memory-two",
                "Client",
                "The client sends requests.",
            ),
        )
        .unwrap();
    assert!(changed);
    assert!(rel.memory(memory.id).is_ok());
    assert!(rel.memory(second_memory.id).is_ok());
    drop(rel);
    legacy(&request.source);
    let before = fs::read(&request.source).unwrap();
    let plan = plan_storage_upgrade(request.clone()).unwrap();
    let staged = stage_storage_upgrade(&plan).unwrap();
    let mut source = Cva::open_v1_read_only(&request.source).unwrap();
    let mut output = Cva::open(&staged.path).unwrap();
    assert_eq!(
        source.latest_global_version(),
        output.latest_global_version()
    );
    for global in 0..=source.latest_global_version() {
        let expected =
            crate::historical_whole_owner_fingerprint::whole_owner_at(&mut source, global).unwrap();
        let actual =
            crate::historical_whole_owner_fingerprint::whole_owner_at(&mut output, global).unwrap();
        assert_eq!(expected.cuts, actual.cuts);
        assert_eq!(expected.fingerprint, actual.fingerprint);
    }
    drop(output);
    drop(staged);
    fixture.assert_no_partial();
    assert_eq!(fs::read(request.source).unwrap(), before);
}

#[test]
fn existing_outputs_and_source_aliases_are_never_overwritten() {
    let fixture = Fixture::new();
    let request = fixture.request();
    drop(Cva::create(&request.source).unwrap());
    legacy(&request.source);
    let source = fs::read(&request.source).unwrap();
    for directory in [false, true] {
        if directory {
            fs::create_dir(&request.destination).unwrap();
        } else {
            fs::write(&request.destination, b"keep").unwrap();
        }
        assert!(matches!(
            plan_storage_upgrade(request.clone()),
            Err(StorageUpgradeError::OutputExists(_))
        ));
        if directory {
            fs::remove_dir(&request.destination).unwrap();
        } else {
            assert_eq!(fs::read(&request.destination).unwrap(), b"keep");
            fs::remove_file(&request.destination).unwrap();
        }
    }
    let mut alias = request.clone();
    alias.destination = alias.source.clone();
    assert!(matches!(
        plan_storage_upgrade(alias),
        Err(StorageUpgradeError::OutputExists(_))
    ));
    let mut alias = request;
    alias.destination = fixture.0.join(".").join("source");
    assert!(matches!(
        plan_storage_upgrade(alias),
        Err(StorageUpgradeError::OutputExists(_))
    ));
    assert_eq!(fs::read(fixture.0.join("source")).unwrap(), source);
    fixture.assert_no_partial();
}

#[test]
fn malformed_source_is_rejected_without_repair_or_output() {
    let fixture = Fixture::new();
    let request = fixture.request();
    drop(Cva::create(&request.source).unwrap());
    legacy(&request.source);
    fs::OpenOptions::new()
        .append(true)
        .open(&request.source)
        .unwrap()
        .write_all(&[1, 2, 3])
        .unwrap();
    let before = fs::read(&request.source).unwrap();
    assert!(matches!(
        plan_storage_upgrade(request.clone()),
        Err(StorageUpgradeError::Container(
            crate::ContainerError::TruncatedChunk(_)
        ))
    ));
    assert_eq!(fs::read(request.source).unwrap(), before);
    assert!(!request.destination.exists());
    fixture.assert_no_partial();
}

#[test]
fn compact_and_identityless_inputs_fail_closed() {
    let fixture = Fixture::new();
    let request = fixture.request();
    drop(Cva::create(&request.source).unwrap());
    assert!(matches!(
        plan_storage_upgrade(request.clone()),
        Err(StorageUpgradeError::Container(
            crate::ContainerError::UnsupportedVersion(_)
        ))
    ));
    fs::remove_file(&request.source).unwrap();
    drop(Container::create(&request.source).unwrap());
    legacy(&request.source);
    let before = fs::read(&request.source).unwrap();
    assert!(matches!(
        plan_storage_upgrade(request.clone()),
        Err(StorageUpgradeError::MissingOwnerIdentity)
    ));
    assert_eq!(fs::read(&request.source).unwrap(), before);
    assert!(!request.destination.exists());
    fixture.assert_no_partial();
}

#[test]
fn source_drift_before_or_during_staging_rejects_and_cleans_up() {
    for during in [false, true] {
        let fixture = Fixture::new();
        let request = fixture.request();
        drop(Cva::create(&request.source).unwrap());
        legacy(&request.source);
        let plan = plan_storage_upgrade(request.clone()).unwrap();
        let mutate = || {
            let mut bytes = fs::read(&request.source)?;
            bytes[24] ^= 1; // Same-size UUID change is still source drift.
            fs::write(&request.source, bytes)?;
            Ok(())
        };
        let result = if during {
            super::shell::stage_with_check(&plan, mutate)
        } else {
            mutate().unwrap();
            stage_storage_upgrade(&plan)
        };
        assert!(matches!(result, Err(StorageUpgradeError::SourceChanged)));
        assert!(!request.destination.exists());
        fixture.assert_no_partial();
    }
}

#[test]
fn abort_and_late_output_collision_clean_only_owned_scratch() {
    for collision in [false, true] {
        let fixture = Fixture::new();
        let request = fixture.request();
        drop(Cva::create(&request.source).unwrap());
        legacy(&request.source);
        let before = fs::read(&request.source).unwrap();
        let plan = plan_storage_upgrade(request.clone()).unwrap();
        let result = super::shell::stage_with_check(&plan, || {
            if collision {
                fs::write(&request.destination, b"late winner")?;
                Ok(())
            } else {
                Err(std::io::Error::other("injected abort").into())
            }
        });
        if collision {
            assert!(matches!(result, Err(StorageUpgradeError::OutputExists(_))));
            assert_eq!(fs::read(request.destination).unwrap(), b"late winner");
        } else {
            assert!(matches!(result, Err(StorageUpgradeError::Io(_))));
            assert!(!request.destination.exists());
        }
        fixture.assert_no_partial();
        assert_eq!(fs::read(request.source).unwrap(), before);
    }
}
