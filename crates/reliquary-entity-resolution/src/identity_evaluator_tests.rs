use super::*;
use reliquary_perception_store::{
    EntityId, MemoryTextField, RelationshipId,
    identity::{IdentityLimit, IdentityMemoryRef, IdentitySpan},
    identity_evidence::{IdentityEntityEvidence, IdentityParticipantEvidence},
};
use std::sync::atomic::{AtomicUsize, Ordering};

const CONTRACT: &str = "Project Meridian Contract";
const QUOTE: &str = "Alex Chen signed Project Meridian Contract.";

pub(crate) struct Scripted {
    calls: AtomicUsize,
    output: Value,
}

impl Scripted {
    pub(crate) fn reply(output: Value) -> Self {
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

pub(crate) fn side(owner: u8, lineage: u8, quote: &str) -> IdentityEvidence {
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
            {"side": "left", "relationship": 0, "memory": 0, "fact": fact, "bearing": "corroborates_identity"},
            {"side": "right", "relationship": 0, "memory": 0, "fact": fact, "bearing": "corroborates_identity"},
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
        evidence.relationships[0].kind = "works_as".into();
        evidence.relationships[0].participants[0].roles = vec!["engineer".into()];
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

    let mut harbor_contract = side(2, 2, "Alex Chen signed the Harbor Contract.");
    harbor_contract.relationships[0].participants[1]
        .entity
        .canonical_name = "Harbor Contract".into();
    distinct["citations"][1]["fact"] = json!("Harbor Contract");
    for citation in distinct["citations"].as_array_mut().unwrap() {
        citation["bearing"] = json!("separates_referents");
    }
    let two_contracts = IdentityPairEvidence::new(left.clone(), harbor_contract);
    let mut same = answer(CONTRACT);
    same["citations"][1]["fact"] = json!("Harbor Contract");
    assert_eq!(
        evaluate_identity_pair(&two_contracts, &Scripted::reply(same))
            .unwrap()
            .outcome,
        IdentityOutcome::Unresolved
    );
    let judgment =
        evaluate_identity_pair(&two_contracts, &Scripted::reply(distinct.clone())).unwrap();
    assert_eq!(judgment.outcome, IdentityOutcome::Unresolved);
    assert_eq!(judgment.reason, IdentityReason::Ambiguity);
}

#[test]
fn distinction_requires_independent_sources_identifying_separate_referents() {
    let witness = |owner, endpoint: &str, kind: &str, context: &str, quote: &str| {
        let mut evidence = side(owner, owner, quote);
        evidence.endpoint.canonical_name = endpoint.into();
        evidence.endpoint.kind = kind.into();
        evidence.endpoint.aliases.clear();
        let relation = &mut evidence.relationships[0];
        relation.kind = "associated_with".into();
        relation.participants[0].entity = evidence.endpoint.clone();
        relation.participants[0].roles.clear();
        relation.participants[1].entity.canonical_name = context.into();
        relation.participants[1].entity.kind = "referent".into();
        relation.participants[1].roles.clear();
        evidence
    };
    let left =
        "The County Survey marks two sites named Eagle Point. This is the ridge north of Lake Ida.";
    let right =
        "The Trail Atlas maps both Eagle Points and locates this one on the southern shore.";
    let pair = IdentityPairEvidence::new(
        witness(1, "Eagle Point", "place", "County Survey", left),
        witness(2, "Eagle Point", "place", "Trail Atlas", right),
    );
    let mut separate = answer(left);
    separate["outcome"] = json!("related_distinct");
    separate["reason"] = json!("distinct");
    separate["citations"][0]["bearing"] = json!("separates_referents");
    separate["citations"][1]["fact"] = json!(right);
    separate["citations"][1]["bearing"] = json!("describes_referent");
    // One source establishing separation is insufficient.
    let evaluate = |pair, output: &Value| {
        evaluate_identity_pair(pair, &Scripted::reply(output.clone())).unwrap()
    };
    assert_eq!(
        evaluate(&pair, &separate).outcome,
        IdentityOutcome::Unresolved
    );
    separate["citations"][1]["bearing"] = json!("separates_referents");
    let judgment = evaluate(&pair, &separate);
    assert_eq!(judgment.outcome, IdentityOutcome::RelatedDistinct);
    assert_eq!(judgment.citations.len(), 2);

    let marker = "Brass Survey Marker K17";
    let same_place = IdentityPairEvidence::new(
        witness(
            1,
            "Eagle Point",
            "place",
            marker,
            "The Eagle Point summit is at Brass Survey Marker K17.",
        ),
        witness(
            2,
            "Pointe de l'Aigle",
            "place",
            marker,
            "Pointe de l'Aigle is the summit at Brass Survey Marker K17.",
        ),
    );
    assert_eq!(
        evaluate(&same_place, &answer(marker)).outcome,
        IdentityOutcome::SameIdentity
    );

    let left = "Register A lists Meridian City as Alex Chen's birthplace.";
    let right = "Register B lists Lantern City as Alex Chen's birthplace.";
    let pair = IdentityPairEvidence::new(
        witness(1, "Alex Chen", "person", "Register A", left),
        witness(2, "Alex Chen", "person", "Register B", right),
    );
    for (index, fact) in [left, right].into_iter().enumerate() {
        separate["citations"][index]["fact"] = json!(fact);
        separate["citations"][index]["bearing"] = json!("conflicting_account");
    }
    let judgment = evaluate(&pair, &separate);
    assert_eq!(judgment.outcome, IdentityOutcome::Unresolved);
    assert_eq!(judgment.reason, IdentityReason::Conflict);
}

#[test]
fn independently_worded_witnesses_can_corroborate_across_relationships() {
    let left_quote = "Alex Chen founded the Glasslight Quartet under the stage name Vale.";
    let right_quote = "At its first concert, Glasslight's founder introduced herself as Alex Chen, performing as Vale.";
    let mut left = side(1, 1, left_quote);
    let mut right = side(2, 2, right_quote);
    left.relationships[0].kind = "founded".into();
    left.relationships[0].participants[0].roles = vec!["founder".into()];
    left.relationships[0].participants[1].entity.canonical_name = "Glasslight Quartet".into();
    right.relationships[0].kind = "appeared_at".into();
    right.relationships[0].participants[0].roles = vec!["performer".into()];
    right.relationships[0].participants[1].entity.canonical_name = "first concert".into();
    let pair = IdentityPairEvidence::new(left, right);
    let mut reply = answer("founded the Glasslight Quartet under the stage name Vale");
    reply["citations"][1]["fact"] =
        json!("Glasslight's founder introduced herself as Alex Chen, performing as Vale");
    let judge = |reply| {
        evaluate_identity_pair(&pair, &Scripted::reply(reply))
            .unwrap()
            .outcome
    };

    assert_eq!(judge(reply.clone()), IdentityOutcome::SameIdentity);
    reply["citations"][1]["bearing"] = json!("describes_referent");
    assert_eq!(judge(reply), IdentityOutcome::Unresolved);
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
