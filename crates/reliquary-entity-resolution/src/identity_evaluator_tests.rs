use super::*;
use reliquary_perception_store::{
    EntityId, MemoryTextField, RelationshipId,
    identity::{IdentityLimit, IdentityMemoryRef, IdentitySpan},
    identity_evidence::{IdentityEntityEvidence, IdentityParticipantEvidence},
};
use std::sync::atomic::{AtomicUsize, Ordering};

const CONTRACT: &str = "Project Meridian Contract";
const QUOTE: &str = "Alex Chen signed Project Meridian Contract.";

struct Scripted {
    calls: AtomicUsize,
    output: Value,
}

impl Scripted {
    fn reply(output: Value) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            output,
        }
    }
}

impl GeneralEndpoint for Scripted {
    fn model(&self) -> &str {
        "identity-fixture"
    }

    fn complete_json(
        &self,
        _system_prompt: &str,
        payload: &str,
        schema_name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        assert_eq!(schema_name, "entity_identity_comparison");
        assert!(payload.len() <= super::super::MAX_ENTITY_MODEL_INPUT_BYTES);
        Ok(self.output.clone())
    }
}

fn side(owner: u8, lineage: u8, quote: &str) -> IdentityEvidence {
    let endpoint = IdentityEntityEvidence {
        id: EntityId([1; 32]),
        revision: 1,
        canonical_name: "Alex Chen".into(),
        aliases: vec!["A. Chen".into()],
        kind: "person".into(),
    };
    IdentityEvidence {
        owner: [owner; 16],
        endpoint: endpoint.clone(),
        relationships: vec![IdentityRelationshipEvidence {
            id: RelationshipId([4; 32]),
            revision: 1,
            kind: "signs".into(),
            participants: vec![
                IdentityParticipantEvidence {
                    entity: endpoint,
                    roles: vec!["signatory".into()],
                },
                IdentityParticipantEvidence {
                    entity: IdentityEntityEvidence {
                        id: EntityId([2; 32]),
                        revision: 1,
                        canonical_name: CONTRACT.into(),
                        aliases: vec![],
                        kind: "contract".into(),
                    },
                    roles: vec!["agreement".into()],
                },
            ],
            memories: vec![IdentityObservedMemory {
                reference: IdentityMemoryRef {
                    id: [5; 32],
                    revision: 1,
                    body_id: [6; 32],
                    span: Some(IdentitySpan {
                        field: MemoryTextField::Content,
                        start_byte: 0,
                        end_byte: quote.len().try_into().unwrap(),
                    }),
                },
                lineage: vec![[lineage; 32]],
                exclusion: None,
            }],
            inactive_memories: Vec::new(),
            quote: Some(quote.into()),
            quote_digest: [7; 32],
        }],
        limit: None,
    }
}

fn answer(fact: &str) -> Value {
    json!({
        "outcome": "same_identity",
        "reason": "corroborated",
        "citations": [
            {"side": "left", "relationship": 0, "memory": 0, "fact": fact},
            {"side": "right", "relationship": 0, "memory": 0, "fact": fact},
        ],
    })
}

#[test]
fn pair_judgment_accepts_grounded_corroboration_and_preserves_uncertainty() {
    let pair = IdentityPairEvidence::new(side(1, 1, QUOTE), side(2, 2, QUOTE));
    assert!(deterministic_identity_judgment(&pair).is_none());

    let judgment = evaluate_identity_pair(&pair, &Scripted::reply(answer(CONTRACT))).unwrap();
    assert_eq!(judgment.outcome, IdentityOutcome::SameIdentity);
    assert_eq!(judgment.citations.len(), 2);
    assert_eq!(pair.quote(judgment.citations[1]), Some(QUOTE));

    for (reason, expected) in [
        ("ambiguity", IdentityReason::Ambiguity),
        ("conflict", IdentityReason::Conflict),
    ] {
        let scripted = Scripted::reply(json!({
            "outcome": "unresolved",
            "reason": reason,
            "citations": [],
        }));
        let judgment = evaluate_identity_pair(&pair, &scripted).unwrap();
        assert_eq!(judgment.outcome, IdentityOutcome::Unresolved);
        assert_eq!(judgment.reason, expected);
        assert_eq!(scripted.calls.load(Ordering::Relaxed), 1);
    }

    let names_only = Scripted::reply(answer("Alex Chen"));
    let judgment = evaluate_identity_pair(&pair, &names_only).unwrap();
    assert_eq!(judgment.outcome, IdentityOutcome::Unresolved);
    assert_eq!(judgment.reason, IdentityReason::Ambiguity);

    let mut invalid = answer(CONTRACT);
    invalid["citations"][1]["memory"] = json!(1);
    assert!(evaluate_identity_pair(&pair, &Scripted::reply(invalid)).is_err());
    assert!(evaluate_identity_pair(&pair, &Scripted::reply(answer("invented fact"))).is_err());
}

