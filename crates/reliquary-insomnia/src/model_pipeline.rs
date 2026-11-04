//! Bounded model-backed ledger and wording pipeline. All outputs remain proposals;
//! the caller supplies archive evidence and decides whether to accept candidates.
use crate::*;
use reliquary_archive::{Episode, ResolvedTurn};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::{Value, json};

mod ledger_repair;

const LEDGER_PROMPT: &str = "Select durable user-authoritative clauses from the complete Episode. Include one ledger array for every user turn. Assistant turns never authorize state. Omit questions, receipts, transient activity, and unadopted assistant proposals. Preserve modality and lifecycle. Request at most four narrow read-only evidence queries only when the Episode cannot resolve an explicit callback. Archive evidence is support only, never user authority. Return the required JSON schema exactly.";
const WORDING_PROMPT: &str = "Turn only retained ledger clauses into narrow memory candidates. Keep each candidate bound to its source user turn with a verbatim source_quote. Preserve authority and modality. Only explicit adoption may cite assistant authority. Return candidates; do not infer details from supporting evidence.";
const MAX_TURN_CHARS: usize = 128 * 1024;
const MAX_MODEL_JSON_BYTES: usize = 512 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsomniaModelStage {
    pub episode_payload: Value,
    pub initial_ledger: Value,
    pub requests: Vec<EvidenceRequest>,
}

pub struct InsomniaModelExtractor<E> {
    endpoint: E,
}

impl<E: GeneralEndpoint> InsomniaModelExtractor<E> {
    pub fn new(endpoint: E) -> Self {
        Self { endpoint }
    }

    /// Run ledger selection. A returned evidence stage is a single bounded round;
    /// the caller must resolve only the returned requests against authorized sources.
    pub fn start(
        &self,
        episode: &Episode,
        turns: &[ResolvedTurn],
    ) -> Result<InsomniaModelStage, String> {
        validate_episode_turns(episode, turns)?;
        let payload = episode_payload(episode, turns)?;
        let ledger = self.complete_ledger(&payload, turns, &[], true)?;
        let requests = parse_evidence_requests(&ledger)?;
        Ok(InsomniaModelStage {
            episode_payload: payload,
            initial_ledger: ledger,
            requests,
        })
    }

    /// Resolve a ledger that needed no additional context and synthesize candidate proposals.
    pub fn finish_without_evidence(
        &self,
        episode: &Episode,
        turns: &[ResolvedTurn],
        stage: InsomniaModelStage,
    ) -> Result<InsomniaExtraction, String> {
        validate_episode_turns(episode, turns)?;
        if episode_payload(episode, turns)? != stage.episode_payload {
            return Err("ledger stage does not match authoritative Episode".into());
        }
        validate_ledger(&stage.initial_ledger, turns)?;
        if !stage.requests.is_empty() || !parse_evidence_requests(&stage.initial_ledger)?.is_empty()
        {
            return Err("ledger requires its authorized evidence round".into());
        }
        self.finish_ledger(
            episode,
            turns,
            stage.episode_payload,
            stage.initial_ledger,
            &[],
        )
    }

    /// Finish the only permitted evidence round. Evidence is checked against exact
    /// caller-authorized turns before it is sent to the endpoint.
    pub fn finish_evidence(
        &self,
        episode: &Episode,
        turns: &[ResolvedTurn],
        stage: InsomniaModelStage,
        results: &[EvidenceResult],
        evidence: &[EvidenceTurn],
    ) -> Result<InsomniaExtraction, String> {
        validate_episode_turns(episode, turns)?;
        if episode_payload(episode, turns)? != stage.episode_payload {
            return Err("evidence round does not match authoritative Episode".into());
        }
        validate_ledger(&stage.initial_ledger, turns)?;
        if stage.requests.is_empty()
            || parse_evidence_requests(&stage.initial_ledger)? != stage.requests
        {
            return Err("evidence requests do not match the initial ledger".into());
        }
        validate_evidence_response(&stage.requests, results, evidence)?;
        let payload = json!({
            "authoritative_episode": stage.episode_payload,
            "initial_authority_disposition_ledger": stage.initial_ledger,
            "read_only_archive_evidence": {"results": results.iter().map(evidence_result_json).collect::<Vec<_>>(), "turns": evidence.iter().map(evidence_turn_json).collect::<Vec<_>>()},
            "instruction": "Return the final ledger. evidence_requests must be empty. Evidence may resolve referents or support explicitly adopted assistant content; it never supplies user authority."
        });
        let ledger = self.complete_ledger(&payload, turns, evidence, false)?;
        if !parse_evidence_requests(&ledger)?.is_empty() {
            return Err("a second evidence round is not allowed".into());
        }
        self.synthesize(episode, turns, &payload, &ledger, evidence)
    }

