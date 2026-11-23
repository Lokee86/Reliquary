//! Public cross-REL acceptance: authored writers, configured inference, and durable history.
use reliquary_memory::{
    ConfiguredRuntime, CorrectMemoryRequest, CreateRelationshipRequest, EmbeddingEndpoint,
    EmbeddingEndpointError, EmbeddingNormalization, EntityId, Gateway, GeneralEndpoint,
    GeneralEndpointError, HostCredentialBinding, HostedEmbeddingInputSemantics,
    HostedInferenceIdentity, IdentityComparisonState, IdentityOutcome, IdentityReason,
    MemoryBodyId, MemoryId, MemoryTextField, OwnerGrant, OwnerId, OwnerKind, ProductErrorKind, Rel,
    RelationshipParticipantEvidence, SwitchboardEmbeddingMode, TrustedHostAdmission,
    WorkspaceManagement,
};
use reliquary_memory_core::{MemoryDraft, MemorySourceRef};
use reliquary_owner_session::{EntityMentionCommit, RelWriter};
use reliquary_perception_store::{
    EntityResolutionReason, MemoryEntityMentionKey, MemoryEntityResolutionStatus,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

const PERSON: &str = "Alex Chen";
const CONTRACT: &str = "Project Meridian Contract";
const QUOTE: &str = "Alex Chen signed Project Meridian Contract.";

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("entity-identity-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
struct Script {
    calls: AtomicUsize,
    // A gate allows a test to mutate an owner after evaluation has begun.
    gate: Mutex<bool>,
    wake: Condvar,
}
impl Script {
    fn pause(&self) {
        *self.gate.lock().unwrap() = true;
    }
    fn entered(&self, expected_calls: usize) {
        let gate = self.gate.lock().unwrap();
        let (_gate, timeout) = self
            .wake
            .wait_timeout_while(gate, Duration::from_secs(10), |_| {
                self.calls.load(Ordering::SeqCst) < expected_calls
            })
            .unwrap();
        assert!(
            !timeout.timed_out(),
            "evaluation did not reach the pause gate"
        );
    }
    fn release(&self) {
        *self.gate.lock().unwrap() = false;
        self.wake.notify_all();
    }
}
impl GeneralEndpoint for Script {
    fn model(&self) -> &str {
        "identity-script"
    }
    fn complete_json(
        &self,
        _: &str,
        payload: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        assert_eq!(schema, "entity_identity_comparison");
        assert!(
            payload.len() <= 16 * 1024,
            "pair request exceeded the Entity input budget"
        );
        let input: Value = serde_json::from_str(payload).unwrap();
        assert!(input["left"]["relationships"].is_array());
        assert!(input["right"]["relationships"].is_array());
        self.calls.fetch_add(1, Ordering::SeqCst);
        let gate = self.gate.lock().unwrap();
        self.wake.notify_all();
        let (_gate, timeout) = self
            .wake
            .wait_timeout_while(gate, Duration::from_secs(15), |state| *state)
            .unwrap();
        if timeout.timed_out() {
            return Err(GeneralEndpointError::Failure(
                "fixture evaluation gate timed out".into(),
            ));
        }
        let signature = |side: &str| {
            input[side]["relationships"]
                .as_array()
                .unwrap()
                .iter()
                .position(|r| r["kind"] == "signed" && r["quote"] == QUOTE)
        };
        let (Some(left), Some(right)) = (signature("left"), signature("right")) else {
            return Ok(json!({"outcome":"unresolved","reason":"ambiguity","citations":[]}));
        };
        Ok(json!({
            "outcome":"same_identity", "reason":"corroborated",
            "citations":[
                {"side":"left","relationship":left,"memory":0,"fact":CONTRACT},
                {"side":"right","relationship":right,"memory":0,"fact":CONTRACT}
            ]
        }))
    }
}
struct UnusedEmbedding;
impl EmbeddingEndpoint for UnusedEmbedding {
    fn model(&self) -> Option<&str> {
        Some("unused")
    }
    fn dimensions(&self) -> u32 {
        2
    }
    fn normalization(&self) -> EmbeddingNormalization {
        EmbeddingNormalization::L2
    }
    fn embed(
        &self,
        _: SwitchboardEmbeddingMode,
        _: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        panic!("identity comparison must not embed")
    }
}
fn runtime(script: Arc<Script>) -> ConfiguredRuntime {
    ConfiguredRuntime::hosted(
        script,
        Arc::new(UnusedEmbedding),
        HostedInferenceIdentity::new(
            "fixture",
            "identity-script",
            "fixture",
            "unused",
            2,
            HostedEmbeddingInputSemantics::Shared,
            EmbeddingNormalization::L2,
        )
        .unwrap(),
    )
    .unwrap()
}

fn draft(owner: [u8; 16], name: &str, text: &str) -> MemoryDraft {
    MemoryDraft {
        title: "Authored evidence".into(),
        content: text.into(),
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
        lifecycle_state: "current".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: Some(MemorySourceRef {
            owner_id: owner.iter().map(|b| format!("{b:02x}")).collect(),
            principal_id: Some("author".into()),
            source_episode_id: [0; 32],
            source_node_id: format!("independent-{}-{name}", owner[0]),
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
        }),
        source_time_ns: Some(10),
        temporal_inference: None,
        mutation_id: name.into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}
fn seed(path: &Path, owner: [u8; 16]) -> (EntityId, EntityId) {
    let mut writer = RelWriter::create_current_path(path, owner).unwrap();
    let mut ids = Vec::new();
    for (mutation, text, kind) in [
        ("person", PERSON, "person"),
        ("contract", CONTRACT, "artifact"),
    ] {
        let memory = writer
            .publish_direct_memory("author", draft(owner, mutation, text))
            .unwrap();
        let resolution = writer
            .commit_entity_mention(EntityMentionCommit {
                owner_id: owner,
                memory_revision: memory.record.revision,
                body_id: memory.record.body_id,
                key: MemoryEntityMentionKey {
                    memory_id: memory.record.id.0,
                    field: MemoryTextField::Content,
                    start_byte: 0,
                    end_byte: text.len() as u32,
                },
                surface: text.into(),
                expected_revision: 0,
                updated_at_ns: 11,
                status: MemoryEntityResolutionStatus::Unresolved {
                    reason: EntityResolutionReason::NamedReferent,
                },
                target_revisions: vec![],
                new_entity: Some((kind.into(), "Authored referent".into())),
            })
            .unwrap();
        let MemoryEntityResolutionStatus::Resolved { entity_id, .. } = resolution.status else {
            panic!("seed unresolved")
        };
        ids.push(entity_id);
    }
    (ids[0], ids[1])
}
fn bytes(text: &str) -> [u8; 32] {
    std::array::from_fn(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).unwrap())
}
fn evidence(
    work: &WorkspaceManagement,
    person: EntityId,
    contract: EntityId,
) -> reliquary_memory::MemorySummary {
    relationship(work, person, contract, "signed", "signatory", QUOTE)
}

fn relationship(
    work: &WorkspaceManagement,
    person: EntityId,
    contract: EntityId,
    kind: &str,
    role: &str,
    quote: &str,
) -> reliquary_memory::MemorySummary {
    let memory = work
        .create_memory(
            format!("Contract {kind}"),
            quote.into(),
            "fact".into(),
            "project".into(),
            20,
        )
        .unwrap();
    work.create_relationship(CreateRelationshipRequest {
        owner_id: work.owner_id().as_bytes(),
        kind: kind.into(),
        participants: vec![
            RelationshipParticipantEvidence {
                entity_id: person,
                entity_revision: 1,
                role: Some(role.into()),
            },
            RelationshipParticipantEvidence {
                entity_id: contract,
                entity_revision: 1,
                role: Some("contract".into()),
            },
        ],
        source_memory_id: MemoryId(bytes(&memory.id)),
        source_memory_revision: memory.revision,
        source_body_id: MemoryBodyId(bytes(&memory.body_id)),
        source_field: MemoryTextField::Content,
        start_byte: 0,
        end_byte: quote.len(),
        source_quote: quote.into(),
        mutation_id: format!("{kind}-relationship"),
        updated_at_ns: 21,
    })
    .unwrap();
    memory
}
fn wait_revision(
    work: &WorkspaceManagement,
    local: EntityId,
    peer: OwnerId,
    remote: EntityId,
    revision: u64,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(view) = work
            .inspect_entity_identity(local, peer, remote, None)
            .unwrap()
        {
            if view.receipt.revision == revision && view.state == IdentityComparisonState::Current {
                return;
            }
        }
        assert!(
            Instant::now() < deadline,
            "worker did not publish revision {revision}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn identity_namesakes_revisit_correction_restart_and_grants() {
    let dir = Directory::new();
    let ap = dir.0.join("a.rel");
    let bp = dir.0.join("b.rel");
    let (a_entity, a_contract) = seed(&ap, [1; 16]);
    let (b_entity, b_contract) = seed(&bp, [2; 16]);
    assert_eq!(
        a_entity, b_entity,
        "equal Entity bytes remain owner-qualified"
    );
    let a_rel = Rel::open(&ap).unwrap();
    let b_rel = Rel::open(&bp).unwrap();
    let gateway = Gateway::new();
    let a = gateway.mount_rel(&a_rel, true).unwrap();
    let b = gateway.mount_rel(&b_rel, true).unwrap();
    let manager_token = "identity-manager-credential-000000000000000";
    let viewer_token = "identity-viewer-credential-0000000000000000";
    let limited_token = "identity-limited-credential-000000000000000";
    let (admission, _) = TrustedHostAdmission::new([
        HostCredentialBinding::new(
            "author",
            manager_token,
            [OwnerGrant::ManageRel(a), OwnerGrant::ManageRel(b)],
        ),
        HostCredentialBinding::new(
            "viewer",
            viewer_token,
            [OwnerGrant::ViewRel(a), OwnerGrant::ViewRel(b)],
        ),
        HostCredentialBinding::new("limited", limited_token, [OwnerGrant::ManageRel(a)]),
    ])
    .install(&gateway)
    .unwrap();
    let manager = admission.authenticate(manager_token).unwrap();
    let work = manager.workspace_management(a).unwrap();
    let peer = manager.workspace_management(b).unwrap();
    let view = admission
        .authenticate(viewer_token)
        .unwrap()
        .workspace_management(a)
        .unwrap();
    let limited = admission
        .authenticate(limited_token)
        .unwrap()
        .workspace_management(a)
        .unwrap();
    let script = Arc::new(Script::default());
    let runtime = runtime(script.clone());
    assert!(
        view.inspect_entity_identity(a_entity, b, b_entity, None)
            .unwrap()
            .is_none()
    );
    let source = evidence(&work, a_entity, a_contract);
    relationship(
        &peer,
        b_entity,
        b_contract,
        "attended",
        "attendee",
        "Alex Chen attended a briefing for Project Meridian Contract.",
    );
    let initial = work
        .compare_entity_identity(a_entity, b, b_entity, &runtime)
        .unwrap();
    assert_eq!(initial.revision, 1);
    assert_eq!(initial.outcome, IdentityOutcome::Unresolved);
    assert_eq!(initial.reason, IdentityReason::Ambiguity);
    assert_eq!(script.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        view.compare_entity_identity(a_entity, b, b_entity, &runtime)
            .unwrap_err()
            .kind(),
        ProductErrorKind::Unauthorized
    );
    assert_eq!(
        limited
            .inspect_entity_identity(a_entity, b, b_entity, None)
            .unwrap_err()
            .kind(),
        ProductErrorKind::Unauthorized
    );
    assert_eq!(
        work.compare_entity_identity(a_entity, b, b_entity, &runtime)
            .unwrap(),
        initial
    );
    assert_eq!(script.calls.load(Ordering::SeqCst), 1);
    let mut processing = work.start_entity_identity_processing(&runtime).unwrap();
    evidence(&peer, b_entity, b_contract);
    wait_revision(&work, a_entity, b, b_entity, 2);
    let positive = view
        .inspect_entity_identity(a_entity, b, b_entity, None)
        .unwrap()
        .unwrap();
    assert_eq!(positive.receipt.outcome, IdentityOutcome::SameIdentity);
    assert_eq!(positive.receipt.citations.len(), 2);
    assert_eq!(positive.cited_quotes.unwrap(), [QUOTE, QUOTE]);
    assert_eq!(script.calls.load(Ordering::SeqCst), 2);
    work.create_memory(
        "Unrelated".into(),
        "An unrelated publication.".into(),
        "fact".into(),
        "note".into(),
        30,
    )
    .unwrap();
    assert_eq!(
        work.compare_entity_identity(a_entity, b, b_entity, &runtime)
            .unwrap()
            .revision,
        2
    );
    processing.stop().unwrap();
    assert_eq!(script.calls.load(Ordering::SeqCst), 2);
    let cut = manager
        .semantic_graph_inspection_cut(a, OwnerKind::Rel)
        .unwrap();
    work.correct_memory_at_cut(
        cut,
        CorrectMemoryRequest {
            request_id: "correct-signatory".into(),
            memory_id: MemoryId(bytes(&source.id)),
            expected_revision: source.revision,
            expected_body_id: MemoryBodyId(bytes(&source.body_id)),
            title: "Corrected signatory".into(),
            content: "Morgan Lee signed Project Meridian Contract.".into(),
            created_at_ns: 40,
        },
    )
    .unwrap();
    work.quiesce().unwrap();
    work.reopen().unwrap();
    let stale = work
        .inspect_entity_identity(a_entity, b, b_entity, None)
        .unwrap()
        .unwrap();
    assert_eq!(stale.receipt.revision, 2);
    assert_eq!(stale.state, IdentityComparisonState::Pending);
    let mut processing = work.start_entity_identity_processing(&runtime).unwrap();
    wait_revision(&work, a_entity, b, b_entity, 3);
    processing.stop().unwrap();
    let current = work
        .inspect_entity_identity(a_entity, b, b_entity, None)
        .unwrap()
        .unwrap();
    assert_eq!(current.receipt.outcome, IdentityOutcome::Unresolved);
    assert_eq!(script.calls.load(Ordering::SeqCst), 2);
    let historical = view
        .inspect_entity_identity(a_entity, b, b_entity, Some(2))
        .unwrap()
        .unwrap();
    assert_eq!(historical.state, IdentityComparisonState::Historical);
    assert_eq!(historical.receipt.outcome, IdentityOutcome::SameIdentity);
    assert_eq!(historical.cited_quotes.unwrap(), [QUOTE, QUOTE]);
}

#[test]
fn identity_paused_evaluation_rechecks_peer_and_revocation() {
    #[derive(Clone, Copy)]
    enum DuringEvaluation {
        CorrectSource,
        Revoke,
        UnrelatedWrite,
    }
    for change in [
        DuringEvaluation::CorrectSource,
        DuringEvaluation::Revoke,
        DuringEvaluation::UnrelatedWrite,
    ] {
        let dir = Directory::new();
        let ap = dir.0.join("a.rel");
        let bp = dir.0.join("b.rel");
        let (ae, ac) = seed(&ap, [3; 16]);
        let (be, bc) = seed(&bp, [4; 16]);
        let ar = Rel::open(&ap).unwrap();
        let br = Rel::open(&bp).unwrap();
        let gateway = Gateway::new();
        let a = gateway.mount_rel(&ar, true).unwrap();
        let b = gateway.mount_rel(&br, true).unwrap();
        let token = "identity-race-manager-credential-00000000000";
        let (admission, control) = TrustedHostAdmission::new([HostCredentialBinding::new(
            "author",
            token,
            [OwnerGrant::ManageRel(a), OwnerGrant::ManageRel(b)],
        )])
        .install(&gateway)
        .unwrap();
        let manager = admission.authenticate(token).unwrap();
        let work = manager.workspace_management(a).unwrap();
        let peer = manager.workspace_management(b).unwrap();
        evidence(&work, ae, ac);
        let source = evidence(&peer, be, bc);
        let script = Arc::new(Script::default());
        let runtime = runtime(script.clone());
        script.pause();
        let result = std::thread::scope(|scope| {
            let pending = scope.spawn(|| work.compare_entity_identity(ae, b, be, &runtime));
            script.entered(1);
            match change {
                DuringEvaluation::Revoke => control.revoke(token).unwrap(),
                DuringEvaluation::UnrelatedWrite => {
                    peer.create_memory(
                        "Weather".into(),
                        "It rained.".into(),
                        "fact".into(),
                        "note".into(),
                        40,
                    )
                    .unwrap();
                }
                DuringEvaluation::CorrectSource => {
                    let cut = manager
                        .semantic_graph_inspection_cut(b, OwnerKind::Rel)
                        .unwrap();
                    peer.correct_memory_at_cut(
                        cut,
                        CorrectMemoryRequest {
                            request_id: "race-correct".into(),
                            memory_id: MemoryId(bytes(&source.id)),
                            expected_revision: source.revision,
                            expected_body_id: MemoryBodyId(bytes(&source.body_id)),
                            title: "Corrected signatory".into(),
                            content: "Morgan Lee signed Project Meridian Contract.".into(),
                            created_at_ns: 40,
                        },
                    )
                    .unwrap();
                }
            }
            script.release();
            pending.join().unwrap()
        });
        match change {
            DuringEvaluation::Revoke => {
                assert_eq!(result.unwrap_err().kind(), ProductErrorKind::Revoked)
            }
            DuringEvaluation::CorrectSource => {
                assert_eq!(result.unwrap_err().kind(), ProductErrorKind::Stale)
            }
            DuringEvaluation::UnrelatedWrite => {
                assert_eq!(result.unwrap().outcome, IdentityOutcome::SameIdentity)
            }
        }
        assert_eq!(script.calls.load(Ordering::SeqCst), 1);
        // A fresh reader checks the durable projection even after the original credential is revoked.
        drop((peer, work, manager, admission, control, gateway, ar, br));
        let gateway = Gateway::new();
        let ar = Rel::open(&ap).unwrap();
        let br = Rel::open(&bp).unwrap();
        let a = gateway.mount_rel(&ar, false).unwrap();
        let b = gateway.mount_rel(&br, false).unwrap();
        let fresh = "identity-race-fresh-reader-credential-00000000";
        let (admission, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
            "fresh",
            fresh,
            [OwnerGrant::ViewRel(a), OwnerGrant::ViewRel(b)],
        )])
        .install(&gateway)
        .unwrap();
        let retained = admission
            .authenticate(fresh)
            .unwrap()
            .workspace_management(a)
            .unwrap()
            .inspect_entity_identity(ae, b, be, None)
            .unwrap();
        assert_eq!(
            retained.map(|view| view.receipt.revision),
            match change {
                DuringEvaluation::UnrelatedWrite => Some(1),
                _ => None,
            }
        );
    }
}

#[test]
fn identity_readonly_peer_and_equivalent_concurrent_requests_publish_once() {
    let dir = Directory::new();
    let ap = dir.0.join("a.rel");
    let bp = dir.0.join("b.rel");
    let (ae, ac) = seed(&ap, [5; 16]);
    let (be, bc) = seed(&bp, [6; 16]);
    // Author the independent witnesses before mounting the peer read-only.
    {
        let gateway = Gateway::new();
        let ar = Rel::open(&ap).unwrap();
        let br = Rel::open(&bp).unwrap();
        let a = gateway.mount_rel(&ar, true).unwrap();
        let b = gateway.mount_rel(&br, true).unwrap();
        let token = "identity-seeding-manager-credential-000000000";
        let (admission, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
            "author",
            token,
            [OwnerGrant::ManageRel(a), OwnerGrant::ManageRel(b)],
        )])
        .install(&gateway)
        .unwrap();
        let manager = admission.authenticate(token).unwrap();
        evidence(&manager.workspace_management(a).unwrap(), ae, ac);
        evidence(&manager.workspace_management(b).unwrap(), be, bc);
    }
    let peer_before = fs::read(&bp).unwrap();
    {
        let gateway = Gateway::new();
        let ar = Rel::open(&ap).unwrap();
        let br = Rel::open(&bp).unwrap();
        let a = gateway.mount_rel(&ar, true).unwrap();
        let b = gateway.mount_rel(&br, false).unwrap();
        let token = "identity-readonly-manager-credential-00000000";
        let (admission, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
            "author",
            token,
            [OwnerGrant::ManageRel(a), OwnerGrant::ViewRel(b)],
        )])
        .install(&gateway)
        .unwrap();
        let work = admission
            .authenticate(token)
            .unwrap()
            .workspace_management(a)
            .unwrap();
        let script = Arc::new(Script::default());
        let runtime = runtime(script.clone());
        script.pause();
        let (first, second) = std::thread::scope(|scope| {
            let one = scope.spawn(|| work.compare_entity_identity(ae, b, be, &runtime));
            script.entered(1);
            let two = scope.spawn(|| work.compare_entity_identity(ae, b, be, &runtime));
            script.entered(2);
            script.release();
            (one.join().unwrap().unwrap(), two.join().unwrap().unwrap())
        });
        assert_eq!(first.revision, 1);
        assert_eq!(second.revision, 1);
        assert_eq!(first.outcome, IdentityOutcome::SameIdentity);
        assert_eq!(second.outcome, IdentityOutcome::SameIdentity);
        assert!(
            work.inspect_entity_identity(ae, b, be, Some(2))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            work.compare_entity_identity(ae, b, be, &runtime)
                .unwrap()
                .revision,
            1
        );
        let person = work
            .inspect(&reliquary_memory::InspectionQuery::Entities, None, 32)
            .unwrap();
        assert!(person.items.iter().any(|item| matches!(item,
            reliquary_memory::InspectionItem::Entity(entity) if entity.id == ae)));
    }
    assert_eq!(
        fs::read(&bp).unwrap(),
        peer_before,
        "recording A must preserve read-only B bytes"
    );
}
