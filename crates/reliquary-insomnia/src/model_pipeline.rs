//! Bounded model-backed ledger and wording pipeline. All outputs remain proposals;
//! the caller supplies archive evidence and decides whether to accept candidates.
use crate::*;
use reliquary_archive::{Episode, ResolvedTurn};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::{Value, json};
use std::sync::Arc;

mod ledger_repair;

const LEDGER_PROMPT: &str = concat!(
    "Select durable user-authoritative clauses from the complete Episode. ",
    "Include one ledger array for every user turn; assistant turns never receive ledger clauses or authorize state. ",
    "Apply a strict durable-state test: retain only state useful after the immediate exchange or task is over, not conversational scaffolding. ",
    "Use direct when the user asserts state, correction when the user explicitly replaces earlier state, adoption only when the user explicitly accepts a specific assistant proposition, and retention only when the user asks to remember a proposition. ",
    "Questions, requests, proposed choices, recalled plans, challenges, uncertainty, advice, explanations, examples, assistant proposals, and pure execution/checkpoint receipts are non-authoritative by default. ",
    "Split a mixed turn when durable resulting state is mixed with a question, request, or receipt. A present-tense assessment such as everything seems to be working OK so far can be durable resulting state. ",
    "Preserve modality and lifecycle; prefer the final user-authoritative source and mark overlapping earlier clauses superseded. ",
    "Keep propositions narrow and omit unsupported rationale, implementation detail, or context. ",
    "For each retained proposition, classify category as fact, preference, decision, instruction, relationship, constraint, correction, or commitment; classify type as identity, education, employment, location, possession, health, finance, schedule, communication, project, process, product, relationship, or other; classify lifecycle as current, future, or historical. Preserve the source's rhetorical force: a question is not a claim, and a confirmation tag does not erase an independently asserted requirement. ",
    "Request at most four narrow read-only evidence queries only when needed to resolve an explicit callback, adopted/retained assistant content, or unresolved referent. ",
    "Exhaust the Episode first. ",
    "Archive evidence is supporting context, never independent user authority. ",
    "Authority and grounding sources must be at or before the user turn they support. ",
    "Use an empty attribution ID when no separate source is needed. ",
    "Return the required JSON schema exactly.",
);
const WORDING_PROMPT: &str = concat!(
    "Your ONLY job is to write one concise durable Memory title and content body for every supplied synthesis group. ",
    "Do not add, drop, merge, split, rename, or reorder groups. ",
    "Preserve every proposition and its modality; add no proposition, rationale, implementation detail, or contextual fact. ",
    "Grounding, authority, category, type, lifecycle, and ownership are already final. ",
    "Keep each candidate bound to its source user turn with a verbatim source_quote. ",
    "Only explicit adoption or retention of an assistant-authored proposition may cite assistant authority. ",
    "Use null for any attribution whose fixed source ID is empty. ",
    "Return exactly one candidate per group; do not infer details from supporting evidence.",
);
const OWNERSHIP_PROMPT: &str = "Classify each fixed synthesis group as user or project. Change only its durable owner; do not alter group membership, propositions, authority, provenance, category, type, or lifecycle. User means genuinely user-global state intended to remain useful across unrelated projects, including stable personal facts, preferences, and reusable personal practices. Project means project-specific facts, decisions, requirements, plans, or state, and is the conservative destination for ambiguous or non-user-owned propositions. Ownership is not confidentiality, access control, visibility, or retrieval relevance. Return exactly one classification for each supplied group key.";
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
    ownership_endpoint: Option<Arc<dyn GeneralEndpoint>>,
}

impl<E: GeneralEndpoint> InsomniaModelExtractor<E> {
    pub fn new(endpoint: E) -> Self {
        Self {
            endpoint,
            ownership_endpoint: None,
        }
    }