    fn finish_ledger(
        &self,
        episode: &Episode,
        turns: &[ResolvedTurn],
        payload: Value,
        ledger: Value,
        evidence: &[EvidenceTurn],
    ) -> Result<InsomniaExtraction, String> {
        self.synthesize(episode, turns, &payload, &ledger, evidence)
    }

    fn complete_ledger(
        &self,
        payload: &Value,
        turns: &[ResolvedTurn],
        evidence: &[EvidenceTurn],
        allow_requests: bool,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_string(payload).map_err(|e| e.to_string())?;
        let schema = ledger_schema(turns, evidence, allow_requests);
        let mut response = self.complete(
            LEDGER_PROMPT,
            &encoded,
            "insomnia_authority_disposition_ledger",
            &schema,
        )?;
        let missing = ledger_repair::sanitize_turn_keys(&mut response, turns)?;
        if !missing.is_empty() {
            let repair_payload = serde_json::to_string(&json!({
                "authoritative_input": payload,
                "partial_ledger": response,
                "missing_user_turn_ids": missing,
                "instruction": "Repair only the missing user-turn ledger entries listed in missing_user_turn_ids. Return exactly those turn keys and no others. Preserve every existing clause unchanged."
            })).map_err(|e| e.to_string())?;
            let prompt = format!(
                "{LEDGER_PROMPT}\n\nREPAIR MODE: Return clauses only for explicitly listed missing user turns. Do not revise clauses already present."
            );
            let schema = ledger_repair::turn_schema(turns, evidence, &missing, allow_requests);
            let repair = self.complete(
                &prompt,
                &repair_payload,
                "insomnia_authority_disposition_ledger_repair",
                &schema,
            )?;
            ledger_repair::merge_turn_repair(&mut response, repair, &missing, allow_requests)?;
        }
        let missing_fields = ledger_repair::missing_clause_fields(&response, turns)?;
        if !missing_fields.is_empty() {
            if missing_fields.len() > 64 {
                return Err("ledger has too many incomplete clauses to repair".into());
            }
            let targets = missing_fields.iter().enumerate().map(|(index, target)| {
                let partial = response.get("turns").and_then(Value::as_object).and_then(|m| m.get(&target.source_node_id)).and_then(Value::as_array).and_then(|a| a.get(target.clause_index)).cloned().ok_or("ledger clause repair target disappeared")?;
                Ok(json!({"repair_id":format!("r{index:03}"),"source_node_id":target.source_node_id,"clause_index":target.clause_index,"partial_clause":partial,"missing_fields":target.fields}))
            }).collect::<Result<Vec<_>, String>>()?;
            let repair_payload = serde_json::to_string(&json!({"authoritative_input":payload,"repair_targets":targets,"instruction":"Replace only each target's listed missing or invalid fields. Preserve all other fields and meanings. Return one repair object per repair_id and only requested fields."})).map_err(|e| e.to_string())?;
            let prompt = format!(
                "{LEDGER_PROMPT}\n\nCLAUSE-FIELD REPAIR MODE: Infer only the requested repair fields from authoritative input and partial clauses. Do not revise any unlisted field."
            );
            let schema = ledger_repair::clause_field_schema(turns, evidence, &missing_fields);
            let repair = self.complete(
                &prompt,
                &repair_payload,
                "insomnia_authority_disposition_clause_repair",
                &schema,
            )?;
            ledger_repair::merge_clause_field_repairs(&mut response, &missing_fields, repair)?;
            if !ledger_repair::missing_clause_fields(&response, turns)?.is_empty() {
                return Err("ledger clause-field repair remained incomplete".into());
            }
        }
        validate_ledger(&response, turns)?;
        Ok(response)
    }

