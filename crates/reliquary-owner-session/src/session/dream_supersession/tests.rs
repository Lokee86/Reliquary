use super::*;
use reliquary_container::ContainerIdentity;
use reliquary_graph::{RelationChange, RelationKind, RelationOrigin};
use reliquary_memory_core::{MemoryDraft, MemoryStore};
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
            "supersession-recovery-{}-{}.phy",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
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
        title: "Dream evidence".into(),
        content: "Crash-safe supersession".into(),
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
fn synced_supersession_replays_all_three_effects_before_memory_projection() {
    let file = Temp::new();
    let owner = [187; 16];
    let mut c = Container::create(&file.0, ContainerIdentity::phylactery(), owner).unwrap();
    super::super::create::initialize_owner(&mut c).unwrap();
    let mut memory_store = MemoryStore::open(&c).unwrap();
    let newer = memory_store
        .publish(&mut c, None, 0, memory("sup-new"))
        .unwrap()
        .revision
        .record;
    let older = memory_store
        .publish(&mut c, None, 0, memory("sup-old"))
        .unwrap()
        .revision
        .record;
    c.sync().unwrap();
    let cut = DreamRelationCut {
        owner_id: owner,
        graph_version: 0,
        source: crate::records::DreamMemoryCut {
            id: newer.id,
            revision: newer.revision,
            body_id: newer.body_id,
        },
        candidate: crate::records::DreamMemoryCut {
            id: older.id,
            revision: older.revision,
            body_id: older.body_id,
        },
        change: RelationChange {
            source: NodeRef::Memory(newer.id.0),
            target: NodeRef::Memory(older.id.0),
            kind: RelationKind::Supersedes,
            active: true,
            origin: RelationOrigin::Dream,
        },
    };
    let delta = memory_store
        .prepare_grouped_supersession(
            older.id,
            older.revision,
            newer.id,
            GroupedDreamSupersession::mutation_id(&cut),
            7,
        )
        .unwrap();
    let group = GroupedDreamSupersession::new(cut, 7, delta).unwrap();
    // Simulate process exit immediately after durable sync: neither the
    // Graph, Memory nor DreamPairs in-memory projections are applied.
    c.append(&group.encode()).unwrap();
    c.sync().unwrap();
    drop(c);
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let session = PhySession::open_path(&file.0, &admission, []).unwrap();
    assert!(session.dream_pair_evaluated(newer.id, older.id));
    assert_eq!(session.graph_version(), 1);
    assert!(
        session
            .graph_relation(
                NodeRef::Memory(newer.id.0),
                NodeRef::Memory(older.id.0),
                RelationKind::Supersedes,
            )
            .unwrap()
            .active
    );
    let history = session.memory_history(older.id).unwrap().unwrap();
    assert_eq!(history.revisions.len(), 2);
    assert_eq!(history.revisions[1].record.superseded_by, Some(newer.id));
    assert!(history.revisions[1].record.archived);
}
