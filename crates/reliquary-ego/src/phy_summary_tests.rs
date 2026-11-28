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
            start_byte: 10,
            end_byte: 10 + text.len() as u32,
        },
        source_date: "2026-11-26".into(),
        text: text.into(),
    }
}

fn paragraph(text: &str, support: Value) -> Value {
    json!({"text": text, "support": support})
}

fn supported() -> Value {
    json!({"judgment": "supported", "hold_reason": ""})
}

fn check(paragraph_id: usize) -> Value {
    json!({
        "paragraph_id": paragraph_id, "meaning": "supported", "time": "supported",
        "scope": "supported", "attribution": "supported", "hold_reason": ""
    })
}

fn bytes(text: &str) -> u64 {
    text.len() as u64
}

#[test]
fn fitting_summary_skips_compression_and_keeps_utf8_paragraph_and_quote_ranges() {
    let evidence = [memory(
        1,
        1,
        "Café meetings use video. Meetings avoid Friday.",
    )];
    let synthesis = Synthesis::new("2026-11-27", None, &[], &evidence).unwrap();
    let draft = synthesis.parse(json!({"paragraphs": [
        paragraph("Café meetings use video.", json!([{"slot": 0, "quote": "Café meetings use video"}])),
        paragraph("Meetings avoid Friday.", json!([{"slot": 0, "quote": "avoid Friday"}])),
    ]})).unwrap();
    let MeasuredDraft::Fits(final_draft) =
        synthesis.measure(draft, TokenBudget::new(DEFAULT_TOKEN_CAP, &bytes).unwrap())
    else {
        panic!("small draft should fit")
    };
    let verify = final_draft.verification();
    let request = verify.request();
    assert_eq!(
        request.input["memories"],
        synthesis.request().input["memories"]
    );
    assert!(request.input.get("uncompressed_draft").is_none());
    let verified = verify
        .accept(json!({"checks": [check(1), check(0)], "continuity": supported()}))
        .unwrap()
        .unwrap();
    assert_eq!(
        verified.text,
        "Café meetings use video.\n\nMeetings avoid Friday."
    );
    assert_eq!(verified.token_count, bytes(&verified.text));
    assert_eq!(verified.paragraphs[1].start_byte, 27);
    assert_eq!(verified.paragraphs[1].end_byte, 49);
    let first = verified.paragraphs[0].citations[0];
    assert_eq!((first.start_byte, first.end_byte), (10, 34));
    let second = verified.paragraphs[1].citations[0];
    assert_eq!((second.start_byte, second.end_byte), (45, 57));
}

#[test]
fn incremental_refresh_rebinds_corrected_sources_and_excludes_withdrawn_support() {
    let carry = memory(1, 1, "Time-box calls.");
    let old = memory(2, 1, "Set the budget to $900.");
    let removed = memory(3, 1, "Email receipts.");
    let prior = PriorSummary {
        text: "Time-box calls.\n\nSet the budget to $900.\n\nEmail receipts.".into(),
        paragraphs: vec![
            PriorParagraph {
                start_byte: 0,
                end_byte: 15,
                citations: vec![carry.citation],
                can_carry: true,
            },
            PriorParagraph {
                start_byte: 17,
                end_byte: 40,
                citations: vec![old.citation],
                can_carry: true,
            },
            PriorParagraph {
                start_byte: 42,
                end_byte: 57,
                citations: vec![removed.citation],
                can_carry: true,
            },
        ],
    };
    let evidence = [memory(2, 2, "Set the budget to $850.")];
    let changes = [
        MemoryChange {
            memory_id: [2; 32],
            kind: ChangeKind::Revised,
        },
        MemoryChange {
            memory_id: [3; 32],
            kind: ChangeKind::Withdrawn,
        },
    ];
    let synthesis = Synthesis::new("2026-11-27", Some(&prior), &changes, &evidence).unwrap();
    let input = synthesis.request().input;
    assert_eq!(input["previous"]["paragraphs"][0]["can_carry"], true);
    assert_eq!(input["previous"]["paragraphs"][1]["can_carry"], false);
    assert_eq!(input["previous"]["paragraphs"][2]["can_carry"], false);
    assert_eq!(
        input["changes"][0]["memory"],
        input["previous"]["paragraphs"][1]["references"][0]["memory"]
    );
    assert!(matches!(
        synthesis.parse(json!({"paragraphs": [paragraph("Email receipts.", json!([]))]})),
        Err(Error::InvalidCitation)
    ));
    assert!(matches!(
        synthesis.parse(json!({"paragraphs": [paragraph("Time box calls.", json!([]))]})),
        Err(Error::InvalidCitation)
    ));
    let draft = synthesis
        .parse(json!({"paragraphs": [
            paragraph("Time-box calls.", json!([])),
            paragraph("Set the budget to $850.", json!([{"slot": 0, "quote": "budget to $850"}]))
        ]}))
        .unwrap();
    let MeasuredDraft::Fits(final_draft) =
        synthesis.measure(draft, TokenBudget::new(DEFAULT_TOKEN_CAP, &bytes).unwrap())
    else {
        panic!("small draft should fit")
    };
    let verify = final_draft.verification();
    assert_eq!(
        verify.request().input["final_summary"]["paragraphs"][0]["carry_from"],
        0
    );
    let verified = verify
        .accept(json!({"checks": [check(0), check(1)], "continuity": supported()}))
        .unwrap()
        .unwrap();
    assert_eq!(verified.paragraphs[0].citations, vec![carry.citation]);
    assert_eq!(verified.paragraphs[1].citations[0].revision, 2);
    assert!(!verified.text.contains("Email"));
}

