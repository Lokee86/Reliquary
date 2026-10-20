use super::EntityReplayPublication;
use crate::entity_model::EntityRecord;
use crate::{Cva, EntityError, EntityGlobalId, EntityId};

#[test]
fn replay_of_an_accepted_older_mutation_is_inert_while_entity_remains_active() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp").join(format!("entity-replay-retry-{}.rel", uuid::Uuid::new_v4()));
    let mut owner = Cva::create(&path).unwrap();
    let first = EntityRecord {
        id: EntityId([8; 32]), global_id: Some(EntityGlobalId([9; 16])), revision: 1,
        canonical_name: "Name".into(), aliases: vec![], kind: "project".into(),
        summary: "First".into(), mutation_id: "first".into(), created_at_ns: 1,
        updated_at_ns: 2, global_version: 1, entity_version: 1,
    };
    assert!(owner.entities.replay_publication(&mut owner.container,
        EntityReplayPublication::Record(first.clone())).unwrap());
    let mut next = first.clone();
    next.revision = 2;
    next.summary = "Second".into();
    next.mutation_id = "second".into();
    assert!(owner.entities.replay_publication(&mut owner.container,
        EntityReplayPublication::Record(next)).unwrap());
    let before = std::fs::read(&path).unwrap();
    let clocks = (owner.entity_version(), owner.latest_global_version());
    assert!(!owner.entities.replay_publication(&mut owner.container,
        EntityReplayPublication::Record(first.clone())).unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!((owner.entity_version(), owner.latest_global_version()), clocks);
    let mut conflicting = first;
    conflicting.global_id = None;
    assert!(matches!(owner.entities.replay_publication(&mut owner.container,
        EntityReplayPublication::Record(conflicting)), Err(EntityError::MutationConflict)));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    drop(owner);
    std::fs::remove_file(path).unwrap();
}
