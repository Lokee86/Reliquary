use super::*;
mod ego_pair;
use crate::{
    InspectedObservation,
    ego_audit_handoff::{shared_ego_run, shared_ego_run_closed},
};
use reliquary_application::ego_pending::{EgoBatchKey, EgoPendingWork, EgoRunKind};
use reliquary_application::perception_workflow::capture_discovery;
use reliquary_archive::{Archive, EpisodeBoundary, EpisodeOrigin, TurnInput};
use reliquary_container::{Container, ReliquaryScope};
use reliquary_memory_core::MemoryDraft;
use reliquary_owner_session::model::{
    EntityResolutionReason, MemoryEntityMentionKey, MemoryEntityResolutionStatus, MemoryRecord,
    MemorySourceRef, MemoryTextField, NodeRef, RelationChange, RelationKind, RelationOrigin,
};
use reliquary_owner_session::observation::{
    ObservationAuditProgress, ObservationBatch, ObservationCitation, ObservationComparison,
    ObservationComparisonOutcome, ObservationPolicy, ObservationProfile, ObservationSource,
    ObservationStanding,
};
use reliquary_owner_session::{
    DreamMemoryCut, DreamRelationCut, EntityMentionCommit, PerceptionAuditAdvance,
    PerceptionAuditProgress,
};
use reliquary_switchboard::{
    ConfiguredRoutes, DecisionConsumer, DecisionEndpoint, DecisionEndpointError, EmbeddingEndpoint,
    EmbeddingEndpointError, EmbeddingMode, EmbeddingNormalization, GeneralEndpoint,
    GeneralEndpointError, HostedEmbeddingInputSemantics, HostedInferenceIdentity,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

const OWNER: [u8; 16] = [162; 16];
const AUDIT_PHY: [u8; 16] = [163; 16];
const CONVERSATION: &str = "Perception acceptance";
const OLD: &str = "A separate archive pilot limited inactive requests during an earlier trial.";
const BASE: &str = "Every Atlas purchase request is reviewed before payment is released.";
const PAIR: &str = "Mira directly supervises the Atlas purchasing desk.";
const LIMIT: &str = "The Atlas payment policy now exempts emergency purchases from prior review.";
const A: &str = "Atlas purchase requests are reviewed before payment.";
const B: &str = "The Atlas purchase procedure places review before payment.";
const C: &str = "Atlas payment review precedes the release step.";
const RELATIONAL: &str = "Mira supervises Atlas purchasing.";
const QUALIFIED: &str =
    "Atlas purchase requests other than emergency purchases are reviewed before payment.";
const B_QUALIFIED: &str =
    "The Atlas purchase procedure places review before payment for nonemergency purchases.";
const C_QUALIFIED: &str =
    "For nonemergency Atlas purchases, review precedes the payment release step.";

#[derive(Clone, Debug, Eq, PartialEq)]
enum Call {
    Discovered { proposals: usize, sampled: bool },
    CapturedMemoryTexts(Vec<String>),
    GraphFollowUp,
    Synthesized(String),
    Receptors,
    Verified(String),
    Compared { claim: String, text: String },
}

struct Script {
    phase: u8,
    calls: Arc<Mutex<Vec<Call>>>,
}

impl Script {
    fn record(&self, call: Call) {
        self.calls.lock().unwrap().push(call);
    }
}

impl GeneralEndpoint for Script {
    fn model(&self) -> &str {
        "perception-acceptance-general"
    }

    fn complete_json(
        &self,
        _: &str,
        payload: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        let input: Value = serde_json::from_str(payload).unwrap();
        Ok(match schema {
            "memory_web_audit_v3" => {
                if self.phase == 8 {
                    assert_eq!(input["active"], json!({"ego":false, "perception":true}));
                }
                let sources = input["sources"].as_array().unwrap();
                self.record(Call::CapturedMemoryTexts(
                    sources
                        .iter()
                        .filter(|row| row["kind"] == "memory")
                        .filter_map(|row| row["text"].as_str().map(str::to_owned))
                        .collect(),
                ));
                let slot = |kind, text| {
                    sources
                        .iter()
                        .find(|row| row["kind"] == kind && row["text"] == text)
                        .map(|row| row["slot"].clone())
                };
                if self.phase == 6 {
                    if input["phase"] == "initial" {
                        let anchor = sources
                            .iter()
                            .find(|row| row["field"] == "title" && row["eligible"]["ego"] == true)
                            .unwrap();
                        return Ok(json!({
                            "ego": {"candidates": [], "lookups": [format!("graph:{}", anchor["slot"])]},
                            "perception": {"candidates": [], "lookups": []},
                        }));
                    }
                    assert_eq!(input["active"], json!({"ego":true, "perception":false}));
                    let links = input["links"].as_array().unwrap();
                    assert_eq!(links.len(), 1);
                    assert_eq!(links[0]["kind"], "Topical");
                    let target = &links[0]["target_slot"];
                    assert!(
                        sources.iter().any(|row| {
                            row["slot"] == *target && row["mandatory"]["ego"] == false
                        })
                    );
                    self.record(Call::GraphFollowUp);
                    return Ok(json!({
                        "ego": {"candidates": [{"subject": "Connected note", "source_slots": [target]}], "lookups": []},
                        "perception": {"candidates": [], "lookups": []},
                    }));
                }
                let mut proposals = Vec::new();
                let mut propose = |text, evidence, participants: Value| {
                    if let Some(slot) = evidence {
                        proposals.push(json!({
                            "proposition": text,
                            "source_slots": [slot],
                            "participant_slots": participants,
                        }));
                    }
                };
                match self.phase {
                    1 => propose(A, slot("memory", BASE), json!([])),
                    2 => {
                        propose(B, slot("observation", A), json!([]));
                        let participants = input["participants"].as_array().unwrap();
                        if let (Some(mira), Some(atlas)) = (
                            participants.iter().find(|row| row["label"] == "Mira"),
                            participants.iter().find(|row| row["label"] == "Atlas"),
                        ) {
                            propose(
                                RELATIONAL,
                                slot("memory", PAIR),
                                json!([mira["slot"], atlas["slot"]]),
                            );
                        }
                    }
                    3 => propose(C, slot("observation", B), json!([])),
                    _ => {}
                }
                self.record(Call::Discovered {
                    proposals: proposals.len(),
                    sampled: sources.iter().any(|row| {
                        row["text"] == OLD
                            && row["grade"] == "stale"
                            && row["eligible"] == json!({"ego":false,"perception":true})
                    }),
                });
                json!({
                    "ego":{"candidates":[],"lookups":[]},
                    "perception":{"candidates":proposals,"lookups":[]},
                })
            }
            "observation_synthesis_v2" => {
                let proposition = input["proposal"]["proposition"].as_str().unwrap();
                self.record(Call::Synthesized(proposition.into()));
                let value = if self.phase <= 3 {
                    let participants = if proposition == RELATIONAL {
                        let sources = input["source_slots"].as_array().unwrap();
                        assert!(sources.iter().any(|row| row["text"] == PAIR));
                        let slots = input["proposal_participants"].as_array().unwrap();
                        let offered = input["participant_slots"].as_array().unwrap();
                        assert_eq!(
                            offered[slots[0].as_u64().unwrap() as usize]["label"],
                            "Mira"
                        );
                        assert_eq!(
                            offered[slots[1].as_u64().unwrap() as usize]["label"],
                            "Atlas"
                        );
                        json!([
                            {"participant_slot":slots[0],"role":"supervisor"},
                            {"participant_slot":slots[1],"role":"purchasing desk"}
                        ])
                    } else {
                        json!([])
                    };
                    draft(proposition, input["proposal_sources"].clone(), participants)
                } else if matches!(proposition, A | B | C) {
                    let (successor, required_text) = match proposition {
                        A => (QUALIFIED, &[BASE, LIMIT][..]),
                        B => (B_QUALIFIED, &[QUALIFIED][..]),
                        C => (C_QUALIFIED, &[B_QUALIFIED][..]),
                        _ => unreachable!(),
                    };
                    let evidence = input["source_slots"].as_array().unwrap();
                    let supporting = required_text
                        .iter()
                        .filter_map(|text| evidence.iter().find(|row| row["text"] == *text))
                        .map(|row| row["slot"].clone())
                        .collect::<Vec<_>>();
                    if supporting.len() != required_text.len() {
                        return Err(GeneralEndpointError::InvalidResponse(
                            "missing supported review sources",
                        ));
                    }
                    draft(successor, json!(supporting), json!([]))
                } else {
                    Value::Null
                };
                json!({"draft":value,"duplicate_slot":null})
            }
            "observation_receptors_v1" => {
                self.record(Call::Receptors);
                json!({
                    "support":["Atlas proof of prior payment review"],
                    "challenge":["Atlas evidence against prior payment review"],
                    "qualification":["Atlas payment review exceptions"],
                    "other":["Atlas updates to the payment process"],
                })
            }
            _ => {
                return Err(GeneralEndpointError::InvalidResponse(
                    "unexpected acceptance schema",
                ));
            }
        })
    }
}

fn draft(proposition: &str, sources: Value, participants: Value) -> Value {
    json!({
        "proposition":proposition,
        "status":"observed",
        "time_scope":"current",
        "source_slots":sources,
        "participants":participants,
    })
}

impl DecisionEndpoint for Script {
    fn model(&self) -> &str {
        "typesafe/jev-1.13"
    }

    fn evaluate(&self, state: &Value, questions: &Value) -> Result<Value, DecisionEndpointError> {
        let questions = questions.as_object().unwrap();
        let mut answers = Map::new();
        if let Some(claim) = state["existing_observation"].as_str() {
            let hits = state["evidence_hits"].as_object().unwrap();
            for (hit, row) in hits {
                let text = row.as_str().unwrap();
                self.record(Call::Compared {
                    claim: claim.into(),
                    text: text.into(),
                });
                let verdict = if claim == A && text == LIMIT {
                    "material_qualification"
                } else {
                    "no_material_effect"
                };
                answers.insert(hit.clone(), choice(verdict));
                answers.insert(format!("{hit}-time"), choice("change_to_ongoing_claim"));
                answers.insert(format!("{hit}-scope"), choice("overlapping_scope"));
                assert!(questions.contains_key(hit));
            }
        } else {
            let claim = state["claim"].as_str().unwrap();
            self.record(Call::Verified(claim.into()));
            let mut needs_revision = false;
            for row in state["backcheck_hits"].as_array().unwrap() {
                let qualification = claim == A && row["text"] == LIMIT;
                needs_revision |= qualification;
                answers.insert(
                    format!("hit-{}", row["id"].as_str().unwrap()),
                    choice(if qualification {
                        "material_qualification"
                    } else {
                        "no_material_effect"
                    }),
                );
            }
            answers.insert(
                "verify".into(),
                choice(if needs_revision {
                    "needs_revision"
                } else {
                    "stands_as_written"
                }),
            );
        }
        assert_eq!(answers.len(), questions.len());
        Ok(json!({"answers":answers}))
    }
}

fn choice(value: &str) -> Value {
    json!({"type":"choice","choice":value})
}

impl EmbeddingEndpoint for Script {
    fn model(&self) -> Option<&str> {
        Some("perception-acceptance-embedding")
    }

    fn dimensions(&self) -> u32 {
        2
    }

    fn normalization(&self) -> EmbeddingNormalization {
        EmbeddingNormalization::L2
    }

    fn supports_bounded_embedding(&self) -> bool {
        true
    }

    fn embed_bounded(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
        _: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.embed(mode, inputs)
    }

    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        Ok(inputs
            .iter()
            .map(|text| {
                if mode == EmbeddingMode::Query || text == OLD || text == LIMIT {
                    vec![1.0, 0.0]
                } else {
                    vec![0.0, 1.0]
                }
            })
            .collect())
    }
}

