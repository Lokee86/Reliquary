use super::tests::{Fixture, draft};
use super::*;
use reliquary_perception_store::{
    RelationshipDraft, RelationshipEntityRef, RelationshipParticipant,
};
use std::io::Read;

fn tombstoned_source(source: &Fixture, owner: [u8; 16]) -> EntityId {
    PhyWriter::create_path(&source.0, owner).unwrap();
    let mut c = Container::open_writer_as(&source.0, FileKind::Phylactery).unwrap();
    let mut p = PerceptionStore::open(&c).unwrap();
    let entity = p
        .publish_entity(
            &mut c,
            draft(
                "alice",
                "Alice",
                "private original ".repeat(120).as_str(),
                1,
            ),
        )
        .unwrap();
    p.update_entity(
        &mut c,
        entity.id,
        1,
        draft(
            "alice-update",
            "Alice",
            "private later ".repeat(120).as_str(),
            2,
        ),
    )
    .unwrap();
    p.tombstone_entity(&mut c, entity.id, 2).unwrap();
    entity.id
}
#[test]
fn entity_reopen_rejects_missing_forged_and_duplicate_proof() {
    let source = Fixture::new("neg-source");
    let accepted = Fixture::new("neg-accepted");
    let owner = [212; 16];
    let id = tombstoned_source(&source, owner);
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    prune_terminal_entity_descriptions_copy(&plan, &source.0, &accepted.0, &[id], &grant).unwrap();
    for mode in ["missing", "forged", "duplicate"] {
        let failed = Fixture::new(mode);
        let copy = Container::open_read_only_as(&accepted.0, FileKind::Phylactery).unwrap();
        let mut dest = Container::create(&failed.0, copy.identity(), copy.owner_id()).unwrap();
        let mut cursor = copy.physical_cursor().unwrap();
        let mut saw = false;
        let mut duplicate = None;
        while let Some(mut frame) = cursor.next_frame().unwrap() {
            let mut bytes = Vec::new();
            frame.read_to_end(&mut bytes).unwrap();
            if bytes.starts_with(b"RELEPR01") {
                saw = true;
                if mode == "missing" {
                    continue;
                }
                if mode == "forged" {
                    bytes[96] ^= 1;
                }
                if mode == "duplicate" && duplicate.is_none() {
                    duplicate = Some(bytes.clone())
                }
            }
            dest.append(&bytes).unwrap();
        }
        if let Some(extra) = duplicate {
            dest.append(&extra).unwrap();
        }
        assert!(saw);
        dest.sync().unwrap();
        drop(dest);
        drop(copy);
        assert!(
            PhySession::open_path(&failed.0, &grant, []).is_err(),
            "{mode}"
        );
        assert!(PhySession::open_path(&accepted.0, &grant, []).is_ok());
    }
}
#[test]
fn entity_prune_refuses_active_relationship_references() {
    let source = Fixture::new("ref-source");
    let refused = Fixture::new("ref-refused");
    let owner = [213; 16];
    PhyWriter::create_path(&source.0, owner).unwrap();
    let id;
    {
        let mut c = Container::open_writer_as(&source.0, FileKind::Phylactery).unwrap();
        let mut p = PerceptionStore::open(&c).unwrap();
        let entity = p
            .publish_entity(
                &mut c,
                draft(
                    "person",
                    "Ada",
                    "private biography ".repeat(120).as_str(),
                    1,
                ),
            )
            .unwrap();
        id = entity.id;
        p.publish_relationship(
            &mut c,
            RelationshipDraft {
                kind: "knows".into(),
                participants: vec![RelationshipParticipant {
                    entity: RelationshipEntityRef {
                        owner_id: "another-authority".into(),
                        entity_id: id,
                    },
                    role: None,
                }],
                evidence: Vec::new(),
                summary: "still references Entity".repeat(40),
                mutation_id: "relationship".into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
        )
        .unwrap();
        p.tombstone_entity(&mut c, id, 1).unwrap();
    }
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let before = fs::read(&source.0).unwrap();
    let error =
        prune_terminal_entity_descriptions_copy(&plan, &source.0, &refused.0, &[id], &grant)
            .unwrap_err();
    assert!(error.contains("active Relationship"), "{error}");
    assert_eq!(fs::read(&source.0).unwrap(), before);
    assert!(!refused.0.exists());
}
