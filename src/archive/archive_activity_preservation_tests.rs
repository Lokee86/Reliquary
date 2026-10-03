use crate::{
    Cva, IncomingAttachment, IncomingTurn, ProjectFileRef, ProjectRepositoryKind,
    ProjectRepositoryRef, ProjectRevisionRef, ReliquaryScopeKind,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let dir =
            std::env::temp_dir().join(format!("activity-preservation-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn turn(id: &str, role: &str) -> IncomingTurn {
    IncomingTurn {
        id: id.into(),
        conversation_id: "c".into(),
        parent_id: None,
        role: role.into(),
        principal_id: None,
        timestamp_ns: -10,
        content: "same content".into(),
        attachments: Vec::new(),
        project_attachments: Vec::new(),
    }
}
fn append(cva: &mut Cva, id: &str, role: &str) {
    let t = turn(id, role);
    cva.append_node(
        t.id,
        t.conversation_id,
        t.parent_id,
        t.role,
        t.timestamp_ns,
        &t.content,
    )
    .unwrap();
}
fn cuts(cva: &Cva) -> Vec<u64> {
    (0..=cva.archive_version())
        .map(|v| cva.activity_cut_at_archive_version(v).unwrap())
        .collect()
}
fn orphan(cva: &mut Cva, id: &str) {
    let mut node = cva.archive.nodes.get("c", id).unwrap().clone();
    node.id = "orphan".into();
    node.role = "assistant".into();
    cva.container
        .append(&crate::archive_codec::encode_node(&node).unwrap())
        .unwrap();
}
fn repeat(cva: &mut Cva, version: u64) {
    let record = cva.archive.record_version(version).unwrap().record;
    cva.archive
        .publish_record(&mut cva.container, record)
        .unwrap();
}

#[test]
fn physical_repacks_preserve_exact_activity_history() {
    for mode in 0..3 {
        let fixture = Fixture::new();
        let source = fixture.path("source.rel");
        let output = fixture.path("output.rel");
        let mut cva = Cva::create(&source).unwrap();
        let mut incoming = turn("u", "user");
        let bytes = b"attached evidence";
        incoming.attachments.push(IncomingAttachment {
            filename: "evidence.txt".into(),
            mime_type: None,
            bytes: bytes.to_vec(),
        });
        let attached = cva.ingest_turn(incoming).unwrap().attachments[0].clone();
        cva.store_file("metadata.txt".into(), None, b"metadata")
            .unwrap();
        append(&mut cva, "a", "assistant");
        append(&mut cva, "tool", "tool");
        repeat(&mut cva, 1);
        orphan(&mut cva, "u");
        if mode == 2 {
            cva.bind_legacy_project_file(
                attached.id,
                ProjectFileRef {
                    revision: ProjectRevisionRef {
                        repository: ProjectRepositoryRef {
                            kind: ProjectRepositoryKind::Lore,
                            repository_id: "repo-1".into(),
                            project_path: ".".into(),
                        },
                        revision: "revision-1".into(),
                    },
                    path: "uploads/evidence.txt".into(),
                    content_hash: Some(Sha256::digest(bytes).into()),
                },
            )
            .unwrap();
        }
        cva.sync().unwrap();
        drop(cva);
        let mut cva = Cva::open(&source).unwrap();
        let source_bytes = fs::read(&source).unwrap();
        assert_eq!(cuts(&cva), [0, 1, 1, 2, 2, 2]);
        match mode {
            0 => {
                cva.repack_missing_user_principal(
                    &output,
                    "phy-00000000-0000-0000-0000-000000000123",
                )
                .unwrap();
            }
            1 => {
                let report = cva.reclaim_storage(&output).unwrap();
                assert!(report.unpublished_backing_chunks > 0);
            }
            _ => {
                let report = cva.repack_project_backed_attachments(&output).unwrap();
                assert_eq!(report.dropped_content_objects, 1);
            }
        }
        let reopened = Cva::open(&output).unwrap();
        assert_eq!(reopened.owner_uuid(), cva.owner_uuid());
        assert_eq!(cuts(&reopened), [0, 1, 1, 2, 2, 2]);
        assert_eq!(reopened.rel_turn_count(), 2);
        assert_eq!(reopened.activity_position_for_turn("c", "u"), Some(1));
        assert_eq!(reopened.activity_position_for_turn("c", "a"), Some(2));
        assert_eq!(reopened.activity_position_for_turn("c", "tool"), None);
        assert_eq!(reopened.activity_position_for_turn("c", "orphan"), None);
        assert_eq!(fs::read(&source).unwrap(), source_bytes);
        drop(reopened);
        drop(cva);
    }
}

#[test]
fn copy_only_reconcile_preserves_selected_side_cuts_in_both_directions() {
    let fixture = Fixture::new();
    let left = fixture.path("left.rel");
    let right = fixture.path("right.rel");
    let mut cva = Cva::create(&left).unwrap();
    append(&mut cva, "u", "user");
    cva.store_file("prefix.txt".into(), None, b"prefix")
        .unwrap();
    cva.sync().unwrap();
    fs::copy(&left, &right).unwrap();
    append(&mut cva, "a", "assistant");
    cva.store_file("suffix.txt".into(), None, b"suffix")
        .unwrap();
    cva.sync().unwrap();
    drop(cva);
    let before = fs::read(&left).unwrap();
    for (index, (first, second, relation)) in [
        (&left, &right, crate::CvaRelation::LeftExtendsRight),
        (&right, &left, crate::CvaRelation::RightExtendsLeft),
        (&left, &left, crate::CvaRelation::Identical),
    ]
    .into_iter()
    .enumerate()
    {
        let output = fixture.path(&format!("copy-{index}.rel"));
        let result = Cva::reconcile(first, second, &output).unwrap();
        assert_eq!(result.comparison.relation, relation);
        let reopened = Cva::open(&output).unwrap();
        let selected = Cva::open(&left).unwrap();
        assert_eq!(reopened.owner_uuid(), selected.owner_uuid());
        assert_eq!(cuts(&reopened), [0, 1, 1, 2, 2]);
        assert_eq!(reopened.activity_position_for_turn("c", "a"), Some(2));
        assert_eq!(fs::read(&output).unwrap(), before);
    }
    assert_eq!(fs::read(&left).unwrap(), before);
}

#[test]
fn divergent_reconcile_counts_union_in_destination_acceptance_order() {
    let fixture = Fixture::new();
    let left = fixture.path("left.rel");
    let right = fixture.path("right.rel");
    let output = fixture.path("merged.rel");
    let mut base = Cva::create(&left).unwrap();
    append(&mut base, "base", "user");
    base.sync().unwrap();
    drop(base);
    fs::copy(&left, &right).unwrap();
    let mut l = Cva::open(&left).unwrap();
    append(&mut l, "shared", "user");
    l.store_file("left.txt".into(), None, b"left").unwrap();
    append(&mut l, "left", "assistant");
    l.sync().unwrap();
    drop(l);
    let mut r = Cva::open(&right).unwrap();
    append(&mut r, "right-first", "assistant");
    r.store_file("right.txt".into(), None, b"right").unwrap();
    r.ingest_turn(turn("shared", "user")).unwrap();
    append(&mut r, "system", "system");
    append(&mut r, "right-last", "user");
    r.sync().unwrap();
    assert_eq!(r.activity_position_for_turn("c", "right-first"), Some(2));
    drop(r);
    let left_bytes = fs::read(&left).unwrap();
    let right_bytes = fs::read(&right).unwrap();
    let result = Cva::reconcile(&left, &right, &output).unwrap();
    assert_eq!(result.comparison.relation, crate::CvaRelation::Diverged);
    let merged = Cva::open(&output).unwrap();
    assert_eq!(merged.rel_turn_count(), 5);
    for (id, position) in [
        ("base", 1),
        ("shared", 2),
        ("left", 3),
        ("right-first", 4),
        ("right-last", 5),
    ] {
        assert_eq!(merged.activity_position_for_turn("c", id), Some(position));
    }
    assert_eq!(merged.activity_position_for_turn("c", "system"), None);
    assert_eq!(cuts(&merged), [0, 1, 2, 2, 3, 4, 4, 4, 5]);
    assert_eq!(fs::read(&left).unwrap(), left_bytes);
    assert_eq!(fs::read(&right).unwrap(), right_bytes);
}

#[test]
fn semantic_duplicate_divergence_restores_exact_left_history() {
    let fixture = Fixture::new();
    let left = fixture.path("left.rel");
    let right = fixture.path("right.rel");
    let output = fixture.path("merged.rel");
    Cva::create(&left).unwrap().sync().unwrap();
    fs::copy(&left, &right).unwrap();
    let mut l = Cva::open(&left).unwrap();
    append(&mut l, "u", "user");
    l.store_file("metadata.txt".into(), None, b"metadata")
        .unwrap();
    repeat(&mut l, 1);
    l.sync().unwrap();
    drop(l);
    let mut r = Cva::open(&right).unwrap();
    r.ingest_turn(turn("u", "user")).unwrap();
    r.store_file("metadata.txt".into(), None, b"metadata")
        .unwrap();
    r.sync().unwrap();
    drop(r);
    let before = fs::read(&left).unwrap();
    let result = Cva::reconcile(&left, &right, &output).unwrap();
    assert_eq!(result.comparison.relation, crate::CvaRelation::Diverged);
    assert!(!result.canonical_change_required);
    let merged = Cva::open(&output).unwrap();
    assert_eq!(cuts(&merged), [0, 1, 1, 1]);
    assert_eq!(merged.rel_turn_count(), 1);
    assert_eq!(merged.activity_position_for_turn("c", "u"), Some(1));
    assert_eq!(fs::read(&output).unwrap(), before);
}

#[test]
fn divergent_conflicting_identity_rejects_without_changing_sources() {
    let fixture = Fixture::new();
    let left = fixture.path("left.rel");
    let right = fixture.path("right.rel");
    let output = fixture.path("merged.rel");
    Cva::create(&left).unwrap().sync().unwrap();
    fs::copy(&left, &right).unwrap();
    let mut l = Cva::open(&left).unwrap();
    append(&mut l, "collision", "user");
    l.sync().unwrap();
    drop(l);
    let mut r = Cva::open(&right).unwrap();
    let mut incoming = turn("collision", "user");
    incoming.content = "conflict".into();
    r.ingest_turn(incoming).unwrap();
    r.sync().unwrap();
    drop(r);
    let before_left = fs::read(&left).unwrap();
    let before_right = fs::read(&right).unwrap();
    let error = Cva::reconcile(&left, &right, &output).unwrap_err();
    assert!(error.conflict().is_some());
    assert!(!output.exists());
    assert_eq!(fs::read(&left).unwrap(), before_left);
    assert_eq!(fs::read(&right).unwrap(), before_right);
}

#[test]
fn fresh_migration_preserves_order_when_redundant_versions_are_removed() {
    let fixture = Fixture::new();
    let source = fixture.path("legacy.rel");
    let output = fixture.path("migrated.rel");
    let mut cva = Cva::create_legacy_typed(&source, ReliquaryScopeKind::Project).unwrap();
    cva.store_file("prefix.txt".into(), None, b"prefix")
        .unwrap();
    append(&mut cva, "u", "user");
    // A distinct legacy backing record with the same immutable identity.
    let node = cva.archive.nodes.get("c", "u").unwrap().clone();
    let mut bytes = crate::archive_codec::encode_node(&node).unwrap();
    bytes.truncate(bytes.len() - 4); // absent principal in CVANODE1
    bytes[..8].copy_from_slice(b"CVANODE1");
    let record = cva.container.append(&bytes).unwrap();
    cva.archive
        .publish_record(&mut cva.container, record)
        .unwrap();
    repeat(&mut cva, 3);
    append(&mut cva, "system", "system");
    cva.ingest_turn(turn("a", "assistant")).unwrap();
    orphan(&mut cva, "u");
    cva.sync().unwrap();
    drop(cva);
    let before = fs::read(&source).unwrap();
    crate::migrate_file(&source, &output).unwrap();
    let original = Cva::open(&source).unwrap();
    let migrated = Cva::open(&output).unwrap();
    assert_eq!(original.rel_turn_count(), 2);
    assert_eq!(migrated.rel_turn_count(), 2);
    assert_eq!(original.archive_version(), 6);
    assert_eq!(migrated.archive_version(), 4);
    assert_eq!(original.activity_cut_at_archive_version(4).unwrap(), 1);
    assert_eq!(migrated.activity_cut_at_archive_version(4).unwrap(), 2);
    for rel in [&original, &migrated] {
        assert_eq!(rel.activity_position_for_turn("c", "u"), Some(1));
        assert_eq!(rel.activity_position_for_turn("c", "a"), Some(2));
        assert_eq!(rel.activity_position_for_turn("c", "system"), None);
        assert_eq!(rel.activity_position_for_turn("c", "orphan"), None);
    }
    assert_eq!(fs::read(&source).unwrap(), before);
}

#[test]
fn preservation_checks_distinguish_order_cuts_and_corrupt_positions() {
    let fixture = Fixture::new();
    let mut left = Cva::create(fixture.path("left.rel")).unwrap();
    let mut right = Cva::create(fixture.path("right.rel")).unwrap();
    append(&mut left, "u", "user");
    left.store_file("metadata.txt".into(), None, b"metadata")
        .unwrap();
    append(&mut left, "a", "assistant");
    right
        .store_file("metadata.txt".into(), None, b"metadata")
        .unwrap();
    append(&mut right, "u", "user");
    append(&mut right, "a", "assistant");
    assert!(left.archive.activity_order_matches(&right.archive));
    assert!(!left.archive.activity_history_matches(&right.archive));
    right.archive.activity.first_activity_position_by_node[0] = 7;
    assert!(!left.archive.activity_order_matches(&right.archive));
    assert!(!left.archive.activity_history_matches(&right.archive));
}