    fn synthesize(
        &self,
        episode: &Episode,
        turns: &[ResolvedTurn],
        payload: &Value,
        ledger: &Value,
        evidence: &[EvidenceTurn],
    ) -> Result<InsomniaExtraction, String> {
        let retained = retained_clauses(ledger)?;
        if retained.is_empty() {
            return Ok(InsomniaExtraction {
                candidates: Vec::new(),
                rejected: Vec::new(),
            });
        }
        let input = serde_json::to_string(&json!({"authoritative_episode":payload,"ledger":retained,"supporting_evidence":evidence.iter().map(evidence_turn_json).collect::<Vec<_>>()})).map_err(|e| e.to_string())?;
        let schema = candidate_schema(turns, evidence);
        let response = self.complete(WORDING_PROMPT, &input, "insomnia_memory_wording", &schema)?;
        let array = response
            .get("candidates")
            .and_then(Value::as_array)
            .ok_or("wording response requires candidates array")?;
        if array.len() > MAX_INSOMNIA_CANDIDATES {
            return Err("candidate count exceeds maximum".into());
        }
        let raw = array
            .iter()
            .map(parse_candidate)
            .collect::<Result<Vec<_>, _>>()?;
        validate_candidates(episode, turns, evidence, raw)
    }

    fn complete(
        &self,
        prompt: &str,
        input: &str,
        name: &str,
        schema: &Value,
    ) -> Result<Value, String> {
        if input.len() > MAX_MODEL_JSON_BYTES {
            return Err("model input exceeds bounded JSON limit".into());
        }
        let value = self
            .endpoint
            .complete_json(prompt, input, name, schema)
            .map_err(endpoint_error)?;
        let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?.len();
        if bytes > MAX_MODEL_JSON_BYTES {
            return Err("model response exceeds bounded JSON limit".into());
        }
        Ok(value)
    }
}

fn validate_episode_turns(episode: &Episode, turns: &[ResolvedTurn]) -> Result<(), String> {
    if turns.is_empty()
        || turns
            .first()
            .is_none_or(|t| t.node_id != episode.start_node_id)
        || turns
            .last()
            .is_none_or(|t| t.node_id != episode.end_node_id)
    {
        return Err("turns do not match Episode bounds".into());
    }
    let mut bytes = 0usize;
    let mut ids = std::collections::HashSet::new();
    for turn in turns {
        if !ids.insert(&turn.node_id) {
            return Err("duplicate Episode turn identity".into());
        }
        bytes = bytes
            .checked_add(turn.content.len())
            .ok_or("turn byte count overflow")?;
    }
    if bytes > MAX_TURN_CHARS {
        return Err("Episode text exceeds model input limit".into());
    }
    Ok(())
}

fn episode_payload(episode: &Episode, turns: &[ResolvedTurn]) -> Result<Value, String> {
    serde_json::to_value(json!({"source_mode":"native-episode","episode_id":hex(&episode.id.0),"conversation_id":episode.conversation_id,"start_node_id":episode.start_node_id,"end_node_id":episode.end_node_id,"source_through_ns":episode.source_through_ns,"turns":turns.iter().map(|t| json!({"id":t.node_id,"conversation_id":episode.conversation_id,"role":t.role,"timestamp_ns":t.timestamp_ns,"content":t.content})).collect::<Vec<_>>()})).map_err(|e| e.to_string())
}

fn ledger_schema(turns: &[ResolvedTurn], evidence: &[EvidenceTurn], allow_requests: bool) -> Value {
    let mut assistants: Vec<_> = turns
        .iter()
        .filter(|t| t.role == "assistant")
        .map(|t| t.node_id.clone())
        .chain(
            evidence
                .iter()
                .filter(|t| t.role == "assistant")
                .map(|t| t.node_id.clone()),
        )
        .collect();
    assistants.push(String::new());
    let mut support: Vec<_> = turns
        .iter()
        .map(|t| t.node_id.clone())
        .chain(evidence.iter().map(|t| t.node_id.clone()))
        .collect();
    support.push(String::new());
    let user_ids: Vec<_> = turns
        .iter()
        .filter(|t| t.role == "user")
        .map(|t| t.node_id.clone())
        .collect();
    let mut properties = serde_json::Map::new();
    for id in &user_ids {
        properties.insert(id.clone(), json!({"type":"array","minItems":1,"maxItems":16,"items":{"type":"object","additionalProperties":false,"properties":{"disposition":{"type":"string","enum":["retain","omit","superseded"]},"authority_kind":{"type":"string","enum":["direct","correction","adoption","retention","none"]},"category":{"type":"string","enum":["fact","preference","decision","instruction","relationship","constraint","correction","commitment","none"]},"type":{"type":"string","enum":["identity","education","employment","location","possession","health","finance","schedule","communication","project","process","product","relationship","other","none"]},"lifecycle":{"type":"string","enum":["current","future","historical","superseded","none"]},"proposition":{"type":"string"},"authority_source_node_id":{"type":"string","enum":assistants},"grounding_source_node_id":{"type":"string","enum":support},"reason":{"type":"string"}},"required":["disposition","authority_kind","category","type","lifecycle","proposition","authority_source_node_id","grounding_source_node_id","reason"]}}));
    }
    let req = if allow_requests {
        json!({"type":"array","maxItems":4,"items":{"type":"object","additionalProperties":false,"properties":{"kind":{"type":"string","enum":["turn","conversation_range","archive_search"]},"conversation_id":{"type":"string"},"node_id":{"type":"string"},"start_node_id":{"type":"string"},"end_node_id":{"type":"string"},"query":{"type":"string"},"limit":{"type":"integer","minimum":0,"maximum":5}},"required":["kind","conversation_id","node_id","start_node_id","end_node_id","query","limit"]}})
    } else {
        json!({"type":"array","maxItems":0})
    };
    json!({"type":"object","additionalProperties":false,"properties":{"turns":{"type":"object","additionalProperties":false,"properties":properties,"required":user_ids},"evidence_requests":req},"required":["turns","evidence_requests"]})
}