struct Fixture {
    path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!("perception-{}.rel", Uuid::new_v4()));
        drop(RelWriter::create_path(&path, ReliquaryScope::Project, OWNER).unwrap());
        Self { path }
    }

    fn append(&self, sources: &[(String, &str)], activity: usize) -> Vec<MemoryRecord> {
        let mut drafts = Vec::new();
        {
            let mut container = Container::open_writer(&self.path).unwrap();
            let mut archive = Archive::open(&container).unwrap();
            for number in 0..activity {
                accept(
                    &mut archive,
                    &mut container,
                    &format!("activity-{number}"),
                    "Unrelated accepted Archive activity.",
                );
            }
            for (name, content) in sources {
                let at = accept(&mut archive, &mut container, name, content);
                let episode = archive
                    .put_episode(
                        &mut container,
                        CONVERSATION,
                        name,
                        name,
                        EpisodeOrigin::Live,
                        EpisodeBoundary::Explicit,
                        at as i64,
                    )
                    .unwrap();
                drafts.push((
                    MemoryDraft {
                        title: name.clone(),
                        content: (*content).into(),
                        category: "fact".into(),
                        memory_type: "semantic".into(),
                        authority_kind: "direct".into(),
                        temporal_status: "current".into(),
                        scope: "project".into(),
                        lifecycle_state: "knowledge".into(),
                        archived: false,
                        parent_id: None,
                        superseded_by: None,
                        source_node_id: Some(name.clone()),
                        content_source_conversation_id: Some(CONVERSATION.into()),
                        content_source_node_id: Some(name.clone()),
                        grounding_source_conversation_id: None,
                        grounding_source_node_id: None,
                        source_episode_id: None,
                        source_ref: Some(MemorySourceRef {
                            owner_id: "a2".repeat(16),
                            principal_id: Some("alice".into()),
                            source_episode_id: episode.0,
                            source_node_id: name.clone(),
                            content_source_conversation_id: Some(CONVERSATION.into()),
                            content_source_node_id: Some(name.clone()),
                            grounding_source_conversation_id: None,
                            grounding_source_node_id: None,
                        }),
                        source_time_ns: Some(at as i64),
                        temporal_inference: None,
                        mutation_id: format!("perception-source-{name}"),
                        created_at_ns: at as i64,
                        updated_at_ns: at as i64,
                    },
                    at,
                ));
            }
            container.sync().unwrap();
        }
        let mut writer = RelWriter::open_path_current(&self.path).unwrap();
        let settled_at = writer.read_session().rel_turn_count();
        let mut written = Vec::with_capacity(drafts.len());
        // An explicit first-cycle Begin is admitted at the source's native
        // Memory birth cut. Complete it before creating the next source.
        for (draft, born) in drafts {
            let row = writer.publish_direct_memory("alice", draft).unwrap().record;
            let pass = format!("initial-{}", row.mutation_id);
            let proof = DreamMemoryCut {
                id: row.id,
                revision: row.revision,
                body_id: row.body_id,
            };
            writer
                .begin_first_empty_dream_pass(pass.clone(), proof, born)
                .unwrap();
            writer
                .commit_empty_dream_pass(&pass, proof, 0, settled_at)
                .unwrap();
            written.push(row);
        }
        for row in &written {
            if row.mutation_id == "perception-source-relational-source" {
                for name in ["Mira", "Atlas"] {
                    let start = PAIR.find(name).unwrap() as u32;
                    writer
                        .commit_entity_mention(EntityMentionCommit {
                            owner_id: OWNER,
                            memory_revision: row.revision,
                            body_id: row.body_id,
                            key: MemoryEntityMentionKey {
                                memory_id: row.id.0,
                                field: MemoryTextField::Content,
                                start_byte: start,
                                end_byte: start + name.len() as u32,
                            },
                            surface: name.into(),
                            expected_revision: 0,
                            updated_at_ns: row.updated_at_ns,
                            status: MemoryEntityResolutionStatus::Unresolved {
                                reason: EntityResolutionReason::NamedReferent,
                            },
                            target_revisions: Vec::new(),
                            new_entity: Some(("participant".into(), name.into())),
                        })
                        .unwrap();
                }
            }
        }
        written
    }

    fn install_embedding_profile(&self) -> CompatibilityProfileId {
        let endpoint = reliquary_application::SwitchboardEmbeddingEndpoint::new(Arc::new(Script {
            phase: 0,
            calls: Arc::new(Mutex::new(Vec::new())),
        }));
        let profile = reliquary_application::prepare_compatibility_profile(&endpoint).unwrap();
        RelWriter::open_path_current(&self.path)
            .unwrap()
            .publish_compatibility_profile(profile)
            .unwrap()
    }

    fn mount(&self) -> (Gateway, OwnerId, Arc<Instance>) {
        let gateway = Gateway::new();
        let owner = gateway
            .mount_writable_reliquary_current(&self.path)
            .unwrap();
        let instance = Arc::new(gateway.instance("alice", [owner], []).unwrap());
        (gateway, owner, instance)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(self.path.with_extension("phy"));
        let pending = self.path.with_extension("sqlite");
        let _ = fs::remove_file(&pending);
        let _ = fs::remove_dir(pending);
    }
}

