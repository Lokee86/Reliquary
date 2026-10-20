use super::{StorageUpgradeRequest, plan_storage_upgrade, stage_memory_storage_upgrade};
use crate::memory_codec::decode_tombstone;
use crate::memory_model::MemoryRecord;
use crate::owner_lifecycle::LifecycleState;
use crate::{Container, ContainerError, Cva, FileKind, MemoryDraft, MemoryError, Phylactery};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("memory-upgrade-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn request(&self, file_kind: FileKind) -> StorageUpgradeRequest {
        let source = self.0.join(match file_kind {
            FileKind::Reliquary => "source.rel",
            FileKind::Phylactery => "source.phy",
        });
        let destination = self.0.join("published-output-must-not-exist");
        StorageUpgradeRequest {
            source,
            destination,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl Fixture {
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

fn downgrade_to_v1(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

fn draft(
    mutation_id: &str,
    title: &str,
    source_time_ns: Option<i64>,
    updated_at_ns: i64,
) -> MemoryDraft {
    MemoryDraft {
        category: "decision".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: title.into(),
        content: format!("Stable body for {title}."),
        scope: "project".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns,
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns,
    }
}

fn latest_revision_draft(mutation_id: &str, superseded_by: crate::MemoryId) -> MemoryDraft {
    let mut value = draft(mutation_id, "Server", None, 30);
    value.archived = false;
    value.lifecycle_state = "knowledge".into();
    value.superseded_by = Some(superseded_by);
    value
}

fn set_revision_two_metadata(value: &mut MemoryDraft) {
    value.archived = true;
    value.lifecycle_state = "archived".into();
}

#[derive(Debug, Eq, PartialEq)]
struct PayloadMeasurement {
    record_carrier_bytes: u64,
    body_bytes: u64,
    remaining_payload_bytes: u64,
    total_file_bytes: u64,
}

fn measure_payloads(path: &Path) -> PayloadMeasurement {
    let total_file_bytes = fs::metadata(path).unwrap().len();
    let mut container = Container::open(path).unwrap();
    let mut measurement = PayloadMeasurement {
        record_carrier_bytes: 0,
        body_bytes: 0,
        remaining_payload_bytes: 0,
        total_file_bytes,
    };
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            let len = payload.len() as u64;
            if payload.starts_with(b"CVAMC") || payload.starts_with(b"CVAMD") {
                measurement.record_carrier_bytes += len;
            } else if payload.starts_with(b"CVAMEMR") {
                measurement.record_carrier_bytes += len;
            } else if payload.starts_with(b"CVAMBDY1") {
                measurement.body_bytes += len;
            } else {
                measurement.remaining_payload_bytes += len;
            }
            Ok(())
        })
        .unwrap();
    measurement
}

fn print_payload_measurement(label: &str, path: &Path) {
    let value = measure_payloads(path);
    println!(
        "storage-upgrade {label}: record_carrier_payload_bytes={} body_payload_bytes={} remaining_payload_bytes={} total_file_bytes={}",
        value.record_carrier_bytes,
        value.body_bytes,
        value.remaining_payload_bytes,
        value.total_file_bytes,
    );
}

fn assert_cva_retry_conflicts_are_inert(
    owner: &mut Cva,
    path: &Path,
    active_id: crate::MemoryId,
    terminal_id: crate::MemoryId,
    birth_mutation: &str,
    latest_mutation: &str,
) {
    let before_bytes = fs::read(path).unwrap();
    let before_records = owner.memories.records().to_vec();
    let before_global = owner.latest_global_version();
    let before_local = owner.memory_version();
    let original_birth = owner.memory_revision(active_id, 1).unwrap();
    let original_latest = owner.memory(active_id).unwrap();

    let (birth_retry, changed) = owner
        .publish_memory(
            Some(active_id),
            0,
            draft(birth_mutation, "Server", Some(10), 10),
        )
        .unwrap();
    assert!(!changed);
    assert_eq!(birth_retry, original_birth);
    let (latest_retry, changed) = owner
        .publish_memory(
            Some(active_id),
            1,
            latest_revision_draft(latest_mutation, terminal_id),
        )
        .unwrap();
    assert!(!changed);
    assert_eq!(latest_retry, original_latest);

    let mut conflicting_payload = latest_revision_draft(latest_mutation, terminal_id);
    conflicting_payload.archived = true;
    assert!(matches!(
        owner.publish_memory(Some(active_id), 3, conflicting_payload),
        Err(MemoryError::MutationConflict)
    ));
    assert!(matches!(
        owner.publish_memory(
            Some(active_id),
            2,
            draft("upgrade-memory-a-stale-new-mutation", "Server", None, 31),
        ),
        Err(MemoryError::RevisionConflict)
    ));
    assert!(matches!(
        owner.tombstone_memory(terminal_id, 1).unwrap(),
        crate::owner_lifecycle::TombstoneOutcome::AlreadyTombstoned(_)
    ));
    assert!(matches!(
        owner.publish_memory(
            Some(terminal_id),
            0,
            draft("upgrade-memory-b-birth", "Client", Some(30), 30),
        ),
        Err(MemoryError::TerminalTombstone)
    ));

    assert_eq!(owner.memories.records(), before_records.as_slice());
    assert_eq!(owner.latest_global_version(), before_global);
    assert_eq!(owner.memory_version(), before_local);
    assert_eq!(fs::read(path).unwrap(), before_bytes);
}

fn assert_phy_retry_conflicts_are_inert(
    owner: &mut Phylactery,
    path: &Path,
    active_id: crate::MemoryId,
    terminal_id: crate::MemoryId,
) {
    let before_bytes = fs::read(path).unwrap();
    let before_records = owner.memories.records().to_vec();
    let before_global = owner.latest_global_version();
    let before_local = owner.memory_version();
    let original_birth = owner.memory_revision(active_id, 1).unwrap();
    let original_latest = owner.memory(active_id).unwrap();

    let (birth_retry, changed) = owner
        .publish_memory(
            Some(active_id),
            0,
            draft("upgrade-phy-memory-a-birth", "Server", Some(10), 10),
        )
        .unwrap();
    assert!(!changed);
    assert_eq!(birth_retry, original_birth);
    let (latest_retry, changed) = owner
        .publish_memory(
            Some(active_id),
            1,
            latest_revision_draft("upgrade-phy-memory-a-revision-three", terminal_id),
        )
        .unwrap();
    assert!(!changed);
    assert_eq!(latest_retry, original_latest);

    let mut conflicting_payload =
        latest_revision_draft("upgrade-phy-memory-a-revision-three", terminal_id);
    conflicting_payload.archived = true;
    assert!(matches!(
        owner.publish_memory(Some(active_id), 3, conflicting_payload),
        Err(MemoryError::MutationConflict)
    ));
    assert!(matches!(
        owner.publish_memory(
            Some(active_id),
            2,
            draft(
                "upgrade-phy-memory-a-stale-new-mutation",
                "Server",
                None,
                31
            ),
        ),
        Err(MemoryError::RevisionConflict)
    ));
    assert!(matches!(
        owner.tombstone_memory(terminal_id, 1).unwrap(),
        crate::owner_lifecycle::TombstoneOutcome::AlreadyTombstoned(_)
    ));
    assert!(matches!(
        owner.publish_memory(
            Some(terminal_id),
            0,
            draft("upgrade-phy-memory-b-birth", "Client", Some(30), 30),
        ),
        Err(MemoryError::TerminalTombstone)
    ));

    assert_eq!(owner.memories.records(), before_records.as_slice());
    assert_eq!(owner.latest_global_version(), before_global);
    assert_eq!(owner.memory_version(), before_local);
    assert_eq!(fs::read(path).unwrap(), before_bytes);
}

fn tamper_v1_legacy_revision(path: &Path, from: u64, to: u64) {
    let mut container = Container::open_v1_read_only(path).unwrap();
    let mut revision_offset = None;
    container
        .visit_payloads::<ContainerError>(|object, payload| {
            if payload.starts_with(b"CVAMEMR8")
                && payload.len() >= 48
                && u64::from_le_bytes(payload[40..48].try_into().unwrap()) == from
            {
                revision_offset = Some(object.offset() + 8 + 40);
            }
            Ok(())
        })
        .unwrap();
    drop(container);
    let revision_offset = revision_offset.expect("fixture contains requested legacy revision");
    let mut bytes = fs::read(path).unwrap();
    let start = usize::try_from(revision_offset).unwrap();
    bytes[start..start + 8].copy_from_slice(&to.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

fn assert_compact_memory_frames(path: &Path, records: &[MemoryRecord]) {
    let mut container = Container::open(path).unwrap();
    let mut payloads = Vec::new();
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            payloads.push(payload.to_vec());
            Ok(())
        })
        .unwrap();

    assert_eq!(
        payloads
            .iter()
            .filter(|payload| payload.starts_with(b"CVAMC001"))
            .count(),
        records.iter().filter(|record| record.revision == 1).count()
    );
    assert_eq!(
        payloads
            .iter()
            .filter(|payload| payload.starts_with(b"CVAMD001"))
            .count(),
        records.iter().filter(|record| record.revision > 1).count()
    );
    for version in 2..=8 {
        let magic = format!("CVAMEMR{version}");
        assert!(
            payloads
                .iter()
                .all(|payload| !payload.starts_with(magic.as_bytes())),
            "legacy Memory postimage found: {magic}"
        );
    }
    assert_eq!(
        payloads
            .iter()
            .filter(|payload| decode_tombstone(payload).unwrap().is_some())
            .count(),
        1
    );
}

fn build_rel(path: &Path) -> (crate::MemoryId, crate::MemoryId) {
    let mut rel = Cva::create(path).unwrap();
    let (first, changed) = rel
        .memories
        .publish_legacy_fixture(
            &mut rel.container,
            None,
            0,
            draft("upgrade-memory-a-birth", "Server", Some(10), 10),
        )
        .unwrap();
    assert!(changed);

    // Interleave a second owner publication into the global clock.
    rel.entities
        .publish_legacy_fixture(
            &mut rel.container,
            None,
            0,
            crate::entity_candidate_test_support::entity_draft("Server", &[], "upgrade-entity", 11),
            None,
        )
        .unwrap();

    let mut revision_two_draft = draft("upgrade-memory-a-revision-two", "Server", Some(20), 20);
    set_revision_two_metadata(&mut revision_two_draft);
    let (revision_two, changed) = rel
        .memories
        .publish_legacy_fixture(&mut rel.container, Some(first.id), 1, revision_two_draft)
        .unwrap();
    assert!(changed);
    let (terminal, changed) = rel
        .memories
        .publish_legacy_fixture(
            &mut rel.container,
            None,
            0,
            draft("upgrade-memory-b-birth", "Client", Some(30), 30),
        )
        .unwrap();
    assert!(changed);
    let (revision_three, changed) = rel
        .memories
        .publish_legacy_fixture(
            &mut rel.container,
            Some(first.id),
            2,
            latest_revision_draft("upgrade-memory-a-revision-three", terminal.id),
        )
        .unwrap();
    assert!(changed);
    assert_eq!(revision_two.revision, 2);
    assert_eq!(revision_three.revision, 3);
    assert_eq!(revision_three.source_time_ns, None);
    let tombstone = rel
        .memories
        .tombstone_legacy_fixture(&mut rel.container, terminal.id, terminal.revision)
        .unwrap();
    assert!(matches!(
        tombstone,
        crate::owner_lifecycle::TombstoneOutcome::Published(_)
    ));
    drop(rel);
    (first.id, terminal.id)
}

fn build_phy(path: &Path) -> (crate::MemoryId, crate::MemoryId) {
    let mut phy = Phylactery::create(path).unwrap();
    let (first, changed) = phy
        .memories
        .publish_legacy_fixture(
            &mut phy.container,
            None,
            0,
            draft("upgrade-phy-memory-a-birth", "Server", Some(10), 10),
        )
        .unwrap();
    assert!(changed);

    // An unowned-but-versioned frame creates a global ticket without advancing
    // the Memory owner-local clock.
    phy.container
        .allocate_version_with_transaction_time(Some(2_500))
        .unwrap();
    phy.container
        .append(b"interleaved-phy-owner-frame")
        .unwrap();

    let mut revision_two_draft = draft("upgrade-phy-memory-a-revision-two", "Server", Some(20), 20);
    set_revision_two_metadata(&mut revision_two_draft);
    let (revision_two, changed) = phy
        .memories
        .publish_legacy_fixture(&mut phy.container, Some(first.id), 1, revision_two_draft)
        .unwrap();
    assert!(changed);
    let (terminal, changed) = phy
        .memories
        .publish_legacy_fixture(
            &mut phy.container,
            None,
            0,
            draft("upgrade-phy-memory-b-birth", "Client", Some(30), 30),
        )
        .unwrap();
    assert!(changed);
    let (revision_three, changed) = phy
        .memories
        .publish_legacy_fixture(
            &mut phy.container,
            Some(first.id),
            2,
            latest_revision_draft("upgrade-phy-memory-a-revision-three", terminal.id),
        )
        .unwrap();
    assert!(changed);
    assert_eq!(revision_two.revision, 2);
    assert_eq!(revision_three.revision, 3);
    assert_eq!(revision_three.source_time_ns, None);
    let tombstone = phy
        .memories
        .tombstone_legacy_fixture(&mut phy.container, terminal.id, terminal.revision)
        .unwrap();
    assert!(matches!(
        tombstone,
        crate::owner_lifecycle::TombstoneOutcome::Published(_)
    ));
    drop(phy);
    (first.id, terminal.id)
}

#[test]
fn staged_rel_memory_migration_preserves_interleaved_revisions_and_terminal_history() {
    exercise_rel_migration();
}

#[test]
fn staged_phy_memory_migration_preserves_interleaved_revisions_and_terminal_history() {
    exercise_phy_migration();
}

fn exercise_rel_migration() {
    let fixture = Fixture::new();
    let request = fixture.request(FileKind::Reliquary);
    let (active_id, tombstoned_id) = build_rel(&request.source);
    downgrade_to_v1(&request.source);
    let source_bytes = fs::read(&request.source).unwrap();
    let source_digest = Sha256::digest(&source_bytes);

    let plan = plan_storage_upgrade(request.clone()).unwrap();
    let staged = stage_memory_storage_upgrade(&plan).unwrap();
    let mut source = Cva::open_v1_read_only(&request.source).unwrap();
    let mut migrated = Cva::open(&staged.path).unwrap();
    let accepted = source.memories.records().to_vec();
    assert!(
        accepted
            .windows(2)
            .any(|pair| { pair[1].global_version > pair[0].global_version + 1 })
    );
    assert_eq!(migrated.memories.records(), accepted.as_slice());
    assert_eq!(
        migrated.latest_global_version(),
        source.latest_global_version()
    );
    assert_eq!(migrated.memory_version(), source.memory_version());
    for global in 0..=source.latest_global_version() {
        assert_eq!(
            migrated.transaction_time_ns(global),
            source.transaction_time_ns(global),
            "REL transaction timestamp at global version {global}"
        );
    }
    assert_eq!(
        migrated
            .memory_revision(active_id, 1)
            .unwrap()
            .source_time_ns,
        Some(10)
    );
    assert_eq!(
        migrated
            .memory_revision(active_id, 2)
            .unwrap()
            .source_time_ns,
        Some(20)
    );
    let revision_two = migrated.memory_revision(active_id, 2).unwrap();
    assert!(revision_two.archived);
    assert_eq!(revision_two.lifecycle_state, "archived");
    assert_eq!(revision_two.superseded_by, None);
    let revision_three = migrated.memory_revision(active_id, 3).unwrap();
    assert!(!revision_three.archived);
    assert_eq!(revision_three.lifecycle_state, "knowledge");
    assert_eq!(revision_three.superseded_by, Some(tombstoned_id));
    assert_eq!(
        migrated
            .memory_revision(active_id, 3)
            .unwrap()
            .source_time_ns,
        None
    );
    assert!(matches!(
        migrated.memory_lifecycle_state(tombstoned_id).unwrap(),
        LifecycleState::Tombstoned(_)
    ));
    assert_eq!(
        migrated.memory_retention_descriptor(tombstoned_id).unwrap(),
        source.memory_retention_descriptor(tombstoned_id).unwrap()
    );
    assert_compact_memory_frames(&staged.path, &accepted);
    assert_cva_retry_conflicts_are_inert(
        &mut migrated,
        &staged.path,
        active_id,
        tombstoned_id,
        "upgrade-memory-a-birth",
        "upgrade-memory-a-revision-three",
    );
    print_payload_measurement("REL V1 source", &request.source);
    print_payload_measurement("REL upgraded output", &staged.path);
    assert_eq!(fs::read(&request.source).unwrap(), source_bytes);
    assert_eq!(
        Sha256::digest(fs::read(&request.source).unwrap()),
        source_digest
    );
    drop(migrated);
    drop(source);
    drop(staged);
    assert!(!request.destination.exists());
}

fn exercise_phy_migration() {
    let fixture = Fixture::new();
    let request = fixture.request(FileKind::Phylactery);
    let (active_id, tombstoned_id) = build_phy(&request.source);
    downgrade_to_v1(&request.source);
    let source_bytes = fs::read(&request.source).unwrap();
    let source_digest = Sha256::digest(&source_bytes);

    let plan = plan_storage_upgrade(request.clone()).unwrap();
    let staged = stage_memory_storage_upgrade(&plan).unwrap();
    let mut source = Phylactery::open_v1_read_only(&request.source).unwrap();
    let mut migrated = Phylactery::open(&staged.path).unwrap();
    let accepted = source.memories.records().to_vec();
    assert!(
        accepted
            .windows(2)
            .any(|pair| { pair[1].global_version > pair[0].global_version + 1 })
    );
    assert_eq!(migrated.memories.records(), accepted.as_slice());
    assert_eq!(
        migrated.latest_global_version(),
        source.latest_global_version()
    );
    assert_eq!(migrated.memory_version(), source.memory_version());
    for global in 0..=source.latest_global_version() {
        assert_eq!(
            migrated.transaction_time_ns(global),
            source.transaction_time_ns(global),
            "PHY transaction timestamp at global version {global}"
        );
    }
    assert_eq!(
        migrated
            .memory_revision(active_id, 1)
            .unwrap()
            .source_time_ns,
        Some(10)
    );
    assert_eq!(
        migrated
            .memory_revision(active_id, 2)
            .unwrap()
            .source_time_ns,
        Some(20)
    );
    let revision_two = migrated.memory_revision(active_id, 2).unwrap();
    assert!(revision_two.archived);
    assert_eq!(revision_two.lifecycle_state, "archived");
    assert_eq!(revision_two.superseded_by, None);
    let revision_three = migrated.memory_revision(active_id, 3).unwrap();
    assert!(!revision_three.archived);
    assert_eq!(revision_three.lifecycle_state, "knowledge");
    assert_eq!(revision_three.superseded_by, Some(tombstoned_id));
    assert_eq!(
        migrated
            .memory_revision(active_id, 3)
            .unwrap()
            .source_time_ns,
        None
    );
    assert!(matches!(
        migrated.memory_lifecycle_state(tombstoned_id).unwrap(),
        LifecycleState::Tombstoned(_)
    ));
    assert_eq!(
        migrated.memory_retention_descriptor(tombstoned_id).unwrap(),
        source.memory_retention_descriptor(tombstoned_id).unwrap()
    );
    assert_compact_memory_frames(&staged.path, &accepted);
    assert_phy_retry_conflicts_are_inert(&mut migrated, &staged.path, active_id, tombstoned_id);
    print_payload_measurement("PHY V1 source", &request.source);
    print_payload_measurement("PHY upgraded output", &staged.path);
    assert_eq!(fs::read(&request.source).unwrap(), source_bytes);
    assert_eq!(
        Sha256::digest(fs::read(&request.source).unwrap()),
        source_digest
    );
    drop(migrated);
    drop(source);
    drop(staged);
    assert!(!request.destination.exists());
}

#[test]
fn malformed_legacy_revision_fails_memory_upgrade_without_output_or_source_write() {
    let fixture = Fixture::new();
    let request = fixture.request(FileKind::Reliquary);
    build_rel(&request.source);
    downgrade_to_v1(&request.source);
    tamper_v1_legacy_revision(&request.source, 2, 4);
    let source_before = fs::read(&request.source).unwrap();

    let plan = plan_storage_upgrade(request.clone()).unwrap();
    assert!(stage_memory_storage_upgrade(&plan).is_err());
    assert_eq!(fs::read(&request.source).unwrap(), source_before);
    assert!(!request.destination.exists());
    fixture.assert_no_partial();
}

#[test]
fn compact_creation_and_delta_cannot_be_mislabeled_as_v1_upgrade_input() {
    let fixture = Fixture::new();
    let request = fixture.request(FileKind::Reliquary);
    let mut rel = Cva::create(&request.source).unwrap();
    let (first, changed) = rel
        .publish_memory(
            None,
            0,
            draft("compact-v1-fake-birth", "Server", Some(10), 10),
        )
        .unwrap();
    assert!(changed);
    let (second, changed) = rel
        .publish_memory(
            Some(first.id),
            1,
            draft("compact-v1-fake-revision-two", "Server", Some(20), 20),
        )
        .unwrap();
    assert!(changed);
    assert_eq!(second.revision, 2);
    rel.sync().unwrap();
    drop(rel);

    let mut found_delta = false;
    let mut container = Container::open(&request.source).unwrap();
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            found_delta |= crate::container::publication_envelope::decode(payload)
                .unwrap()
                .is_some_and(|envelope| envelope.body.starts_with(b"CVAMD002"));
            Ok(())
        })
        .unwrap();
    assert!(found_delta, "fixture includes a compact revision delta");
    drop(container);
    downgrade_to_v1(&request.source);
    let source_before = fs::read(&request.source).unwrap();

    assert!(matches!(
        Cva::open_v1_read_only(&request.source),
        Err(crate::CvaError::Container(
            crate::ContainerError::InvalidVersionRecord
        ))
    ));
    assert!(matches!(
        plan_storage_upgrade(request.clone()),
        Err(super::StorageUpgradeError::Container(
            ContainerError::InvalidVersionRecord
        ))
    ));
    assert_eq!(fs::read(&request.source).unwrap(), source_before);
    assert!(!request.destination.exists());
    fixture.assert_no_partial();
}