fn validate_ledger(value: &Value, turns: &[ResolvedTurn]) -> Result<(), String> {
    let map = value
        .get("turns")
        .and_then(Value::as_object)
        .ok_or("ledger requires turns object")?;
    let expected: std::collections::HashSet<_> = turns
        .iter()
        .filter(|t| t.role == "user")
        .map(|t| t.node_id.as_str())
        .collect();
    if map.len() != expected.len() || map.keys().any(|id| !expected.contains(id.as_str())) {
        return Err("ledger must account for every user turn and no other turn".into());
    }
    for clauses in map.values() {
        let clauses = clauses
            .as_array()
            .ok_or("ledger turn clauses must be an array")?;
        if clauses.is_empty() || clauses.len() > 16 {
            return Err("ledger clause count out of range".into());
        }
    }
    Ok(())
}

fn retained_clauses(ledger: &Value) -> Result<Vec<Value>, String> {
    let mut retained = Vec::new();
    for (node, clauses) in ledger["turns"].as_object().ok_or("ledger requires turns")? {
        for clause in clauses.as_array().ok_or("clauses must be an array")? {
            if clause["disposition"].as_str() == Some("retain") {
                let mut item = clause.clone();
                item["source_node_id"] = Value::String(node.clone());
                retained.push(item);
            }
        }
    }
    Ok(retained)
}

fn candidate_schema(turns: &[ResolvedTurn], evidence: &[EvidenceTurn]) -> Value {
    let users: Vec<_> = turns
        .iter()
        .filter(|t| t.role == "user")
        .map(|t| t.node_id.clone())
        .collect();
    let mut all_ids: Vec<_> = turns
        .iter()
        .map(|t| t.node_id.clone())
        .chain(evidence.iter().map(|t| t.node_id.clone()))
        .collect();
    all_ids.push(String::new());
    let mut assistants: Vec<_> = turns
        .iter()
        .filter(|t| t.role == "assistant")
        .map(|t| t.node_id.clone())
        .chain(
            evidence
                .iter()
                .filter(|t| t.role == "assistant")
                .map(|t| t.node_id.clone()),
        )
        .collect();
    assistants.push(String::new());
    json!({"type":"object","additionalProperties":false,"properties":{"candidates":{"type":"array","maxItems":64,"items":{"type":"object","additionalProperties":false,"properties":{"authority_kind":{"type":"string","enum":["direct","correction","adoption","retention"]},"category":{"type":"string","enum":["fact","preference","decision","instruction","relationship","constraint","correction","commitment"]},"memory_type":{"type":"string","enum":["identity","education","employment","location","possession","health","finance","schedule","communication","project","process","product","relationship","other"]},"temporal_status":{"type":"string","enum":["current","future","historical"]},"ownership":{"type":"string","enum":["user","project"]},"title":{"type":"string"},"content":{"type":"string"},"source_node_id":{"type":"string","enum":users},"source_quote":{"type":"string"},"authority_source":{"type":["object","null"],"properties":{"conversation_id":{"type":"string"},"node_id":{"type":"string","enum":assistants},"quote":{"type":"string"}},"required":["conversation_id","node_id","quote"],"additionalProperties":false},"grounding_source":{"type":["object","null"],"properties":{"conversation_id":{"type":"string"},"node_id":{"type":"string","enum":all_ids},"quote":{"type":"string"}},"required":["conversation_id","node_id","quote"],"additionalProperties":false},"routing_mentions":{"type":"array","maxItems":32,"items":{"type":"object","additionalProperties":false,"properties":{"field":{"type":"string","enum":["title","content"]},"start_byte":{"type":"integer","minimum":0},"end_byte":{"type":"integer","minimum":0},"text":{"type":"string"}},"required":["field","start_byte","end_byte","text"]}}},"required":["authority_kind","category","memory_type","temporal_status","ownership","title","content","source_node_id","source_quote","authority_source","grounding_source","routing_mentions"]}}},"required":["candidates"]})
}

