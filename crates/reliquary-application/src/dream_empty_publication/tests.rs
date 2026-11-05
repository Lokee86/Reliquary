use crate::*;
use reliquary_container::Container;
use reliquary_dream::{DreamClassifier, DreamVerificationPolicy, DreamVerifier};
use reliquary_memory_core::{MemoryDraft, MemoryId, MemoryStore};
use reliquary_owner_session::{
    DreamBeginAdmission, DreamMemoryCut, GraphAdmission, PhySession, PhyWriter, RelSession,
    RelWriter,
};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::{Value, json};
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
            "initial-cycle-{}-{}.rel",
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
struct Endpoint(bool);
impl GeneralEndpoint for Endpoint {
    fn model(&self) -> &str {
        "initial-dream-test"
    }
    fn complete_json(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        Ok(if self.0 {
            json!({"relation":"none", "direction":"none", "evidence":[]})
        } else {
            json!({"relation":"topical", "direction":"undirected", "evidence":[{"side":"a","quote":"Rollout"},{"side":"b","quote":"Rollout"}]})
        })
    }
}
fn memory(mutation: &str) -> MemoryDraft {
    MemoryDraft {
        title: "Rollout plan".into(),
        content: "Rollout evidence for this owner.".into(),
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

fn fixture(rel: bool) -> (Temp, [u8; 16], DreamMemoryCut, DreamMemoryCut) {
    let path = Temp::new();
    let owner = [207; 16];
    if rel {
        drop(
            RelWriter::create_path(&path.0, reliquary_container::ReliquaryScope::Project, owner)
                .unwrap(),
        );
    } else {
        drop(PhyWriter::create_path(&path.0, owner).unwrap());
    }
    let mut c = Container::open_writer(&path.0).unwrap();
    let mut m = MemoryStore::open(&c).unwrap();
    let older = m
        .publish(&mut c, None, 0, memory("older"))
        .unwrap()
        .revision
        .record;
    let mut source_draft = memory("source");
    source_draft.lifecycle_state = "extracted".into();
    let source = m
        .publish(&mut c, None, 0, source_draft)
        .unwrap()
        .revision
        .record;
    if rel {
        let mut f = reliquary_freshness_store::FreshnessStore::open(&c).unwrap();
        f.create_birth(
            &mut c,
            reliquary_freshness_store::FreshnessMemoryId(older.id.0),
            1,
            true,
        )
        .unwrap();
    }
    c.sync().unwrap();
    (
        path,
        owner,
        DreamMemoryCut {
            id: older.id,
            revision: 1,
            body_id: older.body_id,
        },
        DreamMemoryCut {
            id: source.id,
            revision: 1,
            body_id: source.body_id,
        },
    )
}
fn graph(owner: [u8; 16]) -> GraphAdmission {
    GraphAdmission::for_replay(owner, [], [], [], [], [])
}
fn draft(
    path: &Temp,
    owner: [u8; 16],
    rel: bool,
    source: MemoryId,
    candidates: &[MemoryId],
    admissions: &[DreamBeginAdmission],
    none: bool,
) -> DreamOrganizationDraft {
    let r;
    let p;
    let view = if rel {
        r = RelSession::open_path(&path.0, &graph(owner), [], admissions.iter().cloned()).unwrap();
        EntityOwnerSession::Rel(&r)
    } else {
        p = PhySession::open_path(&path.0, &graph(owner), admissions.iter().cloned()).unwrap();
        EntityOwnerSession::Phy(&p)
    };
    draft_dream_organization(
        &view,
        owner,
        source,
        candidates,
        &DreamClassifier::new(Endpoint(none)),
        None::<&DreamVerifier<Endpoint>>,
        DreamVerificationPolicy::default(),
    )
    .unwrap()
}

#[path = "no_link.rs"]
mod no_link;
#[path = "relations.rs"]
mod relations;
