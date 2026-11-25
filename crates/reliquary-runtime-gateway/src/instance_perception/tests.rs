use super::*;
use crate::InspectedObservation;
use reliquary_archive::{Archive, EpisodeBoundary, EpisodeOrigin, TurnInput};
use reliquary_container::{Container, ReliquaryScope};
use reliquary_memory_core::MemoryDraft;
use reliquary_owner_session::model::{
    EntityResolutionReason, MemoryEntityMentionKey, MemoryEntityResolutionStatus, MemoryRecord,
    MemorySourceRef, MemoryTextField,
};
use reliquary_owner_session::observation::{
    ObservationCitation, ObservationComparison, ObservationComparisonOutcome, ObservationProfile,
    ObservationSource, ObservationStanding,
};
use reliquary_owner_session::{
    DreamMemoryCut, EntityMentionCommit, PerceptionAuditAdvance, PerceptionAuditProgress,
};
use reliquary_switchboard::{
    ConfiguredRoutes, DecisionConsumer, DecisionEndpoint, DecisionEndpointError, EmbeddingEndpoint,
    EmbeddingEndpointError, EmbeddingMode, EmbeddingNormalization, GeneralEndpoint,
    GeneralEndpointError, HostedEmbeddingInputSemantics, HostedInferenceIdentity,
};
use serde_json::{Map, Value, json};
use std::{fs, path::PathBuf};

const OWNER: [u8; 16] = [162; 16];
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
            "memory_web_audit_v1" => {
                let sources = input["sources"].as_array().unwrap();
                let slot = |kind, text| {
                    sources
                        .iter()
                        .find(|row| row["kind"] == kind && row["text"] == text)
                        .map(|row| row["slot"].clone())
                };
                let mut proposals = Vec::new();
                let mut propose = |text, evidence, participants| {
                    if let Some(slot) = evidence {
                        proposals.push(draft(text, json!([slot]), participants));
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
                                json!([
                                    {"participant_slot":mira["slot"],"role":"supervisor"},
                                    {"participant_slot":atlas["slot"],"role":"purchasing desk"}
                                ]),
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
            "observation_synthesis_v1" => {
                let proposition = input["proposal"]["proposition"].as_str().unwrap();
                self.record(Call::Synthesized(proposition.into()));
                let value = if self.phase <= 3 {
                    draft(
                        proposition,
                        input["proposal_sources"].clone(),
                        input["proposal_participants"].clone(),
                    )
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
    let (gateway, owner, instance) = fixture.mount();
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
            && instance.with_reliquary(owner, audit_idle).unwrap()
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

    fixture.append(&window("second-group", Some(PAIR)), 0);
    let second = run(&fixture, 2, 3, &calls, |_| true);
    let b = claim(&second, B);
    assert_eq!(b.claim.support, vec![a.claim.citation]);
    assert_eq!(b.claim.lineage, a.claim.lineage);
    let relational = claim(&second, RELATIONAL);
    assert!(
        matches!(&relational.claim.profile, ObservationProfile::Relational(participants)
        if participants.len() == 2 && participants.iter().all(|p| p.revision == 1))
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
