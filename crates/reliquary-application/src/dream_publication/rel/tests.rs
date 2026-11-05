use super::*;
use crate::{EntityOwnerSession, draft_dream_organization};
use reliquary_container::{Container, ReliquaryScope};
use reliquary_dream::{DreamClassifier, DreamVerificationPolicy, DreamVerifier};
use reliquary_memory_core::{MemoryDraft, MemoryStore};
use reliquary_owner_session::{GraphAdmission, RelSession, RelWriter};
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
            "dream-rel-{}-{}.rel",
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

struct NoneEndpoint;
impl GeneralEndpoint for NoneEndpoint {
    fn model(&self) -> &str {
        "dream-rel-test"
    }
    fn complete_json(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        Ok(json!({"relation":"none","direction":"none","evidence":[]}))
    }
}
fn draft(mutation: &str) -> MemoryDraft {
    MemoryDraft {
        title: "Maintenance review".into(),
        content: "Separate nonoverlapping documentation.".into(),
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
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
        source_time_ns: Some(100),
        temporal_inference: None,
        mutation_id: mutation.into(),
        created_at_ns: 100,
        updated_at_ns: 100,
    }
}
fn admission(owner: [u8; 16]) -> GraphAdmission {
    GraphAdmission::for_replay(owner, [], [], [], [], [])
}

#[test]
fn rel_zero_link_publication_is_owner_scoped_and_replayable() {
    let file = Temp::new();
    let owner = [172; 16];
    drop(RelWriter::create_path(&file.0, ReliquaryScope::Project, owner).unwrap());
    let mut container = Container::open_writer(&file.0).unwrap();
    let mut memory = MemoryStore::open(&container).unwrap();
    let a = memory
        .publish(&mut container, None, 0, draft("rel-dream-a"))
        .unwrap()
        .revision
        .record
        .id;
    let b = memory
        .publish(&mut container, None, 0, draft("rel-dream-b"))
        .unwrap()
        .revision
        .record
        .id;
    container.sync().unwrap();
    drop(container);

    let session = RelSession::open_path(&file.0, &admission(owner), [], []).unwrap();
    let proposal = draft_dream_organization(
        &EntityOwnerSession::Rel(&session),
        owner,
        a,
        &[b],
        &DreamClassifier::new(NoneEndpoint),
        None::<&DreamVerifier<NoneEndpoint>>,
        DreamVerificationPolicy::default(),
    )
    .unwrap();
    drop(session);
    let mut writer = RelWriter::open_path(&file.0, admission(owner), [], []).unwrap();
    assert!(
        commit_dream_zero_link(&mut DreamOwnerWriter::Rel(&mut writer), owner, &proposal).unwrap()
    );
    drop(writer);
    let session = RelSession::open_path(&file.0, &admission(owner), [], []).unwrap();
    assert!(session.dream_pair_evaluated(a, b));
    assert_eq!(session.graph_version(), 0);
    drop(session);
    let before = fs::read(&file.0).unwrap();
    let mut writer = RelWriter::open_path(&file.0, admission(owner), [], []).unwrap();
    assert!(
        !commit_dream_zero_link(&mut DreamOwnerWriter::Rel(&mut writer), owner, &proposal).unwrap()
    );
    drop(writer);
    assert_eq!(fs::read(&file.0).unwrap(), before);
}

#[test]
fn rel_no_link_commit_rejects_stale_memory_revision_without_mutation() {
    let file = Temp::new();
    let owner = [173; 16];
    drop(RelWriter::create_path(&file.0, ReliquaryScope::Project, owner).unwrap());
    let mut container = Container::open_writer(&file.0).unwrap();
    let mut memories = MemoryStore::open(&container).unwrap();
    let a = memories
        .publish(&mut container, None, 0, draft("rel-stale-a"))
        .unwrap()
        .revision
        .record
        .id;
    let b = memories
        .publish(&mut container, None, 0, draft("rel-stale-b"))
        .unwrap()
        .revision
        .record
        .id;
    container.sync().unwrap();
    drop(container);

    let session = RelSession::open_path(&file.0, &admission(owner), [], []).unwrap();
    let proposal = draft_dream_organization(
        &EntityOwnerSession::Rel(&session),
        owner,
        a,
        &[b],
        &DreamClassifier::new(NoneEndpoint),
        None::<&DreamVerifier<NoneEndpoint>>,
        DreamVerificationPolicy::default(),
    )
    .unwrap();
    drop(session);

    let mut container = Container::open_writer(&file.0).unwrap();
    let mut memories = MemoryStore::open(&container).unwrap();
    memories
        .publish(&mut container, Some(a), 1, draft("rel-stale-updated"))
        .unwrap();
    container.sync().unwrap();
    drop(container);

    let before = fs::read(&file.0).unwrap();
    let mut writer = RelWriter::open_path(&file.0, admission(owner), [], []).unwrap();
    assert_eq!(
        commit_dream_zero_link(&mut DreamOwnerWriter::Rel(&mut writer), owner, &proposal),
        Err(DreamWorkflowError::StaleSnapshot)
    );
    drop(writer);
    assert_eq!(fs::read(&file.0).unwrap(), before);
}

struct PositiveEndpoint;
impl GeneralEndpoint for PositiveEndpoint {
    fn model(&self) -> &str {
        "dream-rel-positive"
    }
    fn complete_json(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        Ok(json!({
            "relation":"topical","direction":"undirected",
            "evidence":[
                {"side":"a","quote":"Maintenance"},
                {"side":"b","quote":"Maintenance"}
            ]
        }))
    }
}

