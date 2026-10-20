//! Characterize temporary V1 conversion input; removed with the offline converter.
//! These are replay characterization tests, not permanent legacy support.
use crate::entity_model::EntityRecord;
use crate::{Container, Cva, EntityGlobalId, EntityId, Phylactery};
use std::fs;
use std::path::{Path, PathBuf};

fn test_dir() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("luna-entity-v1-admission-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn downgrade_header_to_v1(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

fn append_entity(
    container: &mut Container,
    id: EntityId,
    global_id: Option<EntityGlobalId>,
    revision: u64,
    global_version: u64,
    entity_version: u64,
    updated_at_ns: i64,
    mutation_id: &str,
) {
    let record = EntityRecord {
        id,
        global_id,
        revision,
        canonical_name: "Legacy external referent".into(),
        aliases: vec!["Legacy name".into()],
        kind: "project".into(),
        summary: "Same valid content across mutations".into(),
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns,
        global_version: 0,
        entity_version: 0,
    };
    let payload = match global_id {
        Some(_) => crate::entity_codec::encode_record(&record).unwrap(),
        None => legacy_r1_record(&record),
    };
    let record_ref = container.append(&payload).unwrap();
    container
        .append(&crate::entity_codec::encode_version(
            crate::entity_codec::EntityVersion {
                global_version,
                entity_version,
                record: record_ref,
            },
        ))
        .unwrap();
}

fn legacy_r1_record(record: &EntityRecord) -> Vec<u8> {
    let mut out = b"CVAENTR1".to_vec();
    out.extend_from_slice(&record.id.0);
    out.extend_from_slice(&record.revision.to_le_bytes());
    out.extend_from_slice(&record.created_at_ns.to_le_bytes());
    out.extend_from_slice(&record.updated_at_ns.to_le_bytes());
    write_string(&mut out, &record.canonical_name);
    out.extend_from_slice(&(record.aliases.len() as u16).to_le_bytes());
    for alias in &record.aliases {
        write_string(&mut out, alias);
    }
    write_string(&mut out, &record.kind);
    write_string(&mut out, &record.summary);
    write_string(&mut out, &record.mutation_id);
    out
}

fn write_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn append_identity_history(container: &mut Container) -> EntityId {
    let id = EntityId([0x61; 32]); // nonprincipal, so no derived-identity rule applies
    // Container tickets are allocated monotonically, but V1 owner markers may
    // bind those already-reserved tickets in a different physical order.
    for expected in 1..=5 {
        assert_eq!(container.allocate_version().unwrap(), expected);
    }
    append_entity(container, id, None, 1, 2, 1, 10, "legacy-r1");
    append_entity(
        container,
        id,
        Some(EntityGlobalId([0xA1; 16])),
        2,
        1,
        2,
        15,
        "promote-to-global-id",
    );
    // All semantic content is identical to revision 2, but the fresh mutation
    // and changed optional identity are admitted as a distinct revision.
    append_entity(
        container,
        id,
        Some(EntityGlobalId([0xB2; 16])),
        3,
        3,
        3,
        11,
        "replace-global-id-same-content",
    );
    // updated_at may move backwards as long as it remains >= created_at.
    append_entity(container, id, None, 4, 4, 4, 10, "clear-global-id");
    // A fresh mutation may publish the same resolved fields and timestamp:
    // this is a legal empty mutable-field delta, not a retry of mutation 4.
    append_entity(
        container,
        id,
        None,
        5,
        5,
        5,
        10,
        "same-content-new-mutation",
    );
    id
}

#[test]
fn v1_rel_reopen_preserves_legacy_entity_identity_transitions_and_mutations() {
    let path = test_dir().join("entity-history.rel");
    let mut rel = Cva::create_project(&path).unwrap();
    let id = append_identity_history(&mut rel.container);
    rel.sync().unwrap();
    drop(rel);
    downgrade_header_to_v1(&path);

    let rel = Cva::open_v1_read_only(&path).unwrap();
    assert_eq!(rel.entity_stats().revisions, 5);
    for (revision, global_version) in [(1, 2), (2, 1), (3, 3), (4, 4), (5, 5)] {
        let value = rel.entity_revision(id, revision).unwrap();
        assert_eq!(value.revision, revision);
        assert_eq!(value.entity_version, revision);
        assert_eq!(value.global_version, global_version);
    }
    assert_eq!(rel.entity_revision(id, 1).unwrap().global_id, None);
    assert_eq!(
        rel.entity_revision(id, 2).unwrap().global_id,
        Some(EntityGlobalId([0xA1; 16]))
    );
    assert_eq!(
        rel.entity_revision(id, 3).unwrap().global_id,
        Some(EntityGlobalId([0xB2; 16]))
    );
    assert_eq!(rel.entity_revision(id, 4).unwrap().global_id, None);
    assert_eq!(rel.entity_revision(id, 3).unwrap().updated_at_ns, 11);
    assert_eq!(rel.entity_revision(id, 4).unwrap().updated_at_ns, 10);
    assert_eq!(
        rel.entity_revision(id, 2).unwrap().canonical_name,
        rel.entity_revision(id, 3).unwrap().canonical_name
    );
    assert_eq!(
        rel.entity_revision(id, 2).unwrap().summary,
        rel.entity_revision(id, 3).unwrap().summary
    );
    assert_ne!(
        rel.entity_revision(id, 2).unwrap().mutation_id,
        rel.entity_revision(id, 3).unwrap().mutation_id
    );
    let fourth = rel.entity_revision(id, 4).unwrap();
    let fifth = rel.entity_revision(id, 5).unwrap();
    assert_eq!(fourth.global_id, fifth.global_id);
    assert_eq!(fourth.canonical_name, fifth.canonical_name);
    assert_eq!(fourth.aliases, fifth.aliases);
    assert_eq!(fourth.kind, fifth.kind);
    assert_eq!(fourth.summary, fifth.summary);
    assert_eq!(fourth.created_at_ns, fifth.created_at_ns);
    assert_eq!(fourth.updated_at_ns, fifth.updated_at_ns);
    assert_ne!(fourth.mutation_id, fifth.mutation_id);
}

#[test]
fn v1_phy_reopen_preserves_legacy_entity_identity_transitions_and_mutations() {
    let path = test_dir().join("entity-history.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let id = append_identity_history(&mut phy.container);
    phy.sync().unwrap();
    drop(phy);
    downgrade_header_to_v1(&path);

    let phy = Phylactery::open_v1_read_only(&path).unwrap();
    assert_eq!(phy.entity_stats().revisions, 5);
    for (revision, global_version) in [(1, 2), (2, 1), (3, 3), (4, 4), (5, 5)] {
        let value = phy.entity_revision(id, revision).unwrap();
        assert_eq!(value.revision, revision);
        assert_eq!(value.entity_version, revision);
        assert_eq!(value.global_version, global_version);
    }
    assert_eq!(phy.entity_revision(id, 1).unwrap().global_id, None);
    assert_eq!(
        phy.entity_revision(id, 2).unwrap().global_id,
        Some(EntityGlobalId([0xA1; 16]))
    );
    assert_eq!(
        phy.entity_revision(id, 3).unwrap().global_id,
        Some(EntityGlobalId([0xB2; 16]))
    );
    assert_eq!(phy.entity_revision(id, 4).unwrap().global_id, None);
    assert_eq!(phy.entity_revision(id, 3).unwrap().updated_at_ns, 11);
    assert_eq!(phy.entity_revision(id, 4).unwrap().updated_at_ns, 10);
    assert_eq!(
        phy.entity_revision(id, 2).unwrap().canonical_name,
        phy.entity_revision(id, 3).unwrap().canonical_name
    );
    assert_eq!(
        phy.entity_revision(id, 2).unwrap().summary,
        phy.entity_revision(id, 3).unwrap().summary
    );
    assert_ne!(
        phy.entity_revision(id, 2).unwrap().mutation_id,
        phy.entity_revision(id, 3).unwrap().mutation_id
    );
    let fourth = phy.entity_revision(id, 4).unwrap();
    let fifth = phy.entity_revision(id, 5).unwrap();
    assert_eq!(fourth.global_id, fifth.global_id);
    assert_eq!(fourth.canonical_name, fifth.canonical_name);
    assert_eq!(fourth.aliases, fifth.aliases);
    assert_eq!(fourth.kind, fifth.kind);
    assert_eq!(fourth.summary, fifth.summary);
    assert_eq!(fourth.created_at_ns, fifth.created_at_ns);
    assert_eq!(fourth.updated_at_ns, fifth.updated_at_ns);
    assert_ne!(fourth.mutation_id, fifth.mutation_id);
}

// Ticket allocation remains contiguous; only the V1 Entity owner markers bind
// the reserved tickets in late order 2,1,3,4. This is distinct from decreasing
// or duplicate physical ticket frames and exercises the historical owner path.