#[test]
fn common_provenance_missing_support_and_overflow_skip_provider() {
    let scripted = Scripted::reply(answer(CONTRACT));
    let duplicate = IdentityPairEvidence::new(side(1, 1, QUOTE), side(2, 1, QUOTE));
    assert_eq!(
        evaluate_identity_pair(&duplicate, &scripted)
            .unwrap()
            .reason,
        IdentityReason::Ambiguity
    );
    let mut missing = side(2, 2, QUOTE);
    missing.relationships[0].memories[0].reference.span = None;
    let missing = IdentityPairEvidence::new(side(1, 1, QUOTE), missing);
    assert_eq!(
        evaluate_identity_pair(&missing, &scripted).unwrap().reason,
        IdentityReason::InsufficientEvidence
    );
    let mut large = side(2, 2, QUOTE);
    large.endpoint.aliases = vec!["\\".repeat(512); 20];
    let large = IdentityPairEvidence::new(side(1, 1, QUOTE), large);
    assert_eq!(large.right.references().limit, Some(IdentityLimit::Budget));
    assert_eq!(
        evaluate_identity_pair(&large, &scripted).unwrap().reason,
        IdentityReason::EvidenceLimit
    );
    assert_eq!(scripted.calls.load(Ordering::Relaxed), 0);
}

#[test]
fn affirmative_answers_require_distinguishing_facts_beyond_generic_roles() {
    let cast_as_engineer = |owner| {
        let mut evidence = side(owner, owner, "Alex Chen is an Engineer.");
        evidence.relationships[0].participants[1]
            .entity
            .canonical_name = "Engineer".into();
        evidence
    };
    let generic = IdentityPairEvidence::new(cast_as_engineer(1), cast_as_engineer(2));
    assert_eq!(
        evaluate_identity_pair(&generic, &Scripted::reply(answer("Engineer")))
            .unwrap()
            .outcome,
        IdentityOutcome::Unresolved
    );

    let left = side(1, 1, QUOTE);
    let right = side(2, 2, QUOTE);
    let mut distinct = answer(CONTRACT);
    distinct["outcome"] = json!("related_distinct");
    distinct["reason"] = json!("distinct");
    distinct["citations"][1]["fact"] = json!("signed Project Meridian Contract");
    let same_anchor = IdentityPairEvidence::new(left.clone(), right.clone());
    let judgment =
        evaluate_identity_pair(&same_anchor, &Scripted::reply(distinct.clone())).unwrap();
    assert_eq!(judgment.outcome, IdentityOutcome::Unresolved);

    let birth_register = |owner, registry, place| {
        let quote = format!("Alex Chen was born in {place} as recorded in {registry}.");
        let mut evidence = side(owner, owner, &quote);
        evidence.relationships[0].kind = "place_of_birth".into();
        evidence.relationships[0].participants[0].roles = vec!["person".into()];
        evidence.relationships[0].participants[1]
            .entity
            .canonical_name = place.into();
        evidence.relationships[0].participants[1].entity.kind = "city".into();
        evidence.relationships[0].participants[1].roles = vec!["birthplace".into()];
        evidence
    };
    distinct["citations"][0]["fact"] = json!("born in Meridian City");
    distinct["citations"][1]["fact"] = json!("born in Lantern City");
    let separate_anchors = IdentityPairEvidence::new(
        birth_register(1, "Register A", "Meridian City"),
        birth_register(2, "Register B", "Lantern City"),
    );
    let judgment = evaluate_identity_pair(&separate_anchors, &Scripted::reply(distinct)).unwrap();
    assert_eq!(judgment.outcome, IdentityOutcome::RelatedDistinct);
}

#[test]
fn pair_fingerprint_includes_direction_and_supplied_fields_at_same_revisions() {
    let left = side(1, 1, QUOTE);
    let right = side(2, 2, QUOTE);
    let initial = IdentityPairEvidence::new(left.clone(), right.clone());
    let retry = IdentityPairEvidence::new(left.clone(), right.clone());
    assert_eq!(initial.fingerprint(), retry.fingerprint());
    let reversed = IdentityPairEvidence::new(right.clone(), left.clone());
    assert_ne!(initial.fingerprint(), reversed.fingerprint());
    let mut remounted = right;
    remounted.endpoint.aliases = vec!["Alexandra Chen".into()];
    let changed = IdentityPairEvidence::new(left, remounted);
    assert_eq!(initial.right.references(), changed.right.references());
    assert_ne!(initial.fingerprint(), changed.fingerprint());
}
