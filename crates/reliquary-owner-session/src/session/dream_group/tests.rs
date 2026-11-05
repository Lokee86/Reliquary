use super::*;
use reliquary_graph::{NodeRef, RelationChange, RelationKind, RelationOrigin};
use reliquary_memory_core::MemoryDraft;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&dir).unwrap();
        Self(dir.join(format!(
            "grouped-dream-recovery-{}-{}.phy",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn memory(mutation: &str) -> MemoryDraft {
    MemoryDraft {
        title: "Recovered Dream".into(),
        content: "One durable Graph and pair transaction".into(),
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "user".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: None,
        source_time_ns: Some(1),
        temporal_inference: None,
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

#[test]
fn synced_group_replays_both_effects_without_in_memory_apply() {
    let file = Temp::new();
    let owner = [181; 16];
    drop(PhyWriter::create_path(&file.0, owner).unwrap());
    let mut container = Container::open_writer(&file.0).unwrap();
    let mut memories = MemoryStore::open(&container).unwrap();
    let a = memories
        .publish(&mut container, None, 0, memory("group-recovery-a"))
        .unwrap()
        .revision
        .record;
    let b = memories
        .publish(&mut container, None, 0, memory("group-recovery-b"))
        .unwrap()
        .revision
        .record;
    let group = GroupedDreamRelation::new(DreamRelationCut {
        owner_id: owner,
        graph_version: 0,
        source: crate::records::DreamMemoryCut {
            id: a.id,
            revision: a.revision,
            body_id: a.body_id,
        },
        candidate: crate::records::DreamMemoryCut {
            id: b.id,
            revision: b.revision,
            body_id: b.body_id,
        },
        change: RelationChange {
            source: NodeRef::Memory(a.id.0),
            target: NodeRef::Memory(b.id.0),
            kind: RelationKind::Topical,
            active: true,
            origin: RelationOrigin::Dream,
        },
    })
    .unwrap();
    // The durable append succeeds, but the writer disappears before applying
    // either mutable Graph or DreamPairs projection.
    container.append(&group.encode()).unwrap();
    container.sync().unwrap();
    drop(container);

    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let reopened = PhySession::open_path(&file.0, &admission, []).unwrap();
    assert_eq!(reopened.graph_version(), 1);
    assert!(reopened.dream_pair_evaluated(a.id, b.id));
    assert!(
        reopened
            .graph_relation(
                NodeRef::Memory(a.id.0),
                NodeRef::Memory(b.id.0),
                RelationKind::Topical
            )
            .is_some()
    );
}