#[test]
fn rel_grouped_positive_relation_and_pair_replay_and_retry() {
    let file = Temp::new();
    let owner = [174; 16];
    drop(RelWriter::create_path(&file.0, ReliquaryScope::Project, owner).unwrap());
    let mut container = Container::open_writer(&file.0).unwrap();
    let mut memories = MemoryStore::open(&container).unwrap();
    let a = memories
        .publish(&mut container, None, 0, draft("rel-positive-a"))
        .unwrap()
        .revision
        .record
        .id;
    let b = memories
        .publish(&mut container, None, 0, draft("rel-positive-b"))
        .unwrap()
        .revision
        .record
        .id;
    container.sync().unwrap();
    drop(container);
    let session = RelSession::open_path(&file.0, &admission(owner), [], []).unwrap();
    let proposal = draft_dream_organization(
        &EntityOwnerSession::Rel(&session),
        owner,
        a,
        &[b],
        &DreamClassifier::new(PositiveEndpoint),
        None::<&DreamVerifier<PositiveEndpoint>>,
        DreamVerificationPolicy::default(),
    )
    .unwrap();
    let [change] = proposal.plan.relation_changes.as_slice() else {
        panic!("expected one relation")
    };
    drop(session);
    let mut writer = RelWriter::open_path(&file.0, admission(owner), [], []).unwrap();
    assert!(
        commit_dream_relation(&mut DreamOwnerWriter::Rel(&mut writer), owner, &proposal).unwrap()
    );
    drop(writer);
    let session = RelSession::open_path(&file.0, &admission(owner), [], []).unwrap();
    assert!(session.dream_pair_evaluated(a, b));
    let relation = session
        .graph_relation(change.source, change.target, change.kind)
        .unwrap();
    assert_eq!(relation.origin, RelationOrigin::Dream);
    assert!(relation.active);
    drop(session);
    let committed = fs::read(&file.0).unwrap();
    let mut retry = RelWriter::open_path(&file.0, admission(owner), [], []).unwrap();
    assert!(
        !commit_dream_relation(&mut DreamOwnerWriter::Rel(&mut retry), owner, &proposal).unwrap()
    );
    drop(retry);
    assert_eq!(fs::read(&file.0).unwrap(), committed);
}

struct VerifiedSupersession;
impl GeneralEndpoint for VerifiedSupersession {
    fn model(&self) -> &str {
        "verified-rel-supersession"
    }
    fn complete_json(
        &self,
        _: &str,
        _: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        if schema == "dream_pair_verification" {
            return Ok(json!({"relation_supported":"yes",
                "direction_supported":"yes","evidence_supported":"yes"}));
        }
        Ok(
            json!({"relation":"supersedes","direction":"a_to_b","evidence":[
            {"side":"a","quote":"Maintenance"},
            {"side":"b","quote":"Maintenance"}]}),
        )
    }
}

#[test]
fn rel_verified_supersession_reopens_history_and_retries_idempotently() {
    let file = Temp::new();
    let owner = [175; 16];
    drop(RelWriter::create_path(&file.0, ReliquaryScope::Project, owner).unwrap());
    let mut container = Container::open_writer(&file.0).unwrap();
    let mut memory = MemoryStore::open(&container).unwrap();
    let a = memory
        .publish(&mut container, None, 0, draft("rel-sup-a"))
        .unwrap()
        .revision
        .record
        .id;
    let b = memory
        .publish(&mut container, None, 0, draft("rel-sup-b"))
        .unwrap()
        .revision
        .record
        .id;
    container.sync().unwrap();
    drop(container);
    let session = RelSession::open_path(&file.0, &admission(owner), [], []).unwrap();
    let proposal = draft_dream_organization(
        &EntityOwnerSession::Rel(&session),
        owner,
        a,
        &[b],
        &DreamClassifier::new(VerifiedSupersession),
        Some(&DreamVerifier::new(VerifiedSupersession)),
        DreamVerificationPolicy::default(),
    )
    .unwrap();
    let relation = proposal.plan.relation_changes[0];
    let old = proposal.plan.lifecycle_intents[0].memory_id;
    let newer = proposal.plan.lifecycle_intents[0].superseded_by.unwrap();
    drop(session);
    let mut writer = RelWriter::open_path(&file.0, admission(owner), [], []).unwrap();
    assert!(
        crate::commit_dream_supersession(
            &mut DreamOwnerWriter::Rel(&mut writer),
            owner,
            &proposal,
            101
        )
        .unwrap()
    );
    drop(writer);
    let reopened = RelSession::open_path(&file.0, &admission(owner), [], []).unwrap();
    assert!(reopened.dream_pair_evaluated(a, b));
    assert!(
        reopened
            .graph_relation(relation.source, relation.target, relation.kind)
            .unwrap()
            .active
    );
    let history = reopened.memory_history(old).unwrap().unwrap();
    assert_eq!(history.revisions.len(), 2);
    assert_eq!(history.revisions[1].record.superseded_by, Some(newer));
    assert!(history.revisions[1].record.archived);
    assert_eq!(history.revisions[1].record.updated_at_ns, 101);
    drop(reopened);
    let bytes = fs::read(&file.0).unwrap();
    let mut retry = RelWriter::open_path(&file.0, admission(owner), [], []).unwrap();
    assert!(
        !crate::commit_dream_supersession(
            &mut DreamOwnerWriter::Rel(&mut retry),
            owner,
            &proposal,
            101
        )
        .unwrap()
    );
    drop(retry);
    assert_eq!(fs::read(&file.0).unwrap(), bytes);
}
