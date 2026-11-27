use super::*;
use reliquary_memory_audit::TextField;

fn memory(id: u8, revision: u64, text: &str) -> MemoryEvidence {
    MemoryEvidence {
        citation: SourceCitation {
            owner_id: [1; 16],
            history_generation: [2; 32],
            source: SourceId::Memory([id; 32]),
            revision,
            body_id: [revision as u8; 32],
            field: TextField::Content,
            start_byte: 20,
            end_byte: 20 + text.len() as u32,
        },
        source_date: "2026-11-26".into(),
        text: text.into(),
    }
}

fn embedding(vector: &[f32]) -> Option<Embedding> {
    Some(Embedding {
        profile_id: [1; 32],
        vector: vector.into(),
    })
}

fn work(evidence: &[MemoryEvidence], refresh: bool) -> TopicWork {
    let references: Vec<_> = evidence.iter().map(|memory| memory.citation).collect();
    TopicWork {
        proposal: TopicProposal {
            title: "Printing".into(),
            embedding: embedding(&[1.0, 0.0]),
            references: references.clone(),
        },
        matched: refresh.then_some(TopicTarget {
            id: [2; 16],
            revision: 2,
        }),
        new_references: references,
    }
}

fn draft(statement: &str, support: Value) -> Value {
    json!({"statement": statement, "status": "observed", "time_scope": "current", "support": support})
}

fn check(id: usize) -> Value {
    json!({"point_id": id, "meaning": "supported", "status": "supported", "time_scope": "supported",
        "attribution": "supported", "hold_reason": ""})
}

fn existing(id: u8, memory: MemoryEvidence, vect: Option<Embedding>) -> ExistingPoint {
    ExistingPoint {
        id: [id; 16],
        revision: 4,
        statement: memory.text.clone(),
        status: Status::Observed,
        time_scope: TimeScope::Current,
        embedding: vect,
        citations: vec![memory.citation],
        evidence: vec![memory],
    }
}

#[test]
fn consolidates_similar_topics_and_only_new_revisions_advance_matched_work() {
    let a = memory(1, 1, "Members can print.");
    let b = memory(2, 1, "Visitors can scan.");
    let mut first = work(&[a.clone()], false).proposal;
    first.embedding = embedding(&[1.0, 0.0]);
    let mut second = work(&[b.clone()], false).proposal;
    second.title = "Printing and scanning".into();
    second.embedding = embedding(&[0.86, 0.51]);
    let proposals = consolidate_topics(vec![first, second]);
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].references, vec![a.citation, b.citation]);

    let mut covered = a.citation;
    covered.end_byte -= 1;
    let mut saved = SavedTopic {
        id: [1; 16],
        revision: 2,
        title: "Printing".into(),
        embedding: embedding(&[1.0, 0.0]),
        references: vec![covered],
        suppressed: false,
    };
    let planned = plan_topic(proposals[0].clone(), &[saved.clone()]).unwrap();
    assert_eq!(planned.matched.unwrap().id, saved.id);
    assert_eq!(planned.new_references, [b.citation]);
    saved.references.push(b.citation);
    assert!(plan_topic(proposals[0].clone(), &[saved.clone()]).is_none());
    saved.references.clear();
    saved.suppressed = true;
    assert!(plan_topic(proposals[0].clone(), &[saved]).is_none());
}

#[test]
fn exact_quote_and_judgment_failures_hold_only_their_points() {
    let evidence = [memory(1, 1, "Édition allows color. It needs a badge. aaa")];
    let packet = Synthesis::new("Printing", "2026-11-27", &evidence).unwrap();
    let work = work(&evidence, false);
    let result = json!({"points": [
        draft("Color is allowed.", json!([{"slot": 0, "quote": "allows color"}])),
        draft("A badge is required.", json!([{"slot": 0, "quote": "needs a badge"}])),
        draft("Ambiguous.", json!([{"slot": 0, "quote": "aa"}])),
        draft("Unknown.", json!([{"slot": 1, "quote": "Édition"}])),
        draft("Repeated.", json!([{"slot": 0, "quote": "Édition"}, {"slot": 0, "quote": "Édition"}]))
    ]});
    let verification = Verification::new(&packet, &work, packet.parse(result).unwrap(), &[], &[]);
    assert_eq!(
        verification.request().input["memories"],
        packet.request().input["memories"]
    );
    assert_eq!(
        verification.request().input["points"][1]["statement"],
        "A badge is required."
    );
    let mut hold = check(1);
    hold["attribution"] = json!("uncertain");
    hold["hold_reason"] = json!("The scope was unclear.");
    let accepted = verification
        .accept(json!({"checks": [check(4), hold, check(0), check(3), check(2)]}))
        .unwrap();
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0].citations[0].start_byte, 29);
    assert_eq!(accepted[0].citations[0].end_byte, 41);
}

