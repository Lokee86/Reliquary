use super::*;
use reliquary_perception_store::EntityDraft;
use std::path::PathBuf;

pub(super) struct Fixture(pub(super) PathBuf);
impl Fixture {
    pub(super) fn new(suffix: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&dir).unwrap();
        Self(dir.join(format!("entity-prune-{}-{suffix}.phy", std::process::id())))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(super) fn draft(mutation: &str, name: &str, summary: &str, time: i64) -> EntityDraft {
    EntityDraft {
        canonical_name: name.into(),
        aliases: vec![],
        kind: "person".into(),
        summary: summary.into(),
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: time,
    }
}

#[test]
fn terminal_entity_prune_redacts_every_revision_and_preserves_clock() {
    let source = Fixture::new("source");
    let output = Fixture::new("output");
    let refused = Fixture::new("refused");
    let owner = [182; 16];
    PhyWriter::create_path(&source.0, owner).unwrap();
    let entity;
    let version;
    {
        let mut container = Container::open_writer_as(&source.0, FileKind::Phylactery).unwrap();
        let mut perception = PerceptionStore::open(&container).unwrap();
        entity = perception
            .publish_entity(
                &mut container,
                draft("entity-1", "Alice", &"private biography ".repeat(100), 1),
            )
            .unwrap();
        perception
            .update_entity(
                &mut container,
                entity.id,
                1,
                draft(
                    "entity-2",
                    "Alice",
                    &"updated private biography ".repeat(100),
                    2,
                ),
            )
            .unwrap();
        version = perception.version();
    }
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    assert!(
        prune_terminal_entity_descriptions_copy(&plan, &source.0, &refused.0, &[entity.id], &grant)
            .is_err()
    );
    assert!(!refused.0.exists());
    {
        let mut container = Container::open_writer_as(&source.0, FileKind::Phylactery).unwrap();
        let mut perception = PerceptionStore::open(&container).unwrap();
        perception
            .tombstone_entity(&mut container, entity.id, 2)
            .unwrap();
    }
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let before = fs::read(&source.0).unwrap();
    let report =
        prune_terminal_entity_descriptions_copy(&plan, &source.0, &output.0, &[entity.id], &grant)
            .unwrap();
    assert_eq!((report.pruned_entities, report.pruned_descriptions), (1, 2));
    assert!(report.transformed_bytes < report.source_bytes);
    assert_eq!(fs::read(&source.0).unwrap(), before);
    let reopened = PhySession::open_path(&output.0, &grant, []).unwrap();
    assert!(reopened.entity(entity.id).is_none());
    assert_eq!(
        reopened
            .entity_tombstone(entity.id)
            .unwrap()
            .perception_version,
        version + 1
    );
    for revision in 1..=2 {
        assert_eq!(
            reopened
                .entity_revision(entity.id, revision)
                .unwrap()
                .summary,
            ""
        );
        assert!(
            reopened
                .entity_description_prune(entity.id, revision)
                .is_some()
        );
    }
    assert!(
        reopened
            .retention_manifest(&report.manifest_digest)
            .is_some()
    );
}
