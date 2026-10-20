//! Ordinary Entity publication writes compact creation/delta carriers in REL and PHY.
use crate::compact_delta::FieldChange;
use crate::entity_codec::{decode_direct_tombstone, decode_tombstone};
use crate::entity_compact_codec::{decode_creation, decode_delta};
use crate::{Cva, EntityDraft, EntityError, EntityId, Phylactery};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("entity-compact-writer-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft(name: &str, mutation_id: &str, summary: &str) -> EntityDraft {
    EntityDraft {
        canonical_name: name.into(),
        aliases: vec![format!("{name} alias")],
        kind: "project".into(),
        summary: summary.into(),
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn payloads(path: &Path) -> Vec<Vec<u8>> {
    let mut container = crate::Container::open(path).unwrap();
    let mut values = Vec::new();
    container
        .visit_payloads::<crate::ContainerError>(|_, payload| {
            values.push(payload.to_vec());
            Ok(())
        })
        .unwrap();
    values
}

fn assert_compact_stream(path: &Path, expected_creation_id: EntityId, expected_delta_id: EntityId) {
    let values = payloads(path);
    let creations = values
        .iter()
        .filter(|payload| payload.starts_with(b"CVAENC01"))
        .collect::<Vec<_>>();
    let deltas = values
        .iter()
        .filter(|payload| payload.starts_with(b"CVAEND01"))
        .collect::<Vec<_>>();
    assert_eq!(creations.len(), 3);
    assert_eq!(deltas.len(), 1);
    assert_eq!(
        values
            .iter()
            .filter(|payload| payload.starts_with(b"CVAENTR1") || payload.starts_with(b"CVAENTR2"))
            .count(),
        0,
        "ordinary writer must not persist full Entity records"
    );
    assert_eq!(
        decode_creation(creations[0]).unwrap().record.id,
        expected_creation_id
    );
    assert_eq!(decode_delta(deltas[0]).unwrap().id, expected_delta_id);
    let delta = decode_delta(deltas[0]).unwrap();
    assert_eq!(delta.fields.mask(), 1 << 4);
    assert!(matches!(delta.fields.global_id, FieldChange::Unchanged));
    assert!(matches!(
        delta.fields.canonical_name,
        FieldChange::Unchanged
    ));
    assert!(matches!(delta.fields.aliases, FieldChange::Unchanged));
    assert!(matches!(delta.fields.kind, FieldChange::Unchanged));
    assert!(matches!(delta.fields.summary, FieldChange::Set(_)));
    assert!(matches!(delta.fields.updated_at_ns, FieldChange::Unchanged));
}

fn assert_publication_retry_contract_rel(path: &Path) {
    let mut rel = Cva::create(path).unwrap();
    let (created, changed) = rel
        .publish_entity(
            None,
            0,
            draft("Primary", "create-primary", "Initial summary"),
        )
        .unwrap();
    assert!(changed);
    let (survivor, changed) = rel
        .publish_entity(None, 0, draft("Primary", "create-survivor", "Survivor"))
        .unwrap();
    assert!(changed);
    let (direct, changed) = rel
        .publish_entity(None, 0, draft("Direct", "create-direct", "Direct"))
        .unwrap();
    assert!(changed);

    let revised_draft = draft("Primary", "revise-primary", "Updated summary");
    let (revised, changed) = rel
        .publish_entity(Some(created.id), 1, revised_draft.clone())
        .unwrap();
    assert!(changed);
    assert_eq!(revised.revision, 2);
    let before_retry = fs::metadata(path).unwrap().len();
    let before_global = rel.latest_global_version();
    let before_local = rel.entity_version();
    let (retried, changed) = rel
        .publish_entity(Some(created.id), 1, revised_draft.clone())
        .unwrap();
    assert!(!changed);
    assert_eq!(retried, revised);
    assert_eq!(fs::metadata(path).unwrap().len(), before_retry);
    assert_eq!(rel.latest_global_version(), before_global);
    assert_eq!(rel.entity_version(), before_local);
    let (retried_old, changed) = rel
        .publish_entity(
            Some(created.id),
            0,
            draft("Primary", "create-primary", "Initial summary"),
        )
        .unwrap();
    assert!(!changed);
    assert_eq!(retried_old, created);
    assert_eq!(fs::metadata(path).unwrap().len(), before_retry);
    assert_eq!(rel.latest_global_version(), before_global);
    assert_eq!(rel.entity_version(), before_local);
    assert!(matches!(
        rel.publish_entity(
            Some(created.id),
            0,
            draft("Primary", "create-primary", "Different content")
        ),
        Err(EntityError::MutationConflict)
    ));
    assert_eq!(fs::metadata(path).unwrap().len(), before_retry);
    assert_eq!(rel.latest_global_version(), before_global);
    assert_eq!(rel.entity_version(), before_local);
    assert!(matches!(
        rel.publish_entity(
            Some(created.id),
            1,
            draft("Primary", "revise-primary", "Different content")
        ),
        Err(EntityError::MutationConflict)
    ));
    assert_eq!(fs::metadata(path).unwrap().len(), before_retry);
    assert_eq!(rel.latest_global_version(), before_global);
    assert_eq!(rel.entity_version(), before_local);

    let merged = rel.merge_entities(survivor.id, created.id, 30).unwrap();
    assert!(merged.changed);
    rel.tombstone_entity(direct.id, direct.revision).unwrap();
    rel.sync().unwrap();

    assert_compact_stream(path, created.id, created.id);
    let values = payloads(path);
    assert_eq!(
        values
            .iter()
            .filter(|payload| payload.starts_with(b"CVAENTT1"))
            .count(),
        1
    );
    assert_eq!(
        values
            .iter()
            .filter(|payload| payload.starts_with(b"CVAENTT2"))
            .count(),
        1
    );
    assert_eq!(
        values
            .iter()
            .filter(|payload| decode_tombstone(payload).unwrap().is_some())
            .count(),
        1
    );
    assert_eq!(
        values
            .iter()
            .filter(|payload| decode_direct_tombstone(payload).unwrap().is_some())
            .count(),
        1
    );

    let expected_records = rel.entities.records().to_vec();
    let expected_local = rel.entity_version();
    let expected_global = rel.latest_global_version();
    let times: Vec<_> = (1..=expected_global)
        .map(|global| rel.transaction_time_ns(global))
        .collect();
    drop(rel);
    let mut reopened = Cva::open(path).unwrap();
    assert_eq!(reopened.entities.records(), expected_records.as_slice());
    assert_eq!(reopened.entity_version(), expected_local);
    assert_eq!(reopened.latest_global_version(), expected_global);
    for (index, expected) in times.into_iter().enumerate() {
        assert_eq!(reopened.transaction_time_ns(index as u64 + 1), expected);
    }
    assert_eq!(
        reopened.retired_entity_replacement(created.id),
        Some(survivor.id)
    );
    assert_eq!(reopened.retired_entity_replacement(direct.id), None);
}

fn assert_publication_retry_contract_phy(path: &Path) {
    let mut phy = Phylactery::create(path).unwrap();
    let (created, changed) = phy
        .publish_entity(
            None,
            0,
            draft("Primary", "create-primary", "Initial summary"),
        )
        .unwrap();
    assert!(changed);
    let (survivor, changed) = phy
        .publish_entity(None, 0, draft("Primary", "create-survivor", "Survivor"))
        .unwrap();
    assert!(changed);
    let (direct, changed) = phy
        .publish_entity(None, 0, draft("Direct", "create-direct", "Direct"))
        .unwrap();
    assert!(changed);

    let revised_draft = draft("Primary", "revise-primary", "Updated summary");
    let (revised, changed) = phy
        .publish_entity(Some(created.id), 1, revised_draft.clone())
        .unwrap();
    assert!(changed);
    let before_retry = fs::metadata(path).unwrap().len();
    let before_global = phy.latest_global_version();
    let before_local = phy.entity_version();
    let (retried, changed) = phy
        .publish_entity(Some(created.id), 1, revised_draft.clone())
        .unwrap();
    assert!(!changed);
    assert_eq!(retried, revised);
    assert_eq!(fs::metadata(path).unwrap().len(), before_retry);
    assert_eq!(phy.latest_global_version(), before_global);
    assert_eq!(phy.entity_version(), before_local);
    let (retried_old, changed) = phy
        .publish_entity(
            Some(created.id),
            0,
            draft("Primary", "create-primary", "Initial summary"),
        )
        .unwrap();
    assert!(!changed);
    assert_eq!(retried_old, created);
    assert_eq!(fs::metadata(path).unwrap().len(), before_retry);
    assert_eq!(phy.latest_global_version(), before_global);
    assert_eq!(phy.entity_version(), before_local);
    assert!(matches!(
        phy.publish_entity(
            Some(created.id),
            0,
            draft("Primary", "create-primary", "Different content")
        ),
        Err(EntityError::MutationConflict)
    ));
    assert_eq!(fs::metadata(path).unwrap().len(), before_retry);
    assert_eq!(phy.latest_global_version(), before_global);
    assert_eq!(phy.entity_version(), before_local);
    assert!(matches!(
        phy.publish_entity(
            Some(created.id),
            1,
            draft("Primary", "revise-primary", "Different content")
        ),
        Err(EntityError::MutationConflict)
    ));
    assert_eq!(fs::metadata(path).unwrap().len(), before_retry);
    assert_eq!(phy.latest_global_version(), before_global);
    assert_eq!(phy.entity_version(), before_local);

    let merged = phy.merge_entities(survivor.id, created.id, 30).unwrap();
    assert!(merged.changed);
    phy.tombstone_entity(direct.id, direct.revision).unwrap();
    phy.sync().unwrap();

    assert_compact_stream(path, created.id, created.id);
    let values = payloads(path);
    assert_eq!(
        values
            .iter()
            .filter(|payload| payload.starts_with(b"CVAENTT1"))
            .count(),
        1
    );
    assert_eq!(
        values
            .iter()
            .filter(|payload| payload.starts_with(b"CVAENTT2"))
            .count(),
        1
    );

    let expected_records = phy.entities.records().to_vec();
    let expected_local = phy.entity_version();
    let expected_global = phy.latest_global_version();
    let times: Vec<_> = (1..=expected_global)
        .map(|global| phy.transaction_time_ns(global))
        .collect();
    drop(phy);
    let mut reopened = Phylactery::open(path).unwrap();
    assert_eq!(reopened.entities.records(), expected_records.as_slice());
    assert_eq!(reopened.entity_version(), expected_local);
    assert_eq!(reopened.latest_global_version(), expected_global);
    for (index, expected) in times.into_iter().enumerate() {
        assert_eq!(reopened.transaction_time_ns(index as u64 + 1), expected);
    }
    assert_eq!(
        reopened.retired_entity_replacement(created.id),
        Some(survivor.id)
    );
    assert_eq!(reopened.retired_entity_replacement(direct.id), None);
}

#[test]
fn rel_entity_writer_uses_compact_records_and_preserves_retry_and_terminal_semantics() {
    let scratch = Scratch::new();
    assert_publication_retry_contract_rel(&scratch.file("writer.rel"));
}

#[test]
fn phy_entity_writer_uses_compact_records_and_preserves_retry_and_terminal_semantics() {
    let scratch = Scratch::new();
    assert_publication_retry_contract_phy(&scratch.file("writer.phy"));
}