#[test]
fn rejects_missing_duplicate_or_unknown_checks() {
    let evidence = [memory(1, 1, "A badge is required.")];
    let packet = Synthesis::new("Printing", "2026-11-27", &evidence).unwrap();
    let work = work(&evidence, false);
    let item = draft(
        "A badge is required.",
        json!([{"slot": 0, "quote": "badge"}]),
    );
    assert!(matches!(
        packet.parse(json!({"points": vec![item.clone(); 6]})),
        Err(Error::InvalidSynthesis)
    ));
    for checks in [
        vec![check(0)],
        vec![check(0), check(0)],
        vec![check(0), check(2)],
    ] {
        let drafts = packet
            .parse(json!({"points": [item.clone(), item.clone()]}))
            .unwrap();
        let verification = Verification::new(&packet, &work, drafts, &[], &[]);
        assert_eq!(
            verification.accept(json!({"checks": checks})),
            Err(Error::InvalidVerification)
        );
    }
    let drafts = packet.parse(json!({"points": []})).unwrap();
    let verification = Verification::new(&packet, &work, drafts, &[], &[]);
    assert!(
        verification
            .accept(json!({"checks": []}))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn refresh_keeps_only_points_citing_new_revisions() {
    let evidence = [
        memory(1, 3, "Members need a badge."),
        memory(2, 1, "Visitors cannot print."),
    ];
    let packet = Synthesis::new("Printing", "2026-11-27", &evidence).unwrap();
    let mut work = work(&evidence, true);
    work.new_references.truncate(1);
    let drafts = packet
        .parse(json!({"points": [
            draft("Members need a badge.", json!([{"slot": 0, "quote": "need a badge"}])),
            draft("Visitors cannot print.", json!([{"slot": 1, "quote": "cannot print"}]))
        ]}))
        .unwrap();
    let verified = Verification::new(&packet, &work, drafts, &[], &[])
        .accept(json!({"checks": [check(0), check(1)]}))
        .unwrap();
    assert_eq!(verified.len(), 1);
    assert_eq!(verified[0].statement, "Members need a badge.");
}

#[test]
fn replacement_checks_bind_predecessors_and_allow_independent_scopes() {
    let evidence = [memory(
        1,
        2,
        "Members need two credits. Visitors need one credit.",
    )];
    let packet = Synthesis::new("Printing", "2026-11-27", &evidence).unwrap();
    let work = work(&evidence, true);
    let live = [
        existing(
            7,
            memory(3, 1, "Members need three credits."),
            embedding(&[1.0, 0.0]),
        ),
        existing(8, memory(1, 1, "Members print one job at a time."), None),
    ];
    let points = json!({"points": [
        draft("Members need two credits.", json!([{"slot": 0, "quote": "Members need two credits"}])),
        draft("Visitors need one credit.", json!([{"slot": 0, "quote": "Visitors need one credit"}]))
    ]});
    let verification = || {
        let mut drafts = packet.parse(points.clone()).unwrap();
        drafts[0].embedding = embedding(&[1.0, 0.0]);
        Verification::new(&packet, &work, drafts, &live, &[])
    };
    let request = verification().request();
    let mut checks = Vec::new();
    for candidate in request.input["replacement_candidates"].as_array().unwrap() {
        let mut response = json!({"relationship": "independent"});
        for key in [
            "point_id",
            "candidate_id",
            "predecessor_id",
            "predecessor_revision",
        ] {
            response[key] = candidate[key].clone();
        }
        if candidate["point_id"] == 0 && candidate["candidate_id"] == 0 {
            assert_eq!(candidate["predecessor_id"], json!(vec![7u8; 16]));
            response["relationship"] = json!("replaces");
        }
        checks.push(response);
    }
    assert_eq!(checks.len(), 3);
    let response = json!({"checks": [check(0), check(1)], "replacement_checks": checks});
    let accepted = verification().accept(response.clone()).unwrap();
    assert_eq!(accepted[0].replacement.unwrap().id, [7; 16]);
    assert_eq!(accepted[1].replacement, None);

    let mut fallback = response.clone();
    fallback["replacement_checks"][0]["relationship"] = json!("independent");
    fallback["replacement_checks"][1]["relationship"] = json!("replaces");
    assert_eq!(
        verification().accept(fallback).unwrap()[0]
            .replacement
            .unwrap()
            .id,
        [8; 16]
    );

    let mut ambiguous = response.clone();
    ambiguous["replacement_checks"][0]["relationship"] = json!("uncertain");
    let accepted = verification().accept(ambiguous).unwrap();
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0].statement, "Visitors need one credit.");

    let mut forged = response;
    forged["replacement_checks"][0]["predecessor_revision"] = json!(17);
    assert_eq!(
        verification().accept(forged),
        Err(Error::InvalidVerification)
    );
}

#[test]
fn suppression_blocks_exact_and_close_equivalent_points_but_keeps_changed_value() {
    let evidence = [memory(1, 2, "A badge requires two credits.")];
    let packet = Synthesis::new("Printing", "2026-11-27", &evidence).unwrap();
    let work = work(&evidence, false);
    let suppressed = [existing(
        1,
        memory(1, 1, "A badge requires three credits."),
        embedding(&[1.0]),
    )];
    let make_drafts = || {
        let points: Vec<_> = [
            "A badge requires three credits.",
            "A badge needs three credits.",
            "A badge needs two credits.",
        ]
        .into_iter()
        .map(|statement| draft(statement, json!([{"slot": 0, "quote": "A badge"}])))
        .collect();
        let mut drafts = packet.parse(json!({"points": points})).unwrap();
        for draft in &mut drafts {
            draft.embedding = embedding(&[1.0]);
        }
        drafts
    };
    let response = json!({"checks": [check(0), check(1), check(2)]});
    let verified = Verification::new(&packet, &work, make_drafts(), &[], &suppressed)
        .accept(response.clone())
        .unwrap();
    assert_eq!(verified.len(), 1);
    assert_eq!(verified[0].statement, "A badge needs two credits.");
    assert_eq!(
        Verification::new(&packet, &work, make_drafts(), &[], &[])
            .accept(response)
            .unwrap()
            .len(),
        3
    );
}
