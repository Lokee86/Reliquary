use super::*;
use crate::{EntityOwnerSession, draft_dream_organization};
use reliquary_container::{Container, ContainerIdentity};
use reliquary_dream::{DreamClassifier, DreamVerificationPolicy, DreamVerifier};
use reliquary_memory_core::{MemoryDraft, MemoryId, MemoryStore};
use reliquary_owner_session::{GraphAdmission, PhySession};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use reliquary_vector_store::{CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore};
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
            "dream-no-link-{}-{}.phy",
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

struct Endpoint {
    none: bool,
}
impl GeneralEndpoint for Endpoint {
    fn model(&self) -> &str {
        "dream-publication-test"
    }
    fn complete_json(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        Ok(if self.none {
            json!({"relation":"none","direction":"none","evidence":[]})
        } else {
            json!({
                "relation":"topical","direction":"undirected",
                "evidence":[
                    {"side":"a","quote":"Rollout"},
                    {"side":"b","quote":"Rollout"}
                ]
            })
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
fn fixture() -> (Temp, [u8; 16], MemoryId, MemoryId) {
    let path = Temp::new();
    let owner = [171; 16];
    let mut c = Container::create(&path.0, ContainerIdentity::phylactery(), owner).unwrap();
    PackedVectorStore::default().initialize(&mut c).unwrap();
    CompatibilityProfileStore::default()
        .initialize(&mut c)
        .unwrap();
    MemoryVectorStore::default().initialize(&mut c).unwrap();
    let mut store = MemoryStore::create(&mut c).unwrap();
    let a = store
        .publish(&mut c, None, 0, memory("no-link-a"))
        .unwrap()
        .revision
        .record
        .id;
    let b = store
        .publish(&mut c, None, 0, memory("no-link-b"))
        .unwrap()
        .revision
        .record
        .id;
    c.sync().unwrap();
    (path, owner, a, b)
}
fn admission(owner: [u8; 16]) -> GraphAdmission {
    GraphAdmission::for_replay(owner, [], [], [], [], [])
}
fn proposal(
    path: &Temp,
    owner: [u8; 16],
    a: MemoryId,
    b: MemoryId,
    none: bool,
) -> DreamOrganizationDraft {
    let session = PhySession::open_path(&path.0, &admission(owner), []).unwrap();
    draft_dream_organization(
        &EntityOwnerSession::Phy(&session),
        owner,
        a,
        &[b],
        &DreamClassifier::new(Endpoint { none }),
        None::<&DreamVerifier<Endpoint>>,
        DreamVerificationPolicy::default(),
    )
    .unwrap()
}

#[test]
fn accepted_no_link_is_one_durable_pair_and_replay_is_idempotent() {
    let (path, owner, a, b) = fixture();
    let draft = proposal(&path, owner, a, b, true);
    assert_eq!(draft.classifications.len(), 1);
    assert!(draft.plan.relation_changes.is_empty());
    let mut writer =
        reliquary_owner_session::PhyWriter::open_path(&path.0, &admission(owner), []).unwrap();
    assert!(
        commit_dream_zero_link(&mut DreamOwnerWriter::Phy(&mut writer), owner, &draft).unwrap()
    );
    drop(writer);
    let session = PhySession::open_path(&path.0, &admission(owner), []).unwrap();
    assert!(session.dream_pair_evaluated(a, b));
    assert_eq!(session.graph_version(), draft.graph_version);
    drop(session);
    let before_retry = fs::read(&path.0).unwrap();
    let mut writer =
        reliquary_owner_session::PhyWriter::open_path(&path.0, &admission(owner), []).unwrap();
    assert!(
        !commit_dream_zero_link(&mut DreamOwnerWriter::Phy(&mut writer), owner, &draft).unwrap()
    );
    drop(writer);
    assert_eq!(fs::read(&path.0).unwrap(), before_retry);
}

#[test]
fn no_link_publication_rejects_wrong_owner_and_positive_effects_without_writes() {
    let (path, owner, a, b) = fixture();
    let no_link = proposal(&path, owner, a, b, true);
    let positive = proposal(&path, owner, a, b, false);
    assert!(!positive.plan.relation_changes.is_empty());
    let before = fs::read(&path.0).unwrap();
    let mut writer =
        reliquary_owner_session::PhyWriter::open_path(&path.0, &admission(owner), []).unwrap();
    assert_eq!(
        commit_dream_zero_link(&mut DreamOwnerWriter::Phy(&mut writer), [9; 16], &no_link),
        Err(DreamWorkflowError::OwnerMismatch)
    );
    assert_eq!(
        commit_dream_zero_link(&mut DreamOwnerWriter::Phy(&mut writer), owner, &positive),
        Err(DreamWorkflowError::UnsupportedPublication)
    );
    drop(writer);
    assert_eq!(fs::read(&path.0).unwrap(), before);
}

#[test]
fn no_link_commit_rechecks_current_graph_cut_before_writing() {
    let (path, owner, a, b) = fixture();
    let draft = proposal(&path, owner, a, b, true);
    let change = GraphAdmission::for_changes(owner, [a.0, b.0], []);
    let mut container = Container::open_writer(&path.0).unwrap();
    let mut graph = reliquary_graph::GraphStore::open(&container, &change).unwrap();
    graph
        .put(
            &mut container,
            &change,
            &[reliquary_graph::RelationChange {
                source: reliquary_graph::NodeRef::Memory(a.0),
                target: reliquary_graph::NodeRef::Memory(b.0),
                kind: reliquary_graph::RelationKind::Topical,
                active: true,
                origin: reliquary_graph::RelationOrigin::User,
            }],
        )
        .unwrap();
    drop(container);
    let before = fs::read(&path.0).unwrap();
    let mut writer =
        reliquary_owner_session::PhyWriter::open_path(&path.0, &admission(owner), []).unwrap();
    assert_eq!(
        commit_dream_zero_link(&mut DreamOwnerWriter::Phy(&mut writer), owner, &draft),
        Err(DreamWorkflowError::StaleSnapshot)
    );
    drop(writer);
    assert_eq!(fs::read(&path.0).unwrap(), before);
}

#[test]
fn positive_relation_and_pair_reopen_together_and_retry_is_byte_stable() {
    let (path, owner, a, b) = fixture();
    let draft = proposal(&path, owner, a, b, false);
    let [change] = draft.plan.relation_changes.as_slice() else {
        panic!("one positive Dream effect expected")
    };
    assert!(draft.plan.lifecycle_intents.is_empty());
    let mut writer =
        reliquary_owner_session::PhyWriter::open_path(&path.0, &admission(owner), []).unwrap();
    assert!(commit_dream_relation(&mut DreamOwnerWriter::Phy(&mut writer), owner, &draft).unwrap());
    drop(writer);
    let reopened = PhySession::open_path(&path.0, &admission(owner), []).unwrap();
    assert!(reopened.dream_pair_evaluated(a, b));
    let relation = reopened
        .graph_relation(change.source, change.target, change.kind)
        .unwrap();
    assert!(relation.active);
    assert_eq!(relation.origin, RelationOrigin::Dream);
    assert_eq!(reopened.graph_version(), draft.graph_version + 1);
    drop(reopened);
    let accepted = fs::read(&path.0).unwrap();
    let mut retry =
        reliquary_owner_session::PhyWriter::open_path(&path.0, &admission(owner), []).unwrap();
    assert!(!commit_dream_relation(&mut DreamOwnerWriter::Phy(&mut retry), owner, &draft).unwrap());
    drop(retry);
    assert_eq!(fs::read(&path.0).unwrap(), accepted);
}

#[test]
fn positive_relation_rejects_stale_memory_and_wrong_owner_without_writes() {
    let (path, owner, a, b) = fixture();
    let draft = proposal(&path, owner, a, b, false);
    let original = fs::read(&path.0).unwrap();
    let mut writer =
        reliquary_owner_session::PhyWriter::open_path(&path.0, &admission(owner), []).unwrap();
    assert_eq!(
        commit_dream_relation(&mut DreamOwnerWriter::Phy(&mut writer), [98; 16], &draft),
        Err(DreamWorkflowError::OwnerMismatch)
    );
    drop(writer);
    assert_eq!(fs::read(&path.0).unwrap(), original);
    let mut container = Container::open_writer(&path.0).unwrap();
    let mut memories = MemoryStore::open(&container).unwrap();
    memories
        .publish(&mut container, Some(a), 1, memory("positive-stale-update"))
        .unwrap();
    container.sync().unwrap();
    drop(container);
    let before = fs::read(&path.0).unwrap();
    let mut writer =
        reliquary_owner_session::PhyWriter::open_path(&path.0, &admission(owner), []).unwrap();
    assert_eq!(
        commit_dream_relation(&mut DreamOwnerWriter::Phy(&mut writer), owner, &draft),
        Err(DreamWorkflowError::StaleSnapshot)
    );
    drop(writer);
    assert_eq!(fs::read(&path.0).unwrap(), before);
}
