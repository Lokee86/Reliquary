use super::*;
use reliquary_graph::{RelationChange, RelationKind, RelationOrigin};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(1);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&root).unwrap();
        Self(root.join(format!(
            "batch-owner-{}-{}.rel",
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
fn memory() -> MemoryDraft {
    MemoryDraft {
        title: "First source".into(),
        content: "No matching owner candidate".into(),
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
        mutation_id: "initial-empty-source".into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn graph(owner: [u8; 16]) -> GraphAdmission {
    GraphAdmission::for_replay(owner, [], [], [], [], [])
}
fn fixture(rel: bool) -> (Temp, [u8; 16], DreamBatchCut) {
    let path = Temp::new();
    let owner = [229; 16];
    if rel {
        drop(
            RelWriter::create_path(&path.0, reliquary_container::ReliquaryScope::Project, owner)
                .unwrap(),
        );
    } else {
        drop(PhyWriter::create_path(&path.0, owner).unwrap());
    }
    let mut c = Container::open_writer(&path.0).unwrap();
    let mut memories = MemoryStore::open(&c).unwrap();
    let mut observed = Vec::new();
    for n in 0..3 {
        let mut draft = memory();
        draft.mutation_id = format!("batch-owner-{n}");
        let r = memories
            .publish(&mut c, None, 0, draft)
            .unwrap()
            .revision
            .record;
        observed.push(crate::DreamMemoryCut {
            id: r.id,
            revision: r.revision,
            body_id: r.body_id,
        });
    }
    c.sync().unwrap();
    drop(c);
    let source = observed[0].id;
    let pairs = observed[1..]
        .iter()
        .map(|o| DreamPairPublication {
            source,
            candidate: o.id,
            kind: Some(RelationKind::Supersedes),
            rejected: false,
        })
        .collect();
    let changes = observed[1..]
        .iter()
        .map(|o| RelationChange {
            source: NodeRef::Memory(source.0),
            target: NodeRef::Memory(o.id.0),
            kind: RelationKind::Supersedes,
            active: true,
            origin: RelationOrigin::Dream,
        })
        .collect();
    let mut lifecycles = vec![DreamLifecyclePublication {
        memory_id: source,
        lifecycle_state: "canonical".into(),
        archived: false,
        superseded_by: None,
    }];
    lifecycles.extend(observed[1..].iter().map(|o| DreamLifecyclePublication {
        memory_id: o.id,
        lifecycle_state: "archived".into(),
        archived: true,
        superseded_by: Some(source),
    }));
    (
        path,
        owner,
        DreamBatchCut {
            owner,
            publication_id: [42; 32],
            graph_version: 0,
            observed,
            pairs,
            changes,
            lifecycles,
            updated_at_ns: 10,
        },
    )
}
#[path = "negative.rs"]
mod negative;
#[path = "recovery.rs"]
mod recovery;
