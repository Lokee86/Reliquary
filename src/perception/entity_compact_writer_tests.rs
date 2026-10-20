//! Ordinary Entity publication writes Entity bodies in publication envelopes for REL and PHY.
use crate::compact_delta::FieldChange;
use crate::container::publication_envelope::decode as decode_publication_envelope;
use crate::entity_compact_codec::publication::{self, EntityPublication};
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

fn publications(path: &Path) -> Vec<(u64, EntityPublication)> {
    payloads(path)
        .iter()
        .filter_map(|payload| decode_publication_envelope(payload).unwrap())
        .map(|envelope| {
            (
                envelope.global_version,
                publication::decode(envelope.body, envelope.global_version)
                    .unwrap()
                    .unwrap(),
            )
        })
        .collect()
}

fn assert_compact_stream(path: &Path, expected_creation_id: EntityId, expected_delta_id: EntityId) {
    let values = payloads(path);
    let publications = values
        .iter()
        .filter_map(|payload| decode_publication_envelope(payload).unwrap())
        .map(|envelope| {
            (
                envelope.global_version,
                publication::decode(envelope.body, envelope.global_version)
                    .unwrap()
                    .unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let creations = publications
        .iter()
        .filter_map(|(_, publication)| match publication {
            EntityPublication::Creation(value) => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    let deltas = publications
        .iter()
        .filter_map(|(_, publication)| match publication {
            EntityPublication::Delta(value) => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(creations.len(), 3);
    assert_eq!(deltas.len(), 1);
    assert_eq!(
        publications.len(),
        6,
        "three creations, one delta, two terminals"
    );
    assert!(
        values.iter().all(|payload| {
            !payload.starts_with(b"CVAENC01")
                && !payload.starts_with(b"CVAEND01")
                && !payload.starts_with(b"CVAENTV1")
                && !payload.starts_with(b"CVAENTT1")
                && !payload.starts_with(b"CVAENTT2")
        }),
        "ordinary writer must use envelopes for all Entity publications"
    );
    assert_eq!(
        values
            .iter()
            .filter(|payload| payload.starts_with(b"CVAENTR1") || payload.starts_with(b"CVAENTR2"))
            .count(),
        0,
        "ordinary writer must not persist full Entity records"
    );
    assert_eq!(creations[0].record.id, expected_creation_id);
    assert_eq!(deltas[0].id, expected_delta_id);
    let delta = deltas[0];
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
    let entity_publications = publications(path);
    assert_eq!(
        entity_publications
            .iter()
            .filter(|(_, publication)| matches!(
                publication,
                EntityPublication::MergeTombstone { .. }
            ))
            .count(),
        1
    );
    assert_eq!(
        entity_publications
            .iter()
            .filter(|(_, publication)| matches!(
                publication,
                EntityPublication::DirectTombstone { .. }
            ))
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
    let entity_publications = publications(path);
    assert_eq!(
        entity_publications
            .iter()
            .filter(|(_, publication)| matches!(
                publication,
                EntityPublication::MergeTombstone { .. }
            ))
            .count(),
        1
    );
    assert_eq!(
        entity_publications
            .iter()
            .filter(|(_, publication)| matches!(
                publication,
                EntityPublication::DirectTombstone { .. }
            ))
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

fn assert_validation_rejected_before_append_rel(path: &Path) {
    let mut rel = Cva::create(path).unwrap();
    let before_len = fs::metadata(path).unwrap().len();
    let before_global = rel.latest_global_version();
    let before_local = rel.entity_version();
    assert!(matches!(
        rel.publish_entity(None, 0, draft("  ", "invalid-empty-name", "summary")),
        Err(EntityError::InvalidField("Entity canonical name"))
    ));
    assert_eq!(fs::metadata(path).unwrap().len(), before_len);
    assert_eq!(rel.latest_global_version(), before_global);
    assert_eq!(rel.entity_version(), before_local);
}

fn assert_validation_rejected_before_append_phy(path: &Path) {
    let mut phy = Phylactery::create(path).unwrap();
    let before_len = fs::metadata(path).unwrap().len();
    let before_global = phy.latest_global_version();
    let before_local = phy.entity_version();
    assert!(matches!(
        phy.publish_entity(None, 0, draft("  ", "invalid-empty-name", "summary")),
        Err(EntityError::InvalidField("Entity canonical name"))
    ));
    assert_eq!(fs::metadata(path).unwrap().len(), before_len);
    assert_eq!(phy.latest_global_version(), before_global);
    assert_eq!(phy.entity_version(), before_local);
}

fn assert_truncated_terminal_envelope_recovers_rel(path: &Path) {
    let mut rel = Cva::create(path).unwrap();
    let (created, _) = rel
        .publish_entity(None, 0, draft("Recovery", "recovery-create", "Initial"))
        .unwrap();
    rel.publish_entity(
        Some(created.id),
        1,
        draft("Recovery", "recovery-delta", "Updated"),
    )
    .unwrap();
    rel.tombstone_entity(created.id, 2).unwrap();
    rel.sync().unwrap();
    drop(rel);

    let mut bytes = fs::read(path).unwrap();
    bytes.pop();
    fs::write(path, bytes).unwrap();
    let reopened = Cva::open(path).unwrap();
    assert_eq!(reopened.entity_version(), 2);
    assert_eq!(reopened.latest_global_version(), 2);
    assert_eq!(reopened.entity(created.id).unwrap().summary, "Updated");
}

fn assert_truncated_terminal_envelope_recovers_phy(path: &Path) {
    let mut phy = Phylactery::create(path).unwrap();
    let (created, _) = phy
        .publish_entity(None, 0, draft("Recovery", "recovery-create", "Initial"))
        .unwrap();
    phy.publish_entity(
        Some(created.id),
        1,
        draft("Recovery", "recovery-delta", "Updated"),
    )
    .unwrap();
    phy.tombstone_entity(created.id, 2).unwrap();
    phy.sync().unwrap();
    drop(phy);

    let mut bytes = fs::read(path).unwrap();
    bytes.pop();
    fs::write(path, bytes).unwrap();
    let reopened = Phylactery::open(path).unwrap();
    assert_eq!(reopened.entity_version(), 2);
    assert_eq!(reopened.latest_global_version(), 2);
    assert_eq!(reopened.entity(created.id).unwrap().summary, "Updated");
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

#[test]
fn rel_entity_writer_validates_before_append() {
    let scratch = Scratch::new();
    assert_validation_rejected_before_append_rel(&scratch.file("invalid.rel"));
}

#[test]
fn phy_entity_writer_validates_before_append() {
    let scratch = Scratch::new();
    assert_validation_rejected_before_append_phy(&scratch.file("invalid.phy"));
}

#[test]
fn rel_entity_writer_recovers_incomplete_final_terminal_envelope() {
    let scratch = Scratch::new();
    assert_truncated_terminal_envelope_recovers_rel(&scratch.file("tail.rel"));
}

#[test]
fn phy_entity_writer_recovers_incomplete_final_terminal_envelope() {
    let scratch = Scratch::new();
    assert_truncated_terminal_envelope_recovers_phy(&scratch.file("tail.phy"));
}