fn accept(archive: &mut Archive, container: &mut Container, id: &str, text: &str) -> u64 {
    let position = archive.rel_turn_count() + 1;
    archive
        .accept_turn(
            container,
            TurnInput {
                id: id.into(),
                conversation_id: CONVERSATION.into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("alice".into()),
                timestamp_ns: position as i64,
                content: text.into(),
                attachments: Vec::new(),
                project_attachments: Vec::new(),
            },
        )
        .unwrap();
    position
}

fn audit_idle(rel: &RelSession) -> bool {
    rel.observation_progress().is_some_and(|row| {
        let mut progress = PerceptionAuditProgress::decode(&row.bytes).unwrap();
        progress.advance(rel).unwrap() == PerceptionAuditAdvance::Idle
    })
}

fn run(
    fixture: &Fixture,
    phase: u8,
    count: usize,
    calls: &Arc<Mutex<Vec<Call>>>,
    complete: impl Fn(&[InspectedObservation]) -> bool,
) -> Vec<InspectedObservation> {
    let (gateway, owner, mut instance) = fixture.mount();
    let mut registration = None;
    if matches!(phase, 6..=8) {
        let phy_path = fixture.path.with_extension("phy");
        drop(PhyWriter::create_path(&phy_path, AUDIT_PHY).unwrap());
        let phy = gateway
            .mount_writable_phylactery_current(&phy_path)
            .unwrap();
        let pending = fixture.path.with_extension("sqlite");
        if phase == 7 {
            fs::create_dir(&pending).unwrap();
        }
        assert!(matches!(
            gateway
                .ego_audit_registry
                .admit(&instance, phy, &[owner], &pending, [0; 32]),
            Err(GatewayError::UnauthorizedPhy { .. })
        ));
        instance = Arc::new(gateway.instance("alice", [owner], [phy]).unwrap());
        registration = Some(
            gateway
                .ego_audit_registry
                .admit(&instance, phy, &[owner], &pending, [0; 32])
                .unwrap(),
        );
    }
    let script = Arc::new(Script {
        phase,
        calls: calls.clone(),
    });
    let hosted = HostedInferenceIdentity::new(
        "fixture",
        GeneralEndpoint::model(script.as_ref()),
        "fixture",
        EmbeddingEndpoint::model(script.as_ref()).unwrap(),
        script.dimensions(),
        HostedEmbeddingInputSemantics::Shared,
        script.normalization(),
    )
    .unwrap();
    let routes = ConfiguredRoutes::hosted(script.clone(), script.clone(), hosted)
        .unwrap()
        .with_native_perception_decision(script, "fixture/native-jev")
        .unwrap();
    let identity = routes.processing_route_identity();
    if phase == 8 {
        // Simulate termination after the SQLite commit but before the REL has
        // even published its page acknowledgment.
        let (run, key) = instance
            .with_reliquary(owner, |rel| {
                let mut progress = PerceptionAuditProgress::new(rel.perception_source_cut());
                assert_eq!(
                    progress.advance(rel).unwrap(),
                    PerceptionAuditAdvance::Updated
                );
                let page = progress.page.unwrap();
                let capture = capture_discovery(rel, &page, identity.perception).unwrap();
                let run = shared_ego_run(&page.cut, true, identity.perception);
                (
                    reliquary_application::ego_pending::EgoRun {
                        phy: AUDIT_PHY,
                        ..run
                    },
                    EgoBatchKey {
                        capture: capture.key,
                        ordinal: 0,
                    },
                )
            })
            .unwrap();
        EgoPendingWork::open(&fixture.path.with_extension("sqlite"))
            .unwrap()
            .accept_batch(run, key, &[])
            .unwrap();
    }
    let lifecycle = gateway.perception_lifecycle();
    lifecycle
        .set_routes(Some(PerceptionRoutes {
            general: routes.general_endpoint().unwrap(),
            contribution: routes
                .decision_endpoint(DecisionConsumer::ObservationContribution)
                .unwrap(),
            verification: routes
                .decision_endpoint(DecisionConsumer::ObservationVerification)
                .unwrap(),
            embedding: routes.embedding_endpoint().unwrap(),
            identity: identity.perception,
            embedding_identity: identity.perception_embedding,
        }))
        .unwrap();
    lifecycle.admit(&instance, vec![owner]).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let items = loop {
        let items = instance.list_observations(owner, None, 16).unwrap().items;
        if items.len() == count
            && complete(&items)
            && instance
                .with_reliquary(owner, |rel| {
                    if phase == 7 {
                        return rel.observation_progress().is_some_and(|row| {
                            let progress = PerceptionAuditProgress::decode(&row.bytes).unwrap();
                            progress
                                .page
                                .is_some_and(|page| page.perception_accepted && !page.ego_accepted)
                        });
                    }
                    audit_idle(rel)
                        && rel
                            .observation_claim_index_profiles()
                            .first()
                            .is_none_or(|profile| {
                                rel.observations_after(None, 16).iter().all(|head| {
                                    head.standing != ObservationStanding::Current
                                        || rel
                                            .observation_claim_vector(profile.id, head.revision)
                                            .unwrap()
                                            .is_some()
                                })
                            })
                })
                .unwrap()
        {
            break items;
        }
        assert!(
            Instant::now() < deadline,
            "Perception phase {phase} did not settle. Claims: {items:?}. Calls: {:?}",
            calls.lock().unwrap()
        );
        thread::sleep(Duration::from_millis(25));
    };
    drop(lifecycle);
    drop(registration);
    gateway.quiesce_owner(owner).unwrap();
    items
}

