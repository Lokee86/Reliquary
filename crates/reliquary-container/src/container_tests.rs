use crate::{Container, ContainerError};
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    fs::create_dir_all(&root).unwrap();
    root.join(format!("{name}-{}.cva", uuid::Uuid::new_v4()))
}

#[test]
fn create_then_reopen_legacy_container() {
    let path = test_path("archive.cva");
    let created = Container::create(&path).unwrap();
    let version = created.version();
    assert_eq!((version.major, version.minor), (2, 0));
    assert_eq!(created.identity(), None);
    drop(created);

    let mut reopened = Container::open(&path).unwrap();
    assert_eq!(reopened.chunk_refs().unwrap().len(), 0);
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
    assert_eq!(reopened.chunk_refs().unwrap(), vec![first, second]);
    assert_eq!(reopened.read(first).unwrap(), b"first");
    assert_eq!(reopened.read(second).unwrap(), b"second record");
}

#[test]
fn writer_and_snapshot_leases_exclude_incompatible_opens() {
    let path = test_path("lease-conflict.cva");
    let writer = Container::create(&path).unwrap();
    assert!(matches!(
        Container::open(&path),
        Err(ContainerError::WriterConflict)
    ));
    drop(writer);

    let snapshot = Container::open_read_only_for_scan(&path).unwrap();
    assert!(matches!(
        Container::open(&path),
        Err(ContainerError::WriterConflict)
    ));
    drop(snapshot);
    assert!(Container::open(&path).is_ok());
}

#[test]
fn hardlink_aliases_share_writer_authority() {
    let path = test_path("lease-hardlink.cva");
    let alias = path.with_file_name(format!("lease-hardlink-alias-{}.cva", uuid::Uuid::new_v4()));
    let writer = Container::create(&path).unwrap();
    if let Err(error) = fs::hard_link(&path, &alias) {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            eprintln!("skipping hardlink alias assertion: OS denied hardlink creation");
            drop(writer);
            return;
        }
        panic!("create hardlink alias: {error}");
    }
    assert!(matches!(
        Container::open(&alias),
        Err(ContainerError::WriterConflict)
    ));
    drop(writer);
    assert!(Container::open(&alias).is_ok());
}

#[test]
fn incomplete_tail_requires_explicit_repair_plan() {
    let path = test_path("explicit-repair.cva");
    let mut writer = Container::create(&path).unwrap();
    writer
        .allocate_version_with_transaction_time(Some(77))
        .unwrap();
    writer.append(b"accepted").unwrap();
    writer.sync().unwrap();
    drop(writer);
    let complete_len = fs::metadata(&path).unwrap().len();
    use std::io::Write as _;
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&[8, 0, 0])
        .unwrap();

    assert!(matches!(
        Container::open(&path),
        Err(ContainerError::RepairRequired { valid_end }) if valid_end == complete_len
    ));
    let mut repair = Container::open_for_repair(&path).unwrap();
    let plan = repair.plan_tail_repair().unwrap().unwrap();
    assert_eq!(plan.valid_end(), complete_len);
    assert_eq!(plan.observed_len(), fs::metadata(&path).unwrap().len());
    let frames = {
        let mut cursor = repair.physical_cursor_through(plan.valid_end()).unwrap();
        let mut frames = Vec::new();
        while let Some(frame) = cursor.next_frame().unwrap() {
            frames.push((frame.payload, frame.latest_global));
        }
        frames
    };
    assert_eq!(frames.last().unwrap(), &(b"accepted".to_vec(), 1));
    repair.repair_incomplete_tail(plan).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().len(), complete_len);
    assert!(repair.plan_tail_repair().unwrap().is_none());
    drop(repair);
    let reopened = Container::open(&path).unwrap();
    assert_eq!(reopened.latest_version(), 1);
    assert_eq!(reopened.transaction_time_ns(1), Some(77));
}

#[test]
fn held_read_source_blocks_mutation_without_changing_bytes() {
    let path = test_path("read-source-gate.cva");
    let mut writer = Container::create(&path).unwrap();
    writer.append(b"before").unwrap();
    let source = writer.read_source().unwrap();
    let before_len = writer.physical_length().unwrap();
    let cursor = source.physical_cursor().unwrap();
    assert!(matches!(
        writer.append(b"blocked"),
        Err(ContainerError::UnstableSnapshot)
    ));
    assert_eq!(writer.physical_length().unwrap(), before_len);
    drop(cursor);
    writer.append(b"after").unwrap();
    assert_eq!(writer.physical_length().unwrap(), before_len + 8 + 5);
}

#[test]
fn read_source_keeps_writer_lease_alive_after_writer_drop() {
    let path = test_path("read-source-lease-lifetime.cva");
    let writer = Container::create(&path).unwrap();
    let source = writer.read_source().unwrap();
    drop(writer);
    assert!(matches!(
        Container::open(&path),
        Err(ContainerError::WriterConflict)
    ));
    drop(source);
    assert!(Container::open(&path).is_ok());
}

#[test]
fn physical_cursor_streams_in_order_without_chunk_index() {
    let path = test_path("streaming-chunks.cva");
    let mut container = Container::create(&path).unwrap();
    let first = container.append(b"one").unwrap();
    let second = container.append(b"two").unwrap();

    let mut visited = Vec::new();
    let mut cursor = container.physical_cursor().unwrap();
    while let Some(frame) = cursor.next_frame().unwrap() {
        visited.push((frame.object, frame.payload));
    }
    assert_eq!(
        visited,
        vec![(first, b"one".to_vec()), (second, b"two".to_vec())]
    );
}

