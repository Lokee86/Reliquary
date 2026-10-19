use super::*;
use crate::{
    EntityId, EntityRef, MemoryId, MemoryRef, Phylactery, RelationshipDraft,
    RelationshipParticipant,
};
use std::{fs, path::PathBuf};

fn scratch() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("relationship-oracle-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn draft(mutation: &str) -> RelationshipDraft {
    RelationshipDraft {
        kind: "association".into(),
        participants: vec![RelationshipParticipant {
            entity: EntityRef {
                owner_id: "rel-external-entity".into(),
                entity_id: EntityId([3; 32]),
            },
            role: Some("subject".into()),
        }],
        evidence: vec![MemoryRef {
            owner_id: "rel-external-evidence".into(),
            memory_id: MemoryId([7; 32]),
        }],
        summary: "Exact relationship description".into(),
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

#[test]
fn relationship_cuts_preserve_qualified_participants_evidence_and_terminal_history() {
    let dir = scratch();
    let path = dir.join("owner.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let first = phy
        .publish_relationship(None, 0, draft("create"))
        .unwrap()
        .0;
    let before = relationship_cut(&phy.container, &phy.relationships, 1)
        .unwrap()
        .fingerprint;
    let mut changed = draft("revise");
    changed.participants[0].role = None;
    changed.participants.push(RelationshipParticipant {
        entity: EntityRef {
            owner_id: "phy-other-entity".into(),
            entity_id: EntityId([9; 32]),
        },
        role: Some("object".into()),
    });
    changed.evidence.clear();
    changed.summary = "Exact new description".into();
    changed.updated_at_ns = 2;
    let second = phy
        .publish_relationship(Some(first.id), 1, changed)
        .unwrap()
        .0;
    phy.tombstone_relationship(first.id, second.revision)
        .unwrap();
    assert_eq!(phy.relationship_revision(first.id, 1).unwrap(), first);
    assert_eq!(phy.relationship_revision(first.id, 2).unwrap(), second);
    assert_eq!(
        before,
        relationship_cut(&phy.container, &phy.relationships, 1)
            .unwrap()
            .fingerprint
    );
    assert_eq!(
        relationship_cut(&phy.container, &phy.relationships, 1)
            .unwrap()
            .current[&first.id],
        1
    );
    assert_eq!(
        relationship_cut(&phy.container, &phy.relationships, 2)
            .unwrap()
            .current[&first.id],
        2
    );
    assert!(
        relationship_cut(&phy.container, &phy.relationships, 3)
            .unwrap()
            .current
            .is_empty()
    );
    assert!(relationship_cut(&phy.container, &phy.relationships, 4).is_err());
    let expected: Vec<_> = (0..=3)
        .map(|cut| {
            relationship_cut(&phy.container, &phy.relationships, cut)
                .unwrap()
                .fingerprint
        })
        .collect();
    phy.sync().unwrap();
    drop(phy);
    let reopened = Phylactery::open(&path).unwrap();
    for (cut, expected) in expected.into_iter().enumerate() {
        assert_eq!(
            expected,
            relationship_cut(&reopened.container, &reopened.relationships, cut as u64)
                .unwrap()
                .fingerprint
        );
    }
    drop(reopened);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn relationship_prune_preserves_structural_evidence_and_unavailable_summary() {
    let dir = scratch();
    let mut phy = Phylactery::create(dir.join("owner.phy")).unwrap();
    let relationship = phy
        .publish_relationship(None, 0, draft("create"))
        .unwrap()
        .0;
    phy.tombstone_relationship(relationship.id, 1).unwrap();
    let output = dir.join("pruned.phy");
    phy.prune_tombstoned_history(
        &output,
        crate::HistoryPruneRequest {
            targets: vec![crate::owner_lifecycle::LifecycleTarget::Relationship(
                relationship.id,
            )],
        },
    )
    .unwrap();
    let pruned = Phylactery::open(&output).unwrap();
    let value = revision_value(&pruned.relationships.records()[0], true, None);
    assert!(value["summary"].is_null());
    assert_eq!(
        value["participants"].as_array().unwrap().len(),
        relationship.participants.len()
    );
    assert_eq!(
        value["evidence"].as_array().unwrap().len(),
        relationship.evidence.len()
    );
    assert!(matches!(
        pruned
            .historical_relationship_revision(relationship.id, 1)
            .unwrap(),
        crate::owner_lifecycle::HistoricalPayload::Pruned { .. }
    ));
    assert!(
        relationship_cut(
            &pruned.container,
            &pruned.relationships,
            pruned.relationship_version()
        )
        .is_ok()
    );
    drop(pruned);
    drop(phy);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
#[cfg(feature = "history-oracle-fixtures")]
fn authoritative_relationship_cuts_fingerprint_deterministically() {
    let root = PathBuf::from(std::env::var_os("RELIQUARY_HISTORY_FIXTURES").expect("fixture root"));
    let dir = scratch();
    for (index, corpus) in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ]
    .iter()
    .enumerate()
    {
        for (kind, name) in [("rel", "project.prj.rel"), ("phy", "user.phy")] {
            let path = dir.join(format!("{index}.{kind}"));
            let mut source = fs::File::open(root.join(corpus).join(name)).unwrap();
            let mut destination = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            std::io::copy(&mut source, &mut destination).unwrap();
            drop(destination);
            let verify = |container: &Container, store: &RelationshipStore| {
                let head = store.relationship_version();
                for cut in [0, head.min(1), head / 2, head] {
                    assert_eq!(
                        relationship_cut(container, store, cut).unwrap().fingerprint,
                        relationship_cut(container, store, cut).unwrap().fingerprint
                    );
                }
                assert_eq!(
                    relationship_cut(container, store, head)
                        .unwrap()
                        .current
                        .keys()
                        .copied()
                        .collect::<Vec<_>>(),
                    store
                        .current()
                        .into_iter()
                        .map(|r| r.id)
                        .collect::<Vec<_>>()
                );
                eprintln!(
                    "Relationship fixture {corpus}/{kind}: head {head}, {} revisions",
                    store.records().len()
                );
            };
            if kind == "rel" {
                let cva = crate::Cva::open(&path).unwrap();
                verify(&cva.container, &cva.relationships);
            } else {
                let phy = Phylactery::open(&path).unwrap();
                verify(&phy.container, &phy.relationships);
            }
        }
    }
    fs::remove_dir_all(dir).unwrap();
}