fn window(name: &str, first: Option<&'static str>) -> Vec<(String, &'static str)> {
    let mut rows = Vec::new();
    if let Some(content) = first {
        rows.push(("relational-source".into(), content));
    }
    rows.extend(
        (rows.len()..25).map(|index| (format!("{name}-{index}"), "A routine administrative note.")),
    );
    rows
}

fn claim<'a>(items: &'a [InspectedObservation], proposition: &str) -> &'a InspectedObservation {
    items
        .iter()
        .find(|item| item.claim.proposition == proposition)
        .unwrap()
}

fn observation_id(cite: ObservationCitation) -> ObservationId {
    let ObservationSource::Observation(id) = cite.source else {
        panic!("expected an Observation reference");
    };
    id
}

#[test]
fn mounted_worker_publishes_ordinary_relational_and_derived_claims_and_reviews_new_evidence() {
    let fixture = Fixture::new();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let old = fixture
        .append(&[("older-evidence".into(), OLD)], 0)
        .remove(0);
    fixture.append(&[("atlas-policy".into(), BASE)], 1050);
    let first = run(&fixture, 1, 1, &calls, |_| true);
    let a = claim(&first, A);
    assert_eq!(a.standing, ObservationStanding::Current);
    assert_eq!(a.claim.profile, ObservationProfile::Ordinary);
    assert_eq!(a.claim.lineage.len(), 1);

    let profile = fixture.install_embedding_profile();
    let previous = RelSession::open_path_current(&fixture.path).unwrap();
    assert!(previous.observation_claim_profiles().next().is_none());
    drop(previous);

    fixture.append(&window("second-group", Some(PAIR)), 0);
    let second = run(&fixture, 2, 3, &calls, |_| true);
    let b = claim(&second, B);
    assert_eq!(b.claim.support, vec![a.claim.citation]);
    assert_eq!(b.claim.lineage, a.claim.lineage);
    let indexed = RelSession::open_path_current(&fixture.path).unwrap();
    for claim in [a, b] {
        let head = indexed
            .observation(observation_id(claim.claim.citation))
            .unwrap();
        assert_eq!(
            indexed
                .observation_claim_vector(profile, head.revision)
                .unwrap(),
            Some(vec![0.0, 1.0])
        );
    }
    drop(indexed);
    let relational = claim(&second, RELATIONAL);
    assert!(
        matches!(&relational.claim.profile, ObservationProfile::Relational(participants)
        if participants.len() == 2
            && participants.iter().all(|p| p.revision == 1)
            && participants.iter().any(|p| p.role.as_deref() == Some("supervisor"))
            && participants.iter().any(|p| p.role.as_deref() == Some("purchasing desk")))
    );

    fixture.append(&window("third-group", None), 0);
    let third = run(&fixture, 3, 4, &calls, |_| true);
    let c = claim(&third, C);
    assert_eq!(c.claim.support, vec![b.claim.citation]);
    assert_eq!(c.claim.lineage, a.claim.lineage);
    assert!(
        third
            .iter()
            .all(|item| item.standing == ObservationStanding::Current)
    );

    fixture.append(&window("empty-discovery-group", None), 0);
    let before_empty = calls.lock().unwrap().len();
    assert_eq!(run(&fixture, 4, 4, &calls, |_| true), third);
    assert!(calls.lock().unwrap()[before_empty..].iter().any(|call| {
        matches!(
            call,
            Call::Discovered {
                proposals: 0,
                sampled: true
            }
        )
    }));

    let a_id = observation_id(a.claim.citation);
    let sampled = ObservationCitation {
        source: ObservationSource::Memory(old.id.0),
        revision: old.revision,
        body_id: old.body_id.0,
        start_byte: 0,
        end_byte: OLD.len() as u32,
        ..a.claim.citation
    };
    let reopened = RelSession::open_path_current(&fixture.path).unwrap();
    for id in [a_id, observation_id(c.claim.citation)] {
        assert_eq!(
            reopened.observation_comparison(&ObservationComparison {
                id,
                target_revision: 1,
                citation: sampled,
                outcome: ObservationComparisonOutcome::NoMaterialEffect,
            }),
            Some(ObservationComparisonOutcome::NoMaterialEffect)
        );
        let older_activity = reopened.perception_source_activity(&sampled).unwrap();
        assert_eq!(reopened.observation_award(id, older_activity), None);
    }
    drop(reopened);

    let accepted = calls.lock().unwrap().clone();
    assert!(
        accepted
            .iter()
            .any(|call| matches!(call, Call::Discovered { sampled: true, .. }))
    );
    assert_eq!(
        accepted
            .iter()
            .filter(
                |call| matches!(call, Call::Compared { claim, text } if claim == A && text == OLD)
            )
            .count(),
        1
    );
    assert_eq!(
        accepted
            .iter()
            .filter(|call| matches!(call, Call::Receptors))
            .count(),
        4
    );
    for proposition in [A, B, C, RELATIONAL] {
        assert_eq!(
            accepted
                .iter()
                .filter(|call| matches!(call, Call::Verified(claim) if claim == proposition))
                .count(),
            1
        );
    }

    let before_review = calls.lock().unwrap().len();
    let qualification_source = fixture
        .append(&[("atlas-qualification".into(), LIMIT)], 0)
        .remove(0);
    let reviewed = run(&fixture, 5, 4, &calls, |items| {
        [QUALIFIED, B_QUALIFIED, C_QUALIFIED]
            .iter()
            .all(|expected| {
                items.iter().any(|item| {
                    item.claim.proposition == *expected
                        && item.standing == ObservationStanding::Current
                })
            })
    });
    let review_calls = calls.lock().unwrap()[before_review..].to_vec();
    assert!(
        !review_calls
            .iter()
            .any(|call| matches!(call, Call::Discovered { .. }))
    );
    assert_eq!(
        review_calls
            .iter()
            .filter(|call| {
                matches!(call, Call::Compared { claim, text } if claim == A && text == LIMIT)
            })
            .count(),
        1
    );
    let latest = claim(&reviewed, QUALIFIED);
    assert_eq!(observation_id(latest.claim.citation), a_id);
    assert_eq!(latest.predecessor, Some(1));
    assert_eq!(latest.claim.citation.revision, 2);
    let b_successor = claim(&reviewed, B_QUALIFIED);
    let c_successor = claim(&reviewed, C_QUALIFIED);
    assert_eq!(
        observation_id(b_successor.claim.citation),
        observation_id(b.claim.citation)
    );
    assert_eq!(
        observation_id(c_successor.claim.citation),
        observation_id(c.claim.citation)
    );
    assert_eq!(b_successor.claim.support, vec![latest.claim.citation]);
    assert_eq!(c_successor.claim.support, vec![b_successor.claim.citation]);
    assert_eq!(c_successor.claim.lineage, latest.claim.lineage);
    let qualification = latest
        .claim
        .support
        .iter()
        .find(|cite| {
            cite.source == ObservationSource::Memory(qualification_source.id.0)
                && cite.body_id == qualification_source.body_id.0
        })
        .copied()
        .expect("the verified successor cites the qualifying source");

    let reopened = RelSession::open_path_current(&fixture.path).unwrap();
    assert_eq!(
        reopened.observation_comparison(&ObservationComparison {
            id: a_id,
            target_revision: 1,
            citation: qualification,
            outcome: ObservationComparisonOutcome::MaterialQualification,
        }),
        Some(ObservationComparisonOutcome::MaterialQualification)
    );
    let activity = reopened.perception_source_activity(&qualification).unwrap();
    assert_eq!(reopened.observation_award(a_id, activity), Some(25));
    let historical = reopened.observation_revision(a_id, 1).unwrap();
    assert_eq!(historical.standing, ObservationStanding::Superseded);
    assert_eq!(historical.revision.draft.proposition, A);
    assert_eq!(
        reopened
            .observation(a_id)
            .unwrap()
            .revision
            .draft
            .proposition,
        QUALIFIED
    );
}

