//! Refuse forged retention proof, stale source cuts and active Relationship removal.
use super::tests::{Scratch, draft};
use super::*;
use std::io::Read;

fn terminal_source(source: &Scratch, owner: [u8; 16]) -> RelationshipId {
    PhyWriter::create_path(&source.0, owner).unwrap();
    let mut c = Container::open_writer_as(&source.0, FileKind::Phylactery).unwrap();
    let mut state = PerceptionStore::open(&c).unwrap();
    let relationship = state
        .publish_relationship(&mut c, draft("selected", "terminal description ", 1))
        .unwrap();
    state
        .update_relationship(
            &mut c,
            relationship.id,
            1,
            draft("updated", "updated description ", 2),
        )
        .unwrap();
    state
        .tombstone_relationship(&mut c, relationship.id, 2)
        .unwrap();
    relationship.id
}
#[test]
fn relationship_reopen_rejects_missing_or_forged_prune_evidence() {
    let source = Scratch::new("source-tamper", true);
    let output = Scratch::new("output-tamper", true);
    let owner = [201; 16];
    let id = terminal_source(&source, owner);
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    prune_terminal_relationship_descriptions_copy(&plan, &source.0, &output.0, &[id], &admission)
        .unwrap();
    for kind in ["missing", "forged"] {
        let invalid = Scratch::new(kind, true);
        let copy = Container::open_read_only_as(&output.0, FileKind::Phylactery).unwrap();
        let mut target = Container::create(&invalid.0, copy.identity(), copy.owner_id()).unwrap();
        let mut cursor = copy.physical_cursor().unwrap();
        let mut changed = false;
        while let Some(mut frame) = cursor.next_frame().unwrap() {
            let mut bytes = Vec::new();
            frame.read_to_end(&mut bytes).unwrap();
            if bytes.starts_with(b"RELRLP01")
                && std::str::from_utf8(&bytes[8..])
                    .unwrap()
                    .contains("\"Prune\"")
            {
                changed = true;
                if kind == "missing" {
                    continue;
                }
                let pos = bytes
                    .windows(b"\"manifest_digest\":[".len())
                    .position(|slice| slice == b"\"manifest_digest\":[")
                    .unwrap()
                    + b"\"manifest_digest\":[".len();
                bytes[pos] = if bytes[pos] == b'1' { b'2' } else { b'1' };
            }
            target.append(&bytes).unwrap();
        }
        assert!(changed);
        target.sync().unwrap();
        drop(target);
        drop(copy);
        assert!(PhySession::open_path(&invalid.0, &admission, []).is_err());
        assert!(PhySession::open_path(&output.0, &admission, []).is_ok());
    }
}
#[test]
fn relationship_prune_refuses_stale_plan_or_existing_output() {
    let source = Scratch::new("source-stale", true);
    let output = Scratch::new("output-stale", true);
    let owner = [202; 16];
    let id = terminal_source(&source, owner);
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let stale = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    {
        let mut c = Container::open_writer_as(&source.0, FileKind::Phylactery).unwrap();
        let mut state = PerceptionStore::open(&c).unwrap();
        state
            .publish_relationship(&mut c, draft("unrelated", "retained ", 1))
            .unwrap();
    }
    assert!(
        prune_terminal_relationship_descriptions_copy(
            &stale,
            &source.0,
            &output.0,
            &[id],
            &admission,
        )
        .is_err()
    );
    assert!(!output.0.exists());
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    std::fs::write(&output.0, b"unchanged collision").unwrap();
    let before = std::fs::read(&source.0).unwrap();
    assert!(
        prune_terminal_relationship_descriptions_copy(
            &plan,
            &source.0,
            &output.0,
            &[id],
            &admission,
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&source.0).unwrap(), before);
    assert_eq!(std::fs::read(&output.0).unwrap(), b"unchanged collision");
}
#[test]
fn relationship_prune_refuses_empty_description_and_post_tombstone_mutation() {
    let source = Scratch::new("source-empty", true);
    let output = Scratch::new("output-empty", true);
    let owner = [203; 16];
    PhyWriter::create_path(&source.0, owner).unwrap();
    let id;
    let before;
    {
        let mut c = Container::open_writer_as(&source.0, FileKind::Phylactery).unwrap();
        let mut state = PerceptionStore::open(&c).unwrap();
        let mut original = draft("empty", "", 1);
        original.summary.clear();
        id = state.publish_relationship(&mut c, original).unwrap().id;
        state.tombstone_relationship(&mut c, id, 1).unwrap();
        before = c.latest_version();
        assert!(
            state
                .update_relationship(&mut c, id, 1, draft("later", "later ", 2))
                .is_err()
        );
        assert_eq!(c.latest_version(), before);
    }
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    assert!(
        prune_terminal_relationship_descriptions_copy(
            &plan,
            &source.0,
            &output.0,
            &[id],
            &admission,
        )
        .is_err()
    );
    assert!(!output.0.exists());
}