    /// Enable the bounded User/Project routing classifier. Without this route,
    /// all extracted drafts conservatively remain Project-owned.
    pub fn with_ownership_endpoint<M: GeneralEndpoint + 'static>(mut self, endpoint: M) -> Self {
        self.ownership_endpoint = Some(Arc::new(endpoint));
        self
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
        validate_ledger(&stage.initial_ledger, turns, &[])?;
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
        validate_ledger(&stage.initial_ledger, turns, &[])?;
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
        validate_ledger(&response, turns, evidence)?;
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
        let mut retained = retained_groups(ledger, turns)?;
        if retained.is_empty() {
            return Ok(InsomniaExtraction {
                candidates: Vec::new(),
                rejected: Vec::new(),
            });
        }
        if let Some(endpoint) = &self.ownership_endpoint {
            classify_ownership(endpoint.as_ref(), payload, &mut retained)?;
        }
        let input = serde_json::to_string(&json!({"authoritative_episode":payload,"synthesis_groups":retained,"supporting_evidence":evidence.iter().map(evidence_turn_json).collect::<Vec<_>>()})).map_err(|e| e.to_string())?;
        let schema = candidate_schema(&retained);
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
        if raw.len() != retained.len()
            || raw
                .iter()
                .zip(&retained)
                .any(|(candidate, group)| !matches_retained_clause(candidate, group))
        {
            return Err("wording response differs from retained ledger clauses".into());
        }
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
    let user_ids = turns
        .iter()
        .filter(|t| t.role == "user")
        .map(|t| t.node_id.clone())
        .collect::<Vec<_>>();
    ledger_repair::turn_schema(turns, evidence, &user_ids, allow_requests)
}

fn validate_ledger(
    value: &Value,
    turns: &[ResolvedTurn],
    evidence: &[EvidenceTurn],
) -> Result<(), String> {
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
    for turn in turns.iter().filter(|t| t.role == "user") {
        let clauses = map[&turn.node_id]
            .as_array()
            .ok_or("ledger turn clauses must be an array")?;
        if clauses.is_empty() || clauses.len() > 16 {
            return Err("ledger clause count out of range".into());
        }
        let authority =
            ledger_repair::eligible_attribution_ids(turns, evidence, turn.timestamp_ns, true);
        let grounding =
            ledger_repair::eligible_attribution_ids(turns, evidence, turn.timestamp_ns, false);
        for clause in clauses.iter().filter(|c| c["disposition"] == "retain") {
            if !clause["authority_source_node_id"]
                .as_str()
                .is_some_and(|id| authority.contains(&id))
                || !clause["grounding_source_node_id"]
                    .as_str()
                    .is_some_and(|id| grounding.contains(&id))
            {
                return Err(
                    "ledger attribution is outside the user turn's eligible evidence".into(),
                );
            }
        }
    }
    Ok(())
}

fn retained_groups(ledger: &Value, turns: &[ResolvedTurn]) -> Result<Vec<Value>, String> {
    let mut groups: Vec<Value> = Vec::new();
    for turn in turns.iter().filter(|turn| turn.role == "user") {
        let node = &turn.node_id;
        let clauses = &ledger["turns"][node];
        for clause in clauses.as_array().ok_or("clauses must be an array")? {
            if clause["disposition"].as_str() == Some("retain") {
                let signature = [
                    node.as_str(),
                    clause["authority_kind"].as_str().unwrap_or_default(),
                    clause["category"].as_str().unwrap_or_default(),
                    clause["type"].as_str().unwrap_or_default(),
                    clause["lifecycle"].as_str().unwrap_or_default(),
                    clause["authority_source_node_id"]
                        .as_str()
                        .unwrap_or_default(),
                    clause["grounding_source_node_id"]
                        .as_str()
                        .unwrap_or_default(),
                ];
                let existing = groups.iter_mut().find(|group| {
                    let candidate = [
                        group["source_node_id"].as_str().unwrap_or_default(),
                        group["authority_kind"].as_str().unwrap_or_default(),
                        group["category"].as_str().unwrap_or_default(),
                        group["type"].as_str().unwrap_or_default(),
                        group["lifecycle"].as_str().unwrap_or_default(),
                        group["authority_source_node_id"]
                            .as_str()
                            .unwrap_or_default(),
                        group["grounding_source_node_id"]
                            .as_str()
                            .unwrap_or_default(),
                    ];
                    signature == candidate
                });
                if let Some(group) = existing {
                    group["propositions"]
                        .as_array_mut()
                        .ok_or("synthesis group propositions must be an array")?
                        .push(clause["proposition"].clone());
                } else {
                    if groups.len() == MAX_INSOMNIA_CANDIDATES {
                        return Err("retained ledger requires too many synthesis groups".into());
                    }
                    groups.push(json!({"group_id":format!("g{:03}",groups.len()),"source_node_id":node,"authority_kind":clause["authority_kind"],"category":clause["category"],"type":clause["type"],"lifecycle":clause["lifecycle"],"ownership":"project","authority_source_node_id":clause["authority_source_node_id"],"grounding_source_node_id":clause["grounding_source_node_id"],"propositions":[clause["proposition"]]}));
                }
            }
        }
    }
    Ok(groups)
}