#[test]
fn bootstrap_continues_after_first_bounded_memory_page() {
    let fixture = Fixture::new();
    let sources = (0..130)
        .map(|index| {
            (
                format!("bootstrap-memory-{index:03}"),
                "A durable project note for bootstrap coverage.",
            )
        })
        .collect::<Vec<_>>();
    fixture.append(&sources, 0);

    let calls = Arc::new(Mutex::new(Vec::new()));
    assert!(run(&fixture, 0, 0, &calls, |_| true).is_empty());

    let captured = captured_audit_memory_texts(&calls);
    for (title, _) in &sources {
        assert!(
            captured.contains(title),
            "bootstrap never exposed {title} to discovery"
        );
    }
    if let Some(path) = std::env::var_os("RELIQUARY_TEST_AUDITED_REL") {
        let mut source = fs::File::open(&fixture.path).unwrap();
        let mut copy = fs::File::create_new(path).unwrap();
        std::io::copy(&mut source, &mut copy).unwrap();
        copy.sync_all().unwrap();
    }

    let additions = (0..25)
        .map(|index| {
            (
                format!("recurring-memory-{index:03}"),
                "A new settled note for the recurring audit.",
            )
        })
        .collect::<Vec<_>>();
    fixture.append(&additions, 0);
    calls.lock().unwrap().clear();
    run(&fixture, 0, 0, &calls, |_| true);
    let captured = captured_audit_memory_texts(&calls);
    assert!(sources.iter().all(|(title, _)| !captured.contains(title)));
    assert!(additions.iter().all(|(title, _)| captured.contains(title)));
}