#[test]
fn bounded_physical_cursor_preserves_live_clock_state() {
    let path = test_path("bounded-streaming-clocks.cva");
    let mut container = Container::create(&path).unwrap();
    let first = container.allocate_version().unwrap();
    let second = container.allocate_version().unwrap();
    let before_latest = container.latest_version();
    let before_time = container.transaction_time_ns(1);
    let end = container.chunk_refs().unwrap()[1].legacy_offset();

    let mut cursor = container.physical_cursor_through(end).unwrap();
    let frame = cursor.next_frame().unwrap().unwrap();
    assert_eq!(frame.latest_global, 1);
    assert!(cursor.next_frame().unwrap().is_none());

    assert_eq!(container.latest_version(), before_latest);
    assert_eq!(container.transaction_time_ns(1), before_time);
    assert_eq!((first, second), (1, 2));
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
fn tail_chunk_truncation_uses_reference_position_after_unrelated_io() {
    let path = test_path("truncate-position.cva");
    let mut container = Container::create(&path).unwrap();
    let first = container.append(b"first").unwrap();
    let second = container.append(b"second").unwrap();
    container.write_chunk_prefix(first, b"new").unwrap();
    assert!(!container.truncate_tail_chunk(first).unwrap());
    assert!(container.truncate_tail_chunk(second).unwrap());
    assert_eq!(container.read(first).unwrap(), b"newst");
    assert_eq!(container.chunk_refs().unwrap(), vec![first]);
}

#[test]
fn historical_cut_lookup_never_skips_a_later_prefix_timestamp() {
    let path = test_path("out-of-order.cva");
    let mut container = Container::create(&path).unwrap();

    container.set_next_transaction_time_override(Some(100));
    assert_eq!(container.allocate_version().unwrap(), 1);
    container.set_next_transaction_time_override(Some(300));
    assert_eq!(container.allocate_version().unwrap(), 2);
    container.set_next_transaction_time_override(Some(200));
    assert_eq!(container.allocate_version().unwrap(), 3);
    container.sync().unwrap();
    drop(container);

    let reopened = Container::open(path).unwrap();
    assert_eq!(reopened.transaction_time_ns(1), Some(100));
    assert_eq!(reopened.transaction_time_ns(2), Some(300));
    assert_eq!(reopened.transaction_time_ns(3), Some(200));
    assert_eq!(reopened.version_at_or_before(99), None);
    assert_eq!(reopened.version_at_or_before(100), Some(1));
    assert_eq!(reopened.version_at_or_before(250), Some(1));
    assert_eq!(reopened.version_at_or_before(299), Some(1));
    assert_eq!(reopened.version_at_or_before(300), Some(3));
}

#[test]
fn owner_replay_blocks_writes_and_accepts_only_its_complete_scan() {
    let path = test_path("owner-replay.cva");
    let mut writer = Container::create(&path).unwrap();
    writer
        .allocate_version_with_transaction_time(Some(40))
        .unwrap();
    writer.append(b"owner payload").unwrap();
    writer.sync().unwrap();
    drop(writer);
    let mut replay = Container::open_for_replay(&path).unwrap();
    assert!(matches!(
        replay.append(b"blocked"),
        Err(ContainerError::WriteAdmissionClosed)
    ));
    assert!(matches!(
        replay.close_write_admission(),
        Err(ContainerError::WriteAdmissionClosed)
    ));
    let mut partial = replay.physical_cursor().unwrap();
    partial.next_frame().unwrap().unwrap();
    assert!(matches!(
        partial.finish_open_scan(),
        Err(ContainerError::InvalidIdentity)
    ));
    let state = {
        let mut cursor = replay.physical_cursor().unwrap();
        while cursor.next_frame().unwrap().is_some() {}
        cursor.finish_open_scan().unwrap()
    };
    replay.accept_replay_scan(state).unwrap();
    assert_eq!(replay.latest_version(), 1);
    assert_eq!(replay.transaction_time_ns(1), Some(40));
    replay.append(b"owner admitted").unwrap();
}

#[test]
fn buffered_cursor_crosses_header_and_payload_boundaries_without_reading_later_clocks() {
    let path = test_path("buffer-boundaries.cva");
    let mut container = Container::create(&path).unwrap();
    let first = vec![7; 64 * 1024 - 11];
    let second = vec![8; 64 * 1024 + 20];
    container.append(&first).unwrap();
    container.append(&second).unwrap();
    let accepted_end = container.physical_length().unwrap();
    container.append(b"CVAVERS2").unwrap();
    let mut bounded = container.physical_cursor_through(accepted_end).unwrap();
    assert_eq!(bounded.next_frame().unwrap().unwrap().payload, first);
    assert_eq!(bounded.next_frame().unwrap().unwrap().payload, second);
    assert!(bounded.next_frame().unwrap().is_none());
    drop(bounded);
    let mut full = container.physical_cursor().unwrap();
    full.next_frame().unwrap().unwrap();
    full.next_frame().unwrap().unwrap();
    assert!(matches!(
        full.next_frame(),
        Err(ContainerError::InvalidVersionRecord)
    ));
}