fn classify_ownership(
    endpoint: &dyn GeneralEndpoint,
    episode: &Value,
    groups: &mut [Value],
) -> Result<(), String> {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::with_capacity(groups.len());
    for group in groups.iter() {
        let id = group["group_id"]
            .as_str()
            .ok_or("synthesis group lacks ID")?;
        required.push(Value::String(id.to_owned()));
        properties.insert(
            id.to_owned(),
            json!({"type":"string","enum":["user","project"]}),
        );
    }
    let schema = json!({"type":"object","additionalProperties":false,"properties":{"ownership":{"type":"object","additionalProperties":false,"properties":properties,"required":required}},"required":["ownership"]});
    let payload =
        serde_json::to_string(&json!({"authoritative_episode":episode,"synthesis_groups":groups}))
            .map_err(|error| error.to_string())?;
    if payload.len() > MAX_MODEL_JSON_BYTES {
        return Err("ownership input exceeds bounded JSON limit".into());
    }
    let output = endpoint
        .complete_json(
            OWNERSHIP_PROMPT,
            &payload,
            "insomnia_memory_ownership",
            &schema,
        )
        .map_err(endpoint_error)?;
    let output_object = output
        .as_object()
        .ok_or("ownership response must be an object")?;
    if output_object.len() != 1 || !output_object.contains_key("ownership") {
        return Err("ownership response contains unexpected fields".into());
    }
    let ownership = output
        .get("ownership")
        .and_then(Value::as_object)
        .ok_or("ownership response requires ownership object")?;
    if ownership.len() != groups.len() {
        return Err("ownership group count mismatch".into());
    }
    for group in groups {
        let id = group["group_id"]
            .as_str()
            .ok_or("synthesis group lacks ID")?;
        let owner = ownership
            .get(id)
            .and_then(Value::as_str)
            .ok_or("ownership response omits group")?;
        if !matches!(owner, "user" | "project") {
            return Err("ownership response contains unsupported owner".into());
        }
        group["ownership"] = Value::String(owner.to_owned());
    }
    Ok(())
}

fn matches_retained_clause(candidate: &RawCandidate, clause: &Value) -> bool {
    let fields = [
        ("source_node_id", candidate.source_node_id.as_str()),
        ("authority_kind", candidate.authority_kind.as_str()),
        ("category", candidate.category.as_str()),
        ("type", candidate.memory_type.as_str()),
        ("lifecycle", candidate.temporal_status.as_str()),
        (
            "ownership",
            match candidate.ownership {
                InsomniaOwnership::User => "user",
                InsomniaOwnership::Project => "project",
            },
        ),
        (
            "authority_source_node_id",
            candidate
                .authority_source
                .as_ref()
                .map_or("", |source| &source.node_id),
        ),
        (
            "grounding_source_node_id",
            candidate
                .grounding_source
                .as_ref()
                .map_or("", |source| &source.node_id),
        ),
    ];
    fields
        .into_iter()
        .all(|(key, expected)| clause[key].as_str() == Some(expected))
}