fn parse_candidate(v: &Value) -> Result<RawCandidate, String> {
    let s = |k: &str| {
        v.get(k)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("candidate missing string {k}"))
    };
    let attribution = |key: &str| -> Result<Option<SourceAttribution>, String> {
        match &v[key] {
            Value::Null => Ok(None),
            Value::Object(a) => Ok(Some(SourceAttribution {
                conversation_id: a["conversation_id"]
                    .as_str()
                    .ok_or("attribution conversation_id missing")?
                    .to_owned(),
                node_id: a["node_id"]
                    .as_str()
                    .ok_or("attribution node_id missing")?
                    .to_owned(),
                quote: a["quote"]
                    .as_str()
                    .ok_or("attribution quote missing")?
                    .to_owned(),
            })),
            _ => Err(format!("{key} must be object or null")),
        }
    };
    let mentions = v["routing_mentions"]
        .as_array()
        .ok_or("routing_mentions must be an array")?
        .iter()
        .map(|m| {
            Ok(RoutingMention {
                field: match m["field"].as_str() {
                    Some("title") => TextField::Title,
                    Some("content") => TextField::Content,
                    _ => return Err("unknown routing mention field".into()),
                },
                start_byte: m["start_byte"]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or("invalid mention start")?,
                end_byte: m["end_byte"]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or("invalid mention end")?,
                text: m["text"].as_str().ok_or("mention text missing")?.to_owned(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(RawCandidate {
        authority_kind: s("authority_kind")?,
        category: s("category")?,
        memory_type: s("memory_type")?,
        temporal_status: s("temporal_status")?,
        ownership: match s("ownership")?.as_str() {
            "user" => InsomniaOwnership::User,
            "project" => InsomniaOwnership::Project,
            _ => return Err("unknown ownership".into()),
        },
        title: s("title")?,
        content: s("content")?,
        source_node_id: s("source_node_id")?,
        source_quote: s("source_quote")?,
        authority_source: attribution("authority_source")?,
        grounding_source: attribution("grounding_source")?,
        routing_mentions: mentions,
    })
}

fn evidence_turn_json(t: &EvidenceTurn) -> Value {
    json!({"conversation_id":t.conversation_id,"node_id":t.node_id,"role":t.role,"timestamp_ns":t.timestamp_ns,"content":t.content})
}
fn evidence_result_json(r: &EvidenceResult) -> Value {
    json!({"kind":r.kind,"turns":r.turns.iter().map(evidence_turn_json).collect::<Vec<_>>(),"error":r.error,"truncated":r.truncated})
}
fn endpoint_error(e: GeneralEndpointError) -> String {
    e.to_string()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_archive::{EpisodeBoundary, EpisodeId, EpisodeOrigin};
    use std::sync::Mutex;

    struct FakeEndpoint(Mutex<Vec<Value>>);
    impl GeneralEndpoint for FakeEndpoint {
        fn model(&self) -> &str {
            "test-model"
        }
        fn complete_json(
            &self,
            _prompt: &str,
            _payload: &str,
            _name: &str,
            _schema: &Value,
        ) -> Result<Value, GeneralEndpointError> {
            self.0
                .lock()
                .unwrap()
                .pop()
                .ok_or(GeneralEndpointError::InvalidResponse(
                    "unexpected completion",
                ))
        }
    }

    fn source() -> (Episode, Vec<ResolvedTurn>) {
        (
            Episode {
                id: EpisodeId([7; 32]),
                conversation_id: "c".into(),
                start_node_id: "u1".into(),
                end_node_id: "u1".into(),
                origin: EpisodeOrigin::Live,
                boundary: EpisodeBoundary::Explicit,
                source_through_ns: 1,
                finalized_at_ns: 1,
            },
            vec![ResolvedTurn {
                node_id: "u1".into(),
                role: "user".into(),
                principal_id: None,
                timestamp_ns: 1,
                content: "I prefer concise answers.".into(),
            }],
        )
    }
    fn ledger(request: Value) -> Value {
        json!({"turns":{"u1":[{"disposition":"retain","authority_kind":"direct","category":"preference","type":"communication","lifecycle":"current","proposition":"The user prefers concise answers.","authority_source_node_id":"","grounding_source_node_id":"","reason":"direct assertion"}]},"evidence_requests":request})
    }
    fn wording() -> Value {
        json!({"candidates":[{"authority_kind":"direct","category":"preference","memory_type":"communication","temporal_status":"current","ownership":"user","title":"Concise answers","content":"The user prefers concise answers.","source_node_id":"u1","source_quote":"I prefer concise answers.","authority_source":null,"grounding_source":null,"routing_mentions":[]}]})
    }

    #[test]
    fn no_evidence_path_runs_ledger_then_grounded_wording() {
        let (episode, turns) = source();
        let endpoint = FakeEndpoint(Mutex::new(vec![wording(), ledger(json!([]))]));
        let extractor = InsomniaModelExtractor::new(endpoint);
        let stage = extractor.start(&episode, &turns).unwrap();
        assert!(stage.requests.is_empty());
        let result = extractor
            .finish_without_evidence(&episode, &turns, stage)
            .unwrap();
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].source_node_id, "u1");
    }

    #[test]
    fn no_evidence_stage_rejects_changed_episode_or_hidden_requests() {
        let (episode, turns) = source();
        let extractor =
            InsomniaModelExtractor::new(FakeEndpoint(Mutex::new(vec![ledger(json!([]))])));
        let stage = extractor.start(&episode, &turns).unwrap();
        let mut altered = turns.clone();
        altered[0].content.push_str(" changed");
        assert!(
            extractor
                .finish_without_evidence(&episode, &altered, stage.clone())
                .is_err()
        );
        let mut forged = stage;
        forged.initial_ledger["evidence_requests"] = json!([{"kind":"turn","conversation_id":"c","node_id":"old","start_node_id":"","end_node_id":"","query":"q","limit":1}]);
        assert!(
            extractor
                .finish_without_evidence(&episode, &turns, forged)
                .is_err()
        );
    }

    #[test]
    fn only_one_exact_bounded_evidence_round_is_allowed() {
        let (episode, turns) = source();
        let request = json!([{"kind":"turn","conversation_id":"c","node_id":"old","start_node_id":"","end_node_id":"","query":"proposal","limit":1}]);
        let endpoint = FakeEndpoint(Mutex::new(vec![
            wording(),
            ledger(json!([])),
            ledger(request),
        ]));
        let extractor = InsomniaModelExtractor::new(endpoint);
        let stage = extractor.start(&episode, &turns).unwrap();
        assert_eq!(stage.requests.len(), 1);
        let fact = EvidenceTurn {
            conversation_id: "c".into(),
            node_id: "old".into(),
            role: "assistant".into(),
            timestamp_ns: 0,
            content: "Maybe use concise answers.".into(),
        };
        let result = EvidenceResult {
            kind: "turn".into(),
            turns: vec![fact.clone()],
            error: None,
            truncated: false,
        };
        let extraction = extractor
            .finish_evidence(&episode, &turns, stage, &[result], &[fact])
            .unwrap();
        assert_eq!(extraction.candidates.len(), 1);
    }

    #[test]
    fn final_ledger_cannot_open_a_second_evidence_round() {
        let (episode, turns) = source();
        let request = json!([{"kind":"turn","conversation_id":"c","node_id":"old","start_node_id":"","end_node_id":"","query":"proposal","limit":1}]);
        let endpoint = FakeEndpoint(Mutex::new(vec![ledger(request.clone()), ledger(request)]));
        let extractor = InsomniaModelExtractor::new(endpoint);
        let stage = extractor.start(&episode, &turns).unwrap();
        let fact = EvidenceTurn {
            conversation_id: "c".into(),
            node_id: "old".into(),
            role: "assistant".into(),
            timestamp_ns: 0,
            content: "An earlier proposal.".into(),
        };
        let result = EvidenceResult {
            kind: "turn".into(),
            turns: vec![fact.clone()],
            error: None,
            truncated: false,
        };
        let error = extractor
            .finish_evidence(&episode, &turns, stage, &[result], &[fact])
            .unwrap_err();
        assert!(error.contains("second evidence round"));
    }
}
