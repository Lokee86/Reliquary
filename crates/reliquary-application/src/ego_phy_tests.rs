use super::*;
use std::sync::Mutex;

struct Endpoint {
    calls: Mutex<Vec<String>>,
    over_cap: bool,
    hold: bool,
}

impl GeneralEndpoint for Endpoint {
    fn model(&self) -> &str {
        "scripted"
    }

    fn complete_json(
        &self,
        _: &str,
        payload: &str,
        schema_name: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.calls.lock().unwrap().push(schema_name.into());
        let payload: Value = serde_json::from_str(payload).unwrap();
        let supported = json!({"judgment": "supported", "hold_reason": ""});
        let reply = match schema_name {
            "ego_phy_draft" => {
                let text = if self.over_cap {
                    "Meetings use video. ".repeat(500).trim().into()
                } else {
                    "Meetings use video.".to_owned()
                };
                json!({"paragraphs": [{"text": text, "support": [{"slot": 0, "quote": "use video"}]}]})
            }
            "ego_phy_verify" => {
                assert_eq!(payload["final_summary"]["text"], "Meetings use video.");
                let mut reply = json!({
                    "checks": [{"paragraph_id": 0, "meaning": "supported", "time": "supported",
                        "scope": "supported", "attribution": "supported", "hold_reason": ""}],
                    "continuity": supported
                });
                if self.over_cap {
                    reply["preservation"] = supported;
                }
                if self.hold {
                    reply["checks"][0]["attribution"] = json!("uncertain");
                    reply["checks"][0]["hold_reason"] = json!("Attribution is uncertain.");
                }
                reply
            }
            _ => panic!("unexpected request"),
        };
        Ok(reply)
    }

    fn complete_json_with_tool(
        &self,
        _: &str,
        _: &str,
        schema_name: &str,
        _: &Value,
        tool: GeneralTool<'_>,
        run: &mut dyn FnMut(Value) -> Result<Value, GeneralEndpointError>,
    ) -> Result<Value, GeneralEndpointError> {
        self.calls.lock().unwrap().push(schema_name.into());
        assert_eq!(tool.name, phy_summary::TOKENIZER_TOOL);
        assert_eq!(tool.max_results, 4);
        let result = run(json!({"text": "Meetings use video."}))?;
        assert_eq!(result, json!({"tokens": 19, "cap": 4000, "fits": true}));
        Ok(
            json!({"paragraphs": [{"text": "Meetings use video.", "support":[{"slot":0,"quote":"use video"}]}]}),
        )
    }
}

fn capture() -> PhySummaryCapture {
    PhySummaryCapture {
        completion: EgoCompletion {
            id: [6; 32],
            cut: reliquary_owner_session::EgoSourceCut {
                owner_id: [1; 16],
                history_generation: [2; 32],
                memory_version: 3,
                audit_frontier: 4,
            },
            batch_ids: vec![[5; 32]],
        },
        expected_revision: 7,
        as_of: "2026-11-27".into(),
        prior: None,
        changes: vec![],
        evidence: vec![MemoryEvidence {
            citation: SourceCitation {
                owner_id: [1; 16],
                history_generation: [2; 32],
                source: SourceId::Memory([3; 32]),
                revision: 1,
                body_id: [4; 32],
                field: TextField::Content,
                start_byte: 0,
                end_byte: 19,
            },
            source_date: "2026-11-26".into(),
            text: "Meetings use video.".into(),
        }],
    }
}

#[test]
fn prepares_a_publication_after_one_final_verification_with_optional_compression() {
    let capture = capture();
    for over_cap in [false, true] {
        let endpoint = Endpoint {
            over_cap,
            hold: false,
            calls: Mutex::default(),
        };
        let actual = prepare_phy_publication(&capture, &endpoint, &|s| s.len() as u64, 60_000)
            .unwrap()
            .unwrap();
        assert_eq!(actual.completion, capture.completion);
        assert_eq!(actual.expected_revision, capture.expected_revision);
        let text = actual.replacement.unwrap();
        assert_eq!(text.text, "Meetings use video.");
        assert_eq!(text.token_count, 19);
        assert_eq!(text.paragraphs[0].citations[0].start_byte, 9);
        assert_eq!(text.paragraphs[0].citations[0].end_byte, 18);
        let expected = if over_cap {
            vec!["ego_phy_draft", "ego_phy_compress", "ego_phy_verify"]
        } else {
            vec!["ego_phy_draft", "ego_phy_verify"]
        };
        assert_eq!(*endpoint.calls.lock().unwrap(), expected);
    }
}

#[test]
fn held_verification_prepares_no_publication() {
    let endpoint = Endpoint {
        over_cap: false,
        hold: true,
        calls: Mutex::default(),
    };
    assert!(
        prepare_phy_publication(&capture(), &endpoint, &|s| s.len() as u64, 60_000)
            .unwrap()
            .is_none()
    );
}

#[test]
fn over_limit_capture_analyzes_each_packet_before_final_verification() {
    let mut capture = capture();
    for id in 6..12 {
        let mut new = capture.evidence[0].clone();
        new.citation.source = SourceId::Memory([id; 32]);
        new.text.push_str(&" padding".repeat(225));
        new.citation.end_byte = new.text.len() as u32;
        capture.evidence.push(new);
    }
    let endpoint = Endpoint {
        over_cap: false,
        hold: false,
        calls: Mutex::default(),
    };
    let summary = prepare_phy_publication(&capture, &endpoint, &|s| s.len() as u64, 10_000)
        .unwrap()
        .unwrap();
    assert_eq!(summary.replacement.unwrap().text, "Meetings use video.");
    let calls = endpoint.calls.lock().unwrap();
    assert!(calls.len() > 2);
    assert!(
        calls
            .chunks_exact(2)
            .all(|pair| pair == ["ego_phy_draft", "ego_phy_verify"])
    );
}