#[test]
fn bootstrap_resumes_past_an_accepted_version_one_page() {
    let fixture = Fixture::new();
    let sources = (0..130)
        .map(|index| {
            (
                format!("bootstrap-memory-{index:03}"),
                "A durable project note for bootstrap coverage.",
            )
        })
        .collect::<Vec<_>>();
    let mut memories = fixture.append(&sources, 0);
    memories.sort_by_key(|memory| memory.id);

    let mut writer = RelWriter::open_path_current(&fixture.path).unwrap();
    let graph_version = writer.read_session().graph_version();
    let source = &memories[125];
    let target = &memories[0];
    let dream_cut = |row: &MemoryRecord| DreamMemoryCut {
        id: row.id,
        revision: row.revision,
        body_id: row.body_id,
    };
    writer
        .commit_dream_grouped_relation(DreamRelationCut {
            owner_id: OWNER,
            graph_version,
            source: dream_cut(source),
            candidate: dream_cut(target),
            change: RelationChange {
                source: NodeRef::Memory(source.id.0),
                target: NodeRef::Memory(target.id.0),
                kind: RelationKind::Topical,
                active: true,
                origin: RelationOrigin::Dream,
            },
        })
        .unwrap();
    let cut = writer.read_session().perception_source_cut();
    let mut progress = PerceptionAuditProgress::new(cut.clone());
    let publish_version_one = |writer: &mut RelWriter, progress: Value| {
        assert_eq!(progress["version"], 1);
        let progress: PerceptionAuditProgress = serde_json::from_value(progress).unwrap();
        let bytes = progress.encode().unwrap();
        let fingerprint = Sha256::digest(&bytes).into();
        let previous = writer
            .read_session()
            .observation_progress()
            .map(|row| row.fingerprint);
        writer
            .publish_observation_batch(ObservationBatch {
                owner_id: cut.owner_id,
                history_generation: cut.history_generation,
                input_fingerprint: fingerprint,
                policy: ObservationPolicy {
                    version: 1,
                    general_route: [1; 32],
                    decision_route: [1; 32],
                },
                operations: Vec::new(),
                progress: Some(ObservationAuditProgress {
                    previous,
                    fingerprint,
                    bytes,
                }),
            })
            .unwrap();
    };
    publish_version_one(&mut writer, json!(progress));
    assert_eq!(
        progress.advance(writer.read_session()).unwrap(),
        PerceptionAuditAdvance::Updated
    );
    assert_eq!(progress.page.as_ref().unwrap().sources.len(), 125);
    let mut legacy = json!(progress);
    legacy["bootstrap_cut"] = Value::Null;
    legacy["bootstrap_after"] = Value::Null;
    publish_version_one(&mut writer, legacy.clone());
    legacy["page"] = Value::Null;
    publish_version_one(&mut writer, legacy);
    drop(writer);

    let calls = Arc::new(Mutex::new(Vec::new()));
    run(&fixture, 6, 0, &calls, |_| true);
    assert!(calls.lock().unwrap().contains(&Call::GraphFollowUp));
    let rel = RelSession::open_path_current(&fixture.path).unwrap();
    let mut pending = EgoPendingWork::open(&fixture.path.with_extension("sqlite")).unwrap();
    let runs = pending.pending_runs(AUDIT_PHY, OWNER).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].kind, EgoRunKind::RelSharedBootstrap);
    assert!(shared_ego_run_closed(&rel, runs[0]).unwrap());
    let batch = pending.next_batch(runs[0], 0).unwrap().unwrap();
    assert_eq!(batch.candidates[0].subject, "Connected note");
    assert_eq!(batch.candidates[0].sources[0].body_id, target.body_id.0);
    pending.seal(runs[0]).unwrap();
    assert_eq!(pending.ready_run(AUDIT_PHY, OWNER).unwrap(), Some(runs[0]));
    let captured = captured_audit_memory_texts(&calls);
    for (index, memory) in memories.iter().enumerate() {
        let title = memory.source_node_id.as_ref().unwrap();
        assert_eq!(captured.contains(title), index >= 125, "{title}");
    }
}