fn candidate_schema(groups: &[Value]) -> Value {
    let ids = |field: &str| {
        groups
            .iter()
            .filter_map(|group| group[field].as_str())
            .collect::<std::collections::BTreeSet<_>>()
    };
    let users = ids("source_node_id");
    let assistants = ids("authority_source_node_id");
    let support = ids("grounding_source_node_id");
    json!({"type":"object","additionalProperties":false,"properties":{"candidates":{"type":"array","maxItems":64,"items":{"type":"object","additionalProperties":false,"properties":{"authority_kind":{"type":"string","enum":["direct","correction","adoption","retention"]},"category":{"type":"string","enum":["fact","preference","decision","instruction","relationship","constraint","correction","commitment"]},"memory_type":{"type":"string","enum":["identity","education","employment","location","possession","health","finance","schedule","communication","project","process","product","relationship","other"]},"temporal_status":{"type":"string","enum":["current","future","historical"]},"ownership":{"type":"string","enum":["user","project"]},"title":{"type":"string"},"content":{"type":"string"},"source_node_id":{"type":"string","enum":users},"source_quote":{"type":"string"},"authority_source":{"type":["object","null"],"properties":{"conversation_id":{"type":"string"},"node_id":{"type":"string","enum":assistants},"quote":{"type":"string"}},"required":["conversation_id","node_id","quote"],"additionalProperties":false},"grounding_source":{"type":["object","null"],"properties":{"conversation_id":{"type":"string"},"node_id":{"type":"string","enum":support},"quote":{"type":"string"}},"required":["conversation_id","node_id","quote"],"additionalProperties":false},"routing_mentions":{"type":"array","maxItems":32,"items":{"type":"object","additionalProperties":false,"properties":{"field":{"type":"string","enum":["title","content"]},"start_byte":{"type":"integer","minimum":0},"end_byte":{"type":"integer","minimum":0},"text":{"type":"string"}},"required":["field","start_byte","end_byte","text"]}}},"required":["authority_kind","category","memory_type","temporal_status","ownership","title","content","source_node_id","source_quote","authority_source","grounding_source","routing_mentions"]}}},"required":["candidates"]})
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

    #[test]
    fn frozen_extraction_classes_and_wording_constraints_are_explicit() {
        for clause in [
            "strict durable-state test",
            "correction when the user explicitly replaces earlier state",
            "adoption only when the user explicitly accepts a specific assistant proposition",
            "pure execution/checkpoint receipts are non-authoritative by default",
            "mark overlapping earlier clauses superseded",
            "Archive evidence is supporting context, never independent user authority",
            "classify category as fact, preference, decision, instruction, relationship, constraint, correction, or commitment",
        ] {
            assert!(
                LEDGER_PROMPT.contains(clause),
                "missing ledger class rule: {clause}"
            );
        }
        for clause in [
            "Your ONLY job is to write one concise durable Memory title and content body",
            "Do not add, drop, merge, split, rename, or reorder groups",
            "Grounding, authority, category, type, lifecycle, and ownership are already final",
        ] {
            assert!(
                WORDING_PROMPT.contains(clause),
                "missing wording rule: {clause}"
            );
        }
        assert!(OWNERSHIP_PROMPT.contains("User means genuinely user-global state"));
    }

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
        json!({"candidates":[{"authority_kind":"direct","category":"preference","memory_type":"communication","temporal_status":"current","ownership":"project","title":"Concise answers","content":"The user prefers concise answers.","source_node_id":"u1","source_quote":"I prefer concise answers.","authority_source":null,"grounding_source":null,"routing_mentions":[]}]})
    }

    struct GroupEndpoint(Mutex<Option<Value>>);
    impl GeneralEndpoint for GroupEndpoint {
        fn model(&self) -> &str {
            "test-model"
        }
        fn complete_json(
            &self,
            _prompt: &str,
            payload: &str,
            name: &str,
            _schema: &Value,
        ) -> Result<Value, GeneralEndpointError> {
            if name == "insomnia_memory_wording" {
                *self.0.lock().unwrap() = Some(serde_json::from_str(payload).unwrap());
                Ok(wording())
            } else {
                let mut ledger = ledger(json!([]));
                ledger["turns"]["u1"].as_array_mut().unwrap().push(json!({"disposition":"retain","authority_kind":"direct","category":"preference","type":"communication","lifecycle":"current","proposition":"The user values predictable formatting.","authority_source_node_id":"","grounding_source_node_id":"","reason":"direct assertion"}));
                Ok(ledger)
            }
        }
    }

    #[test]
    fn equal_authority_clauses_are_grouped_before_wording() {
        let (episode, turns) = source();
        let endpoint = GroupEndpoint(Mutex::new(None));
        let extractor = InsomniaModelExtractor::new(endpoint);
        let stage = extractor.start(&episode, &turns).unwrap();
        let extraction = extractor
            .finish_without_evidence(&episode, &turns, stage)
            .unwrap();
        assert_eq!(extraction.candidates.len(), 1);
        let payload = extractor.endpoint.0.lock().unwrap().clone().unwrap();
        let groups = payload["synthesis_groups"].as_array().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["propositions"].as_array().unwrap().len(), 2);
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
        assert_eq!(result.candidates[0].ownership, InsomniaOwnership::Project);
    }

    #[test]
    fn ownership_is_project_by_default_and_user_only_with_dedicated_classification() {
        let (episode, turns) = source();
        let mut default_wording = wording();
        default_wording["candidates"][0]["ownership"] = json!("user");
        let extractor = InsomniaModelExtractor::new(FakeEndpoint(Mutex::new(vec![
            default_wording,
            ledger(json!([])),
        ])));
        let stage = extractor.start(&episode, &turns).unwrap();
        assert!(
            extractor
                .finish_without_evidence(&episode, &turns, stage)
                .is_err()
        );

        let mut routed_wording = wording();
        routed_wording["candidates"][0]["ownership"] = json!("user");
        let main = FakeEndpoint(Mutex::new(vec![routed_wording, ledger(json!([]))]));
        let owner = FakeEndpoint(Mutex::new(vec![json!({"ownership":{"g000":"user"}})]));
        let extractor = InsomniaModelExtractor::new(main).with_ownership_endpoint(owner);
        let stage = extractor.start(&episode, &turns).unwrap();
        let result = extractor
            .finish_without_evidence(&episode, &turns, stage)
            .unwrap();
        assert_eq!(result.candidates[0].ownership, InsomniaOwnership::User);
    }

    fn chronological_source() -> (Episode, Vec<ResolvedTurn>, Vec<EvidenceTurn>) {
        let (mut episode, mut turns) = source();
        turns[0].timestamp_ns = 2;
        for (node_id, role, timestamp_ns) in [
            ("a1", "assistant", 3),
            ("u2", "user", 4),
            ("a2", "assistant", 5),
        ] {
            turns.push(ResolvedTurn {
                node_id: node_id.into(),
                role: role.into(),
                timestamp_ns,
                principal_id: None,
                content: "test".into(),
            });
        }
        episode.end_node_id = "a2".into();
        let evidence = [("earlier", 1), ("later", 6)]
            .into_iter()
            .map(|(id, timestamp_ns)| EvidenceTurn {
                conversation_id: "archive".into(),
                node_id: id.into(),
                role: "assistant".into(),
                timestamp_ns,
                content: "test".into(),
            })
            .collect();
        (episode, turns, evidence)
    }

    #[test]
    fn attribution_schema_is_scoped_to_each_user_turn_in_ledger_and_repairs() {
        let (_, turns, evidence) = chronological_source();
        let schema = ledger_schema(&turns, &evidence, false);
        let turns_schema = &schema["properties"]["turns"]["properties"];
        assert_eq!(
            turns_schema["u1"]["items"]["properties"]["authority_source_node_id"]["enum"],
            json!(["earlier", ""])
        );
        assert_eq!(
            turns_schema["u1"]["items"]["properties"]["grounding_source_node_id"]["enum"],
            json!(["u1", "earlier", ""])
        );
        assert_eq!(
            turns_schema["u2"]["items"]["properties"]["authority_source_node_id"]["enum"],
            json!(["a1", "earlier", ""])
        );
        assert_eq!(
            turns_schema["u2"]["items"]["properties"]["grounding_source_node_id"]["enum"],
            json!(["u1", "a1", "u2", "earlier", ""])
        );

        let mut ledger = ledger(json!([]));
        ledger["turns"]["u2"] = ledger["turns"]["u1"].clone();
        ledger["turns"]["u2"][0]["grounding_source_node_id"] = Value::Null;
        let missing = ledger_repair::missing_clause_fields(&ledger, &turns).unwrap();
        let repair = ledger_repair::clause_field_schema(&turns, &evidence, &missing);
        assert_eq!(
            repair["properties"]["repairs"]["properties"]["r000"]["properties"]["grounding_source_node_id"]
                ["enum"],
            json!(["u1", "a1", "u2", "earlier", ""])
        );
    }

    #[test]
    fn wording_is_bound_to_retained_attributions_and_ledger_rejects_future_sources() {
        let (episode, turns, evidence) = chronological_source();
        let mut ledger = ledger(json!([]));
        ledger["turns"]["u2"] = ledger["turns"]["u1"].clone();
        ledger["turns"]["u1"][0]["grounding_source_node_id"] = json!("u1");
        ledger["turns"]["u2"][0]["grounding_source_node_id"] = json!("a1");
        validate_ledger(&ledger, &turns, &evidence).unwrap();
        let schema = candidate_schema(&retained_groups(&ledger, &turns).unwrap());
        let props = &schema["properties"]["candidates"]["items"]["properties"];
        assert_eq!(
            props["grounding_source"]["properties"]["node_id"]["enum"],
            json!(["a1", "u1"])
        );
        assert_eq!(
            props["authority_source"]["properties"]["node_id"]["enum"],
            json!([""])
        );

        ledger["turns"]["u1"][0]["grounding_source_node_id"] = json!("a1");
        let extractor = InsomniaModelExtractor::new(FakeEndpoint(Mutex::new(vec![ledger])));
        assert_eq!(
            extractor.start(&episode, &turns),
            Err("ledger attribution is outside the user turn's eligible evidence".into())
        );
    }

    #[test]
    fn omitted_and_superseded_clauses_never_reach_wording() {
        let (episode, turns) = source();
        let mut disposition = ledger(json!([]));
        let mut superseded = disposition["turns"]["u1"][0].clone();
        superseded["disposition"] = json!("superseded");
        disposition["turns"]["u1"][0]["disposition"] = json!("omit");
        disposition["turns"]["u1"]
            .as_array_mut()
            .unwrap()
            .push(superseded);
        let extractor = InsomniaModelExtractor::new(FakeEndpoint(Mutex::new(vec![disposition])));
        let stage = extractor.start(&episode, &turns).unwrap();
        let extraction = extractor
            .finish_without_evidence(&episode, &turns, stage)
            .unwrap();
        assert!(extraction.candidates.is_empty());
        assert!(extraction.rejected.is_empty());
    }

    #[test]
    fn wording_cannot_drop_add_or_reclassify_retained_clauses() {
        let (episode, turns) = source();
        let mut added = wording();
        added["candidates"]
            .as_array_mut()
            .unwrap()
            .push(wording()["candidates"][0].clone());
        let mut changed = wording();
        changed["candidates"][0]["temporal_status"] = json!("future");
        for response in [json!({"candidates": []}), added, changed] {
            let endpoint = FakeEndpoint(Mutex::new(vec![response, ledger(json!([]))]));
            let extractor = InsomniaModelExtractor::new(endpoint);
            let stage = extractor.start(&episode, &turns).unwrap();
            assert_eq!(
                extractor.finish_without_evidence(&episode, &turns, stage),
                Err("wording response differs from retained ledger clauses".into())
            );
        }
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