#[test]
fn over_cap_job_has_four_tokenizer_results_and_one_terminal_candidate() {
    let evidence = [memory(1, 1, "Meetings of every kind use video.")];
    let synthesis = Synthesis::new("2026-11-27", None, &[], &evidence).unwrap();
    let initial = json!({"paragraphs": [
        paragraph("Meetings of every kind use video.", json!([{"slot": 0, "quote": "Meetings of every kind use video"}]))
    ]});
    let terminal = json!({"paragraphs": [
        paragraph("All meetings use video.", json!([{"slot": 0, "quote": "use video"}]))
    ]});
    let compress = || {
        let draft = synthesis.parse(initial.clone()).unwrap();
        let budget = TokenBudget::new(24, &bytes).unwrap();
        let MeasuredDraft::Compress(compress) = synthesis.measure(draft, budget) else {
            panic!("draft should be over cap")
        };
        compress
    };
    let mut job = compress();
    assert_eq!(job.request().tool_name, TOKENIZER_TOOL);
    for _ in 0..MAX_TOKENIZER_RESULTS {
        let result = job
            .tokenize(json!({"text": "All meetings use video."}))
            .unwrap();
        assert_eq!(result, json!({"tokens": 23, "cap": 24, "fits": true}));
    }
    let verified = job
        .finish(terminal.clone())
        .unwrap()
        .unwrap()
        .verification();
    assert_eq!(
        verified.request().input["uncompressed_draft"]["text"],
        "Meetings of every kind use video."
    );
    let accepted = verified
        .accept(
            json!({"checks": [check(0)], "continuity": supported(), "preservation": supported()}),
        )
        .unwrap()
        .unwrap();
    assert_eq!(accepted.text, "All meetings use video.");
    assert_eq!(accepted.token_count, 23);

    let mut job = compress();
    for _ in 0..MAX_TOKENIZER_RESULTS {
        job.tokenize(json!({"text": "Still working"})).unwrap();
    }
    assert_eq!(
        job.tokenize(json!({"text": "Fifth request"})),
        Err(Error::CompressionExhausted)
    );
    assert!(job.finish(terminal.clone()).unwrap().is_none());
    assert!(compress().finish(initial.clone()).unwrap().is_none());

    let verified = compress().finish(terminal).unwrap().unwrap().verification();
    assert!(
        verified
            .accept(json!({
                "checks": [check(0)], "continuity": supported(),
                "preservation": {"judgment": "unsupported", "hold_reason": "Lost material detail."}
            }))
            .unwrap()
            .is_none()
    );
}

#[test]
fn malformed_checks_and_unsupported_paragraphs_hold_the_entire_summary() {
    let evidence = [memory(1, 1, "Confirm calls. Avoid Friday. aaa")];
    let synthesis = Synthesis::new("2026-11-27", None, &[], &evidence).unwrap();
    assert!(matches!(
        synthesis.parse(json!({"paragraphs": [
            paragraph("Bad quote.", json!([{"slot": 0, "quote": "aa"}]))
        ]})),
        Err(Error::InvalidCitation)
    ));
    let verify = || {
        let draft = synthesis
            .parse(json!({"paragraphs": [
                paragraph("Confirm calls.", json!([{"slot": 0, "quote": "Confirm calls"}])),
                paragraph("Avoid Friday.", json!([{"slot": 0, "quote": "Avoid Friday"}]))
            ]}))
            .unwrap();
        let MeasuredDraft::Fits(candidate) =
            synthesis.measure(draft, TokenBudget::new(DEFAULT_TOKEN_CAP, &bytes).unwrap())
        else {
            panic!("small draft should fit")
        };
        candidate.verification()
    };
    let mut held = check(1);
    held["scope"] = json!("uncertain");
    held["hold_reason"] = json!("The relevant participants are unclear.");
    assert!(
        verify()
            .accept(json!({"checks": [check(0), held], "continuity": supported()}))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        verify().accept(json!({"checks": [check(0), check(0)], "continuity": supported()})),
        Err(Error::InvalidVerification)
    );
}