fn captured_audit_memory_texts(calls: &Mutex<Vec<Call>>) -> BTreeSet<String> {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter_map(|call| match call {
            Call::CapturedMemoryTexts(texts) => Some(texts),
            _ => None,
        })
        .flatten()
        .cloned()
        .collect()
}

#[test]
fn perception_accepts_its_list_while_ego_storage_is_held() {
    let fixture = Fixture::new();
    fixture.append(&[("independent-list".into(), "A durable note.")], 0);
    let calls = Arc::new(Mutex::new(Vec::new()));
    run(&fixture, 7, 0, &calls, |_| true);
    let rel = RelSession::open_path_current(&fixture.path).unwrap();
    let row = rel.observation_progress().unwrap();
    let page = PerceptionAuditProgress::decode(&row.bytes)
        .unwrap()
        .page
        .unwrap();
    assert!(page.perception_accepted);
    assert!(!page.ego_accepted);
}

#[test]
fn shared_empty_batch_replays_before_owner_page_acknowledgment() {
    let fixture = Fixture::new();
    fixture.append(&[("durable-empty".into(), "A stand-alone note.")], 0);
    let calls = Arc::new(Mutex::new(Vec::new()));
    run(&fixture, 8, 0, &calls, |_| true);

    let rel = RelSession::open_path_current(&fixture.path).unwrap();
    let pending = EgoPendingWork::open(&fixture.path.with_extension("sqlite")).unwrap();
    let runs = pending.pending_runs(AUDIT_PHY, OWNER).unwrap();
    assert_eq!(runs.len(), 1);
    assert!(shared_ego_run_closed(&rel, runs[0]).unwrap());
    let batch = pending.next_batch(runs[0], 0).unwrap().unwrap();
    assert!(batch.candidates.is_empty());
    assert!(
        pending
            .next_batch(runs[0], batch.sequence)
            .unwrap()
            .is_none()
    );
}
