use crate::{Container, ContainerError};
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn create_then_reopen_legacy_container() {
    let path = test_path("archive.cva");
    let created = Container::create(&path).unwrap();
    let version = created.version();
    assert_eq!((version.major, version.minor), (1, 0));
    assert_eq!(created.identity(), None);
    drop(created);

    let mut reopened = Container::open(&path).unwrap();
    assert_eq!(reopened.chunks().unwrap().len(), 0);
    assert_eq!(reopened.identity(), None);
}

#[test]
fn typed_reliquary_identity_roundtrips() {
    let path = test_path("project.prj.rel");
    let identity = crate::ContainerIdentity {
        file_kind: crate::FileKind::Reliquary,
        scope: Some(crate::ReliquaryScopeKind::Project),
    };
    let created = Container::create_with_identity(&path, identity).unwrap();
    assert_eq!(created.identity(), Some(identity));
    drop(created);

    let reopened = Container::open(&path).unwrap();
    assert_eq!(reopened.identity(), Some(identity));
}

#[test]
fn typed_phylactery_identity_roundtrips() {
    let path = test_path("identity.phy");
    let identity = crate::ContainerIdentity {
        file_kind: crate::FileKind::Phylactery,
        scope: None,
    };
    let created = Container::create_with_identity(&path, identity).unwrap();
    assert_eq!(created.identity(), Some(identity));
    drop(created);

    let reopened = Container::open(&path).unwrap();
    assert_eq!(reopened.identity(), Some(identity));
}

#[test]
fn file_kind_and_scope_combinations_are_validated() {
    let rel_without_scope = crate::ContainerIdentity {
        file_kind: crate::FileKind::Reliquary,
        scope: None,
    };
    assert!(Container::create_with_identity(test_path("current.rel"), rel_without_scope).is_ok());

    let phy_with_scope = crate::ContainerIdentity {
        file_kind: crate::FileKind::Phylactery,
        scope: Some(crate::ReliquaryScopeKind::Project),
    };
    assert!(matches!(
        Container::create_with_identity(test_path("bad.phy"), phy_with_scope),
        Err(ContainerError::InvalidIdentity)
    ));
}

#[test]
fn append_then_read_chunks() {
    let path = test_path("chunks.cva");
    let mut container = Container::create(&path).unwrap();
    let first = container.append(b"first").unwrap();
    let second = container.append(b"second record").unwrap();
    container.sync().unwrap();
    drop(container);

    let mut reopened = Container::open(&path).unwrap();
    assert_eq!(reopened.chunks().unwrap(), vec![first, second]);
    assert_eq!(reopened.read(first).unwrap(), b"first");
    assert_eq!(reopened.read(second).unwrap(), b"second record");
}

#[test]
fn visit_payloads_streams_in_order_without_chunk_index() {
    let path = test_path("streaming-chunks.cva");
    let mut container = Container::create(&path).unwrap();
    let first = container.append(b"one").unwrap();
    let second = container.append(b"two").unwrap();

    let mut visited = Vec::new();
    container
        .visit_payloads::<ContainerError>(|object, payload| {
            visited.push((object, payload.to_vec()));
            Ok(())
        })
        .unwrap();
    assert_eq!(
        visited,
        vec![(first, b"one".to_vec()), (second, b"two".to_vec())]
    );
}

#[test]
fn reject_truncated_header() {
    let path = test_path("truncated.cva");
    fs::write(&path, b"CVA").unwrap();
    assert!(matches!(
        Container::open(path),
        Err(ContainerError::TruncatedHeader)
    ));
}

#[test]
fn recover_truncated_trailing_chunk() {
    let path = test_path("truncated-chunk.cva");
    let mut container = Container::create(&path).unwrap();
    container.append(b"complete").unwrap();
    container.sync().unwrap();
    drop(container);
    let mut bytes = fs::read(&path).unwrap();
    bytes.pop();
    fs::write(&path, bytes).unwrap();

    let mut reopened = Container::open(&path).unwrap();
    assert!(reopened.chunks().unwrap().is_empty());
    assert_eq!(fs::metadata(path).unwrap().len(), 16);
}

#[test]
fn read_only_stream_scan_rejects_truncated_tail_without_repair() {
    let path = test_path("readonly-truncated-chunk.cva");
    let mut container = Container::create(&path).unwrap();
    container.append(b"complete").unwrap();
    container.sync().unwrap();
    drop(container);
    let mut bytes = fs::read(&path).unwrap();
    bytes.pop();
    fs::write(&path, &bytes).unwrap();

    let mut scan = Container::open_read_only_for_scan(&path).unwrap();
    assert!(matches!(
        scan.visit_payloads::<ContainerError>(|_, _| Ok(())),
        Err(ContainerError::TruncatedChunk(_))
    ));
    assert_eq!(fs::metadata(path).unwrap().len(), bytes.len() as u64);
}

#[test]
fn reject_non_cva_file() {
    let path = test_path("wrong.cva");
    fs::write(&path, [0_u8; 16]).unwrap();
    assert!(matches!(
        Container::open(path),
        Err(ContainerError::InvalidMagic)
    ));
}

#[test]
fn global_versions_survive_reopen() {
    let path = test_path("versions.cva");
    let mut container = Container::create(&path).unwrap();
    assert_eq!(container.allocate_version().unwrap(), 1);
    assert_eq!(container.allocate_version().unwrap(), 2);
    assert!(container.transaction_time_ns(1).is_some());
    assert!(container.transaction_time_ns(2).is_some());
    container.sync().unwrap();
    drop(container);

    let mut reopened = Container::open(path).unwrap();
    assert_eq!(reopened.latest_version(), 2);
    assert!(reopened.transaction_time_ns(1).is_some());
    assert!(reopened.transaction_time_ns(2).is_some());
    assert_eq!(reopened.allocate_version().unwrap(), 3);
}

#[test]
fn transaction_times_round_trip_and_resolve_version_cuts() {
    let path = test_path("transaction-times.cva");
    let mut container = Container::create(&path).unwrap();
    assert_eq!(
        container
            .allocate_version_with_transaction_time(Some(100))
            .unwrap(),
        1
    );
    assert_eq!(
        container
            .allocate_version_with_transaction_time(Some(250))
            .unwrap(),
        2
    );
    assert_eq!(container.transaction_time_ns(1), Some(100));
    assert_eq!(container.transaction_time_ns(2), Some(250));
    assert_eq!(container.version_at_or_before(99), None);
    assert_eq!(container.version_at_or_before(100), Some(1));
    assert_eq!(container.version_at_or_before(249), Some(1));
    assert_eq!(container.version_at_or_before(250), Some(2));
    container.sync().unwrap();
    drop(container);

    let reopened = Container::open(path).unwrap();
    assert_eq!(reopened.transaction_time_ns(1), Some(100));
    assert_eq!(reopened.transaction_time_ns(2), Some(250));
    assert_eq!(reopened.version_at_or_before(250), Some(2));
}

#[test]
fn legacy_versions_reopen_without_fabricated_transaction_time() {
    let path = test_path("legacy-version-time.cva");
    let mut container = Container::create(&path).unwrap();
    let mut payload = Vec::from(*b"CVAVERS1");
    payload.extend_from_slice(&1_u64.to_le_bytes());
    container.append(&payload).unwrap();
    container.sync().unwrap();
    drop(container);

    let reopened = Container::open(path).unwrap();
    assert_eq!(reopened.latest_version(), 1);
    assert_eq!(reopened.transaction_time_ns(1), None);
    assert_eq!(reopened.version_at_or_before(i64::MAX), None);
}
