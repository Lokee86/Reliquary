use super::*;
use reliquary_perception_store::{
    RelationshipDraft, RelationshipEntityRef, RelationshipMemoryRef, RelationshipParticipant,
};
use std::path::PathBuf;
pub(super) struct Scratch(pub(super) PathBuf);
impl Scratch {
    pub(super) fn new(suffix: &str, phy: bool) -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&dir).unwrap();
        Self(dir.join(format!("relationship-prune-{}-{}-{suffix}.{}",
   std::process::id(),std::time::SystemTime::now().duration_since(
    std::time::UNIX_EPOCH).unwrap().as_nanos(),
   if phy{"phy"}else{"rel"})))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
pub(super) fn draft(mutation: &str, text: &str, at: i64) -> RelationshipDraft {
    RelationshipDraft {
        kind: "observes".into(),
        participants: vec![RelationshipParticipant {
            entity: RelationshipEntityRef {
                owner_id: "remote-owner".into(),
                entity_id: EntityId([41; 32]),
            },
            role: Some("witness".into()),
        }],
        evidence: vec![RelationshipMemoryRef {
            owner_id: "source-owner".into(),
            memory_id: [42; 32],
        }],
        summary: text.repeat(100),
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: at,
    }
}
#[test]
fn terminal_relationship_prune_preserves_revisions_and_owner_clocks_rel_phy() {
    for phy in [true, false] {
        let source = Scratch::new("source", phy);
        let target = Scratch::new("target", phy);
        let refused = Scratch::new("refused", phy);
        let owner = [190; 16];
        if phy {
            PhyWriter::create_path(&source.0, owner).unwrap();
        } else {
            RelWriter::create_path(
                &source.0,
                reliquary_container::ReliquaryScope::Project,
                owner,
            )
            .unwrap();
        }
        let kind = if phy {
            FileKind::Phylactery
        } else {
            FileKind::Reliquary
        };
        let relationship;
        let version;
        let relationship_version;
        {
            let mut c = Container::open_writer_as(&source.0, kind).unwrap();
            let mut store = PerceptionStore::open(&c).unwrap();
            relationship = store
                .publish_relationship(&mut c, draft("r-1", "confidential first text ", 1))
                .unwrap();
            store
                .update_relationship(
                    &mut c,
                    relationship.id,
                    1,
                    draft("r-2", "confidential second text ", 2),
                )
                .unwrap();
            version = store.version() + 1;
            relationship_version = store.relationship_version() + 1;
        }
        let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
        let prior = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
        assert!(
            prune_terminal_relationship_descriptions_copy(
                &prior,
                &source.0,
                &refused.0,
                &[relationship.id],
                &admission
            )
            .is_err()
        );
        assert!(!refused.0.exists());
        {
            let mut c = Container::open_writer_as(&source.0, kind).unwrap();
            let mut store = PerceptionStore::open(&c).unwrap();
            store
                .tombstone_relationship(&mut c, relationship.id, 2)
                .unwrap();
        }
        let prior = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
        let untouched = fs::read(&source.0).unwrap();
        let report = prune_terminal_relationship_descriptions_copy(
            &prior,
            &source.0,
            &target.0,
            &[relationship.id],
            &admission,
        )
        .unwrap();
        assert_eq!(
            (report.pruned_relationships, report.pruned_descriptions),
            (1, 2)
        );
        assert!(report.transformed_bytes < report.source_bytes);
        assert_eq!(fs::read(&source.0).unwrap(), untouched);
        let copy = Container::open_read_only_as(&target.0, kind).unwrap();
        let reopened = PerceptionStore::open(&copy).unwrap();
        assert_eq!(reopened.version(), version);
        assert_eq!(reopened.relationship_version(), relationship_version);
        assert!(reopened.relationship(relationship.id).is_none());
        assert_eq!(
            reopened
                .relationship_tombstone(relationship.id)
                .unwrap()
                .revision,
            2
        );
        for rev in 1..=2 {
            let original =
                PerceptionStore::open(&Container::open_read_only_as(&source.0, kind).unwrap())
                    .unwrap();
            let first = original
                .relationship_revision(relationship.id, rev)
                .unwrap();
            let second = reopened
                .relationship_revision(relationship.id, rev)
                .unwrap();
            assert!(second.summary.is_empty());
            assert_eq!(
                (
                    &second.id,
                    &second.participants,
                    &second.evidence,
                    &second.kind,
                    &second.mutation_id
                ),
                (
                    &first.id,
                    &first.participants,
                    &first.evidence,
                    &first.kind,
                    &first.mutation_id
                )
            );
            assert_eq!(
                reopened
                    .relationship_description_prune(relationship.id, rev)
                    .unwrap()
                    .manifest_digest,
                report.manifest_digest
            );
        }
        if phy {
            assert!(PhySession::open_path(&target.0, &admission, []).is_ok())
        } else {
            assert!(RelSession::open_path(&target.0, &admission, [], []).is_ok())
        }
    }
}
#[test]
fn relationship_prune_fails_on_duplicate_target_and_wrong_file_cut() {
    let source = Scratch::new("source", true);
    let target = Scratch::new("target", true);
    let owner = [191; 16];
    PhyWriter::create_path(&source.0, owner).unwrap();
    let relationship = {
        let mut c = Container::open_writer_as(&source.0, FileKind::Phylactery).unwrap();
        let mut state = PerceptionStore::open(&c).unwrap();
        let r = state
            .publish_relationship(&mut c, draft("r-a", "secret ", 1))
            .unwrap();
        state.tombstone_relationship(&mut c, r.id, 1).unwrap();
        r
    };
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let before = fs::read(&source.0).unwrap();
    assert!(
        prune_terminal_relationship_descriptions_copy(
            &plan,
            &source.0,
            &target.0,
            &[relationship.id, relationship.id],
            &admission
        )
        .is_err()
    );
    assert!(!target.0.exists());
    assert_eq!(fs::read(&source.0).unwrap(), before);
}
