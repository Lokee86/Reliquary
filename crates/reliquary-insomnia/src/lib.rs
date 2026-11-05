//! Pure Insomnia validation over a caller-supplied authoritative Episode and
//! explicitly resolved evidence. This crate does not read Archive or publish Memory.

use reliquary_archive::{Episode, ResolvedTurn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

mod candidate_policy;
mod candidate_receipt_policy;
mod candidate_text;
mod model_pipeline;
pub use candidate_policy::validate_semantic_authority;
pub use model_pipeline::{InsomniaModelExtractor, InsomniaModelStage};

pub const INSOMNIA_EXTRACTOR_CONTRACT_VERSION: &str = "v3-4";
pub const MAX_INSOMNIA_CANDIDATES: usize = 64;
pub const MAX_INSOMNIA_EVIDENCE_REQUESTS: usize = 4;
pub const MAX_INSOMNIA_EVIDENCE_TURNS: usize = 64;
pub const MAX_INSOMNIA_EVIDENCE_BYTES: usize = 128 << 10;
pub const MAX_MEMORY_ENTITY_MENTIONS: usize = 32;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum InsomniaOwnership {
    User,
    Project,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TextField {
    Title,
    Content,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RoutingMention {
    pub field: TextField,
    pub start_byte: usize,
    pub end_byte: usize,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceAttribution {
    pub conversation_id: String,
    pub node_id: String,
    pub quote: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceTurn {
    pub conversation_id: String,
    pub node_id: String,
    pub role: String,
    pub timestamp_ns: i64,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceResult {
    pub kind: String,
    pub turns: Vec<EvidenceTurn>,
    pub error: Option<String>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceRequest {
    pub kind: String,
    pub conversation_id: String,
    pub node_id: String,
    pub start_node_id: String,
    pub end_node_id: String,
    pub query: String,
    pub limit: usize,
}

pub fn parse_evidence_requests(value: &Value) -> Result<Vec<EvidenceRequest>, String> {
    let Some(requests) = value.get("evidence_requests") else {
        return Ok(Vec::new());
    };
    let requests = requests
        .as_array()
        .ok_or("evidence_requests must be an array")?;
    if requests.len() > MAX_INSOMNIA_EVIDENCE_REQUESTS {
        return Err("too many evidence requests".into());
    }
    requests
        .iter()
        .map(|value| {
            let request = EvidenceRequest {
                kind: string_field(value, "kind")?.trim().to_lowercase(),
                conversation_id: optional_string(value, "conversation_id")?,
                node_id: optional_string(value, "node_id")?,
                start_node_id: optional_string(value, "start_node_id")?,
                end_node_id: optional_string(value, "end_node_id")?,
                query: optional_string(value, "query")?,
                limit: optional_usize(value, "limit")?,
            };
            validate_evidence_request(&request)?;
            Ok(request)
        })
        .collect()
}

fn validate_evidence_request(request: &EvidenceRequest) -> Result<(), String> {
    if request.limit > 5 {
        return Err("evidence request limit exceeds 5".into());
    }
    match request.kind.as_str() {
        "turn"
            if !request.conversation_id.trim().is_empty() && !request.node_id.trim().is_empty() =>
        {
            Ok(())
        }
        "conversation_range"
            if !request.conversation_id.trim().is_empty()
                && !request.start_node_id.trim().is_empty()
                && !request.end_node_id.trim().is_empty() =>
        {
            Ok(())
        }
        "archive_search"
            if !request.query.trim().is_empty() && request.query.trim().len() <= 512 =>
        {
            Ok(())
        }
        "turn" | "conversation_range" | "archive_search" => {
            Err("evidence request lacks a valid target".into())
        }
        _ => Err("unsupported evidence request kind".into()),
    }
}

/// Validates that per-request references reproduce only the exact bounded facts
/// supplied by the caller's authorized evidence resolver.
pub fn validate_evidence_response(
    requests: &[EvidenceRequest],
    results: &[EvidenceResult],
    authorized_turns: &[EvidenceTurn],
) -> Result<(), String> {
    if requests.len() > MAX_INSOMNIA_EVIDENCE_REQUESTS {
        return Err("too many evidence requests".into());
    }
    if results.len() != requests.len() {
        return Err("evidence result count mismatch".into());
    }
    if authorized_turns.len() > MAX_INSOMNIA_EVIDENCE_TURNS {
        return Err("evidence turn limit exceeded".into());
    }
    let mut authorized = std::collections::HashMap::new();
    let mut bytes = 0usize;
    for turn in authorized_turns {
        if turn.conversation_id.trim().is_empty()
            || turn.node_id.trim().is_empty()
            || turn.role.trim().is_empty()
        {
            return Err("evidence turn lacks source identity".into());
        }
        if authorized
            .insert((turn.conversation_id.as_str(), turn.node_id.as_str()), turn)
            .is_some()
        {
            return Err("duplicate evidence turn identity".into());
        }
        bytes = bytes
            .checked_add(turn.content.len())
            .ok_or("evidence byte count overflow")?;
    }
    if bytes > MAX_INSOMNIA_EVIDENCE_BYTES {
        return Err("evidence byte limit exceeded".into());
    }
    let mut referenced = HashSet::new();
    let mut result_count = 0usize;
    for (request, result) in requests.iter().zip(results) {
        validate_evidence_request(request)?;
        if result.kind != request.kind {
            return Err("evidence kind mismatch".into());
        }
        if result.error.is_some() && !result.turns.is_empty() {
            return Err("failed evidence result contains turns".into());
        }
        if request.kind == "turn"
            && (result.turns.len() > 1
                || result.turns.iter().any(|turn| {
                    turn.conversation_id != request.conversation_id.trim()
                        || turn.node_id != request.node_id.trim()
                }))
        {
            return Err("turn evidence differs from requested target".into());
        }
        if request.kind == "conversation_range"
            && result
                .turns
                .iter()
                .any(|turn| turn.conversation_id != request.conversation_id.trim())
        {
            return Err("range evidence differs from requested conversation".into());
        }
        if request.kind == "archive_search"
            && !request.conversation_id.trim().is_empty()
            && result
                .turns
                .iter()
                .any(|turn| turn.conversation_id != request.conversation_id.trim())
        {
            return Err("search evidence differs from requested conversation".into());
        }
        for turn in &result.turns {
            result_count = result_count
                .checked_add(1)
                .ok_or("evidence result count overflow")?;
            if result_count > MAX_INSOMNIA_EVIDENCE_TURNS * MAX_INSOMNIA_EVIDENCE_REQUESTS {
                return Err("evidence reference limit exceeded".into());
            }
            let key = (turn.conversation_id.as_str(), turn.node_id.as_str());
            if authorized
                .get(&key)
                .is_none_or(|expected| **expected != *turn)
            {
                return Err("evidence result contains unauthorized or changed turn".into());
            }
            referenced.insert(key);
        }
    }
    if referenced.len() != authorized.len() {
        return Err("evidence references do not cover authorized turns".into());
    }
    Ok(())
}
fn string_field(value: &Value, field: &str) -> Result<String, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("missing string field {field}"))
}
fn optional_string(value: &Value, field: &str) -> Result<String, String> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(value) => value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{field} must be a string")),
    }
}
fn optional_usize(value: &Value, field: &str) -> Result<usize, String> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(0),
        Some(value) => value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| format!("{field} must be a non-negative integer")),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawCandidate {
    pub authority_kind: String,
    pub category: String,
    pub memory_type: String,
    pub temporal_status: String,
    pub ownership: InsomniaOwnership,
    pub title: String,
    pub content: String,
    pub source_node_id: String,
    pub source_quote: String,
    pub authority_source: Option<SourceAttribution>,
    pub grounding_source: Option<SourceAttribution>,
    pub routing_mentions: Vec<RoutingMention>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InsomniaCandidate {
    pub authority_kind: String,
    pub category: String,
    pub memory_type: String,
    pub temporal_status: String,
    pub ownership: InsomniaOwnership,
    pub title: String,
    pub content: String,
    pub source_node_id: String,
    pub source_quote: String,
    pub authority_source: Option<SourceAttribution>,
    pub grounding_source: Option<SourceAttribution>,
    pub routing_mentions: Vec<RoutingMention>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InsomniaRejection {
    pub candidate_index: usize,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InsomniaExtraction {
    pub candidates: Vec<InsomniaCandidate>,
    pub rejected: Vec<InsomniaRejection>,
}

/// Validates model proposals against exact Episode turn text and caller-resolved
/// evidence. Candidate membership is bounded and every asserted quote is verbatim.
pub fn validate_candidates(
    episode: &Episode,
    turns: &[ResolvedTurn],
    evidence: &[EvidenceTurn],
    raw: Vec<RawCandidate>,
) -> Result<InsomniaExtraction, String> {
    if raw.len() > MAX_INSOMNIA_CANDIDATES {
        return Err("candidate count exceeds maximum".into());
    }
    if turns.is_empty()
        || turns
            .first()
            .is_none_or(|turn| turn.node_id != episode.start_node_id)
        || turns
            .last()
            .is_none_or(|turn| turn.node_id != episode.end_node_id)
    {
        return Err("authoritative turns do not match Episode bounds".into());
    }
    let mut turn_ids = HashSet::new();
    if turns
        .iter()
        .any(|turn| !turn_ids.insert(turn.node_id.as_str()))
    {
        return Err("duplicate authoritative Episode turn identity".into());
    }
    validate_evidence_facts(evidence)?;
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let mut seen = HashSet::new();
    for (index, candidate) in raw.into_iter().enumerate() {
        let validated = validate_candidate(episode, turns, evidence, candidate);
        match validated {
            Ok(candidate) => {
                let identity = (
                    candidate.source_node_id.clone(),
                    candidate.source_quote.clone(),
                    candidate.title.clone(),
                    candidate.content.clone(),
                );
                if !seen.insert(identity) {
                    rejected.push(InsomniaRejection {
                        candidate_index: index,
                        reason: "duplicate source-grounded candidate".into(),
                    });
                } else {
                    accepted.push(candidate);
                }
            }
            Err(reason) => rejected.push(InsomniaRejection {
                candidate_index: index,
                reason,
            }),
        }
    }
    Ok(InsomniaExtraction {
        candidates: accepted,
        rejected,
    })
}

fn validate_evidence_facts(evidence: &[EvidenceTurn]) -> Result<(), String> {
    if evidence.len() > MAX_INSOMNIA_EVIDENCE_TURNS {
        return Err("evidence turn limit exceeded".into());
    }
    let mut identities = HashSet::new();
    let mut bytes = 0usize;
    for turn in evidence {
        if turn.conversation_id.trim().is_empty()
            || turn.node_id.trim().is_empty()
            || turn.role.trim().is_empty()
            || !identities.insert((turn.conversation_id.as_str(), turn.node_id.as_str()))
        {
            return Err("invalid or duplicate evidence turn identity".into());
        }
        bytes = bytes
            .checked_add(turn.content.len())
            .ok_or("evidence byte count overflow")?;
    }
    if bytes > MAX_INSOMNIA_EVIDENCE_BYTES {
        return Err("evidence byte limit exceeded".into());
    }
    Ok(())
}

fn validate_candidate(
    episode: &Episode,
    turns: &[ResolvedTurn],
    evidence: &[EvidenceTurn],
    raw: RawCandidate,
) -> Result<InsomniaCandidate, String> {
    let authority_kind = canonical_label(&raw.authority_kind);
    let category = canonical_label(&raw.category);
    let memory_type = canonical_label(&raw.memory_type);
    let temporal_status = canonical_label(&raw.temporal_status);
    let source_node_id = raw.source_node_id.trim().to_owned();
    let source_quote = raw.source_quote.trim().to_owned();
    if !["direct", "correction", "adoption", "retention"].contains(&authority_kind.as_str()) {
        return Err("authority kind is not recognized".into());
    }
    if ![
        "fact",
        "preference",
        "decision",
        "instruction",
        "relationship",
        "constraint",
        "correction",
        "commitment",
    ]
    .contains(&category.as_str())
    {
        return Err("category is not recognized".into());
    }
    if ![
        "identity",
        "education",
        "employment",
        "location",
        "possession",
        "health",
        "finance",
        "schedule",
        "communication",
        "project",
        "process",
        "product",
        "relationship",
        "other",
    ]
    .contains(&memory_type.as_str())
    {
        return Err("type is not recognized".into());
    }
    if !["current", "future", "historical"].contains(&temporal_status.as_str()) {
        return Err("temporal status is not recognized".into());
    }
    if raw.title.trim().is_empty() || raw.content.trim().is_empty() {
        return Err("title and content are required".into());
    }
    let Some(source) = turns.iter().find(|turn| turn.node_id == source_node_id) else {
        return Err("source node is outside the authoritative Episode".into());
    };
    if source.role != "user" {
        return Err("source node is not a user authority turn".into());
    }
    if source_quote.is_empty() || !source.content.contains(&source_quote) {
        return Err("source quote is not verbatim from authority turn".into());
    }
    for attribution in [&raw.authority_source, &raw.grounding_source]
        .into_iter()
        .flatten()
    {
        if attribution.conversation_id.trim().is_empty()
            || attribution.node_id.trim().is_empty()
            || attribution.quote.trim().is_empty()
        {
            return Err("source attribution must be complete".into());
        }
        let in_episode = attribution.conversation_id == episode.conversation_id
            && turns.iter().any(|turn| {
                turn.node_id == attribution.node_id && turn.content.contains(&attribution.quote)
            });
        let in_evidence = evidence.iter().any(|turn| {
            turn.conversation_id == attribution.conversation_id
                && turn.node_id == attribution.node_id
                && turn.content.contains(&attribution.quote)
        });
        if !in_episode && !in_evidence {
            return Err("attribution quote is not in supplied authoritative evidence".into());
        }
        let timely_episode = attribution.conversation_id == episode.conversation_id
            && turns.iter().any(|turn| {
                turn.node_id == attribution.node_id
                    && turn.content.contains(&attribution.quote)
                    && turn.timestamp_ns <= source.timestamp_ns
            });
        let timely_evidence = evidence.iter().any(|turn| {
            turn.conversation_id == attribution.conversation_id
                && turn.node_id == attribution.node_id
                && turn.content.contains(&attribution.quote)
                && turn.timestamp_ns <= source.timestamp_ns
        });
        if !timely_episode && !timely_evidence {
            return Err("attribution must not postdate the user authority turn".into());
        }
    }
    if !matches!(authority_kind.as_str(), "adoption" | "retention")
        && raw.authority_source.is_some()
    {
        return Err("only adoption or retention may use assistant authority provenance".into());
    }
    if authority_kind == "adoption" && raw.authority_source.is_none() {
        return Err("adoption authority requires assistant authority provenance".into());
    }
    if let Some(attribution) = &raw.authority_source {
        let assistant_in_episode = attribution.conversation_id == episode.conversation_id
            && turns.iter().any(|turn| {
                turn.node_id == attribution.node_id
                    && turn.role == "assistant"
                    && turn.content.contains(&attribution.quote)
                    && turn.timestamp_ns <= source.timestamp_ns
            });
        let assistant_in_evidence = evidence.iter().any(|turn| {
            turn.conversation_id == attribution.conversation_id
                && turn.node_id == attribution.node_id
                && turn.role == "assistant"
                && turn.content.contains(&attribution.quote)
                && turn.timestamp_ns <= source.timestamp_ns
        });
        if !assistant_in_episode && !assistant_in_evidence {
            return Err("authority attribution must identify an assistant turn".into());
        }
    }
    let title = raw.title.trim();
    let content = raw.content.trim();
    if let Some(reason) = validate_semantic_authority(
        &authority_kind,
        &category,
        title,
        &source_quote,
        content,
        raw.authority_source.is_some(),
        raw.grounding_source.is_some(),
    ) {
        return Err(reason);
    }
    if raw.routing_mentions.len() > MAX_MEMORY_ENTITY_MENTIONS {
        return Err("too many routing mentions".into());
    }
    for mention in &raw.routing_mentions {
        validate_mention(title, content, mention)?;
    }
    Ok(InsomniaCandidate {
        authority_kind,
        category,
        memory_type,
        temporal_status,
        ownership: raw.ownership,
        title: title.to_owned(),
        content: content.to_owned(),
        source_node_id,
        source_quote,
        authority_source: raw.authority_source,
        grounding_source: raw.grounding_source,
        routing_mentions: raw.routing_mentions,
    })
}

fn validate_mention(title: &str, content: &str, mention: &RoutingMention) -> Result<(), String> {
    let text = match mention.field {
        TextField::Title => title,
        TextField::Content => content,
    };
    if mention.start_byte >= mention.end_byte || mention.end_byte > text.len() {
        return Err("routing mention span is out of bounds".into());
    }
    let exact = text
        .get(mention.start_byte..mention.end_byte)
        .ok_or("routing mention splits UTF-8")?;
    if exact != mention.text || mention.text.trim().is_empty() {
        return Err("routing mention is not bound to exact candidate text".into());
    }
    Ok(())
}
fn canonical_label(value: &str) -> String {
    value.trim().to_lowercase().replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_archive::{EpisodeBoundary, EpisodeId, EpisodeOrigin};
    use serde_json::json;

    fn episode() -> Episode {
        Episode {
            id: EpisodeId([1; 32]),
            conversation_id: "c".into(),
            start_node_id: "n1".into(),
            end_node_id: "n2".into(),
            origin: EpisodeOrigin::Live,
            boundary: EpisodeBoundary::Explicit,
            source_through_ns: 2,
            finalized_at_ns: 2,
        }
    }
    fn turns() -> Vec<ResolvedTurn> {
        vec![
            ResolvedTurn {
                node_id: "n1".into(),
                role: "user".into(),
                principal_id: None,
                timestamp_ns: 1,
                content: "I prefer concise answers.".into(),
            },
            ResolvedTurn {
                node_id: "n2".into(),
                role: "assistant".into(),
                principal_id: None,
                timestamp_ns: 2,
                content: "I will keep answers concise.".into(),
            },
        ]
    }
    fn candidate() -> RawCandidate {
        RawCandidate {
            authority_kind: "direct".into(),
            category: "preference".into(),
            memory_type: "communication".into(),
            temporal_status: "current".into(),
            ownership: InsomniaOwnership::User,
            title: "Concise answers".into(),
            content: "The user prefers concise answers.".into(),
            source_node_id: "n1".into(),
            source_quote: "I prefer concise answers.".into(),
            authority_source: None,
            grounding_source: None,
            routing_mentions: vec![],
        }
    }

    #[test]
    fn candidate_requires_verbatim_user_turn_and_rejects_assistant_or_fabricated_quotes() {
        let turns = turns();
        assert_eq!(
            validate_candidates(&episode(), &turns, &[], vec![candidate()])
                .unwrap()
                .candidates
                .len(),
            1
        );
        let mut wrong = candidate();
        wrong.source_node_id = "n2".into();
        wrong.source_quote = "I will keep answers concise.".into();
        assert!(
            validate_candidates(&episode(), &turns, &[], vec![wrong])
                .unwrap()
                .candidates
                .is_empty()
        );
        let mut invented = candidate();
        invented.source_quote = "I like concise answers.".into();
        assert!(
            validate_candidates(&episode(), &turns, &[], vec![invented])
                .unwrap()
                .candidates
                .is_empty()
        );
    }

    #[test]
    fn attribution_requires_caller_supplied_exact_evidence() {
        let mut turns = turns();
        turns[1].timestamp_ns = 0;
        let mut value = candidate();
        value.authority_kind = "adoption".into();
        value.authority_source = Some(SourceAttribution {
            conversation_id: "c".into(),
            node_id: "n2".into(),
            quote: "I will keep answers concise.".into(),
        });
        assert!(
            validate_candidates(&episode(), &turns, &[], vec![value.clone()])
                .unwrap()
                .candidates
                .len()
                == 1
        );
        let mut later = turns.clone();
        later[1].timestamp_ns = 2;
        assert!(
            validate_candidates(&episode(), &later, &[], vec![value.clone()])
                .unwrap()
                .candidates
                .is_empty()
        );
        let mut user_as_assistant = value.clone();
        user_as_assistant.authority_source = Some(SourceAttribution {
            conversation_id: "c".into(),
            node_id: "n1".into(),
            quote: "I prefer concise answers.".into(),
        });
        assert!(
            validate_candidates(&episode(), &turns, &[], vec![user_as_assistant])
                .unwrap()
                .candidates
                .is_empty()
        );
        let mut retention_with_assistant = value.clone();
        retention_with_assistant.authority_kind = "retention".into();
        turns[0].content = "Remember that.".into();
        retention_with_assistant.source_quote = "Remember that.".into();
        assert!(
            validate_candidates(&episode(), &turns, &[], vec![retention_with_assistant])
                .unwrap()
                .candidates
                .len()
                == 1
        );
        turns[0].content = "I prefer concise answers.".into();
        value.authority_source.as_mut().unwrap().quote = "I will answer briefly.".into();
        assert!(
            validate_candidates(&episode(), &turns, &[], vec![value])
                .unwrap()
                .candidates
                .is_empty()
        );
    }

    #[test]
    fn routing_mentions_are_byte_exact_and_evidence_requests_are_bounded() {
        let turns = turns();
        let mut value = candidate();
        value.title = "Café responses".into();
        value.routing_mentions = vec![RoutingMention {
            field: TextField::Title,
            start_byte: 0,
            end_byte: 5,
            text: "Café".into(),
        }];
        assert_eq!(
            validate_candidates(&episode(), &turns, &[], vec![value])
                .unwrap()
                .candidates
                .len(),
            1
        );
        assert_eq!(parse_evidence_requests(&json!({"evidence_requests":[{"kind":" TURN ","conversation_id":"c","node_id":"x"}]})).unwrap()[0].kind,"turn");
        assert!(parse_evidence_requests(&json!({"evidence_requests":[{},{},{},{},{}]})).is_err());
        assert!(
            parse_evidence_requests(&json!({"evidence_requests":[{"kind":"unknown"}]})).is_err()
        );
        assert!(
            parse_evidence_requests(
                &json!({"evidence_requests":[{"kind":"turn","conversation_id":"c"}]})
            )
            .is_err()
        );
        assert!(
            parse_evidence_requests(
                &json!({"evidence_requests":[{"kind":"archive_search","query":"topic","limit":6}]})
            )
            .is_err()
        );
    }

    #[test]
    fn evidence_results_must_reference_exact_authorized_bounded_turns() {
        let request = EvidenceRequest {
            kind: "turn".into(),
            conversation_id: "c".into(),
            node_id: "n2".into(),
            start_node_id: String::new(),
            end_node_id: String::new(),
            query: String::new(),
            limit: 1,
        };
        let turn = EvidenceTurn {
            conversation_id: "c".into(),
            node_id: "n2".into(),
            role: "assistant".into(),
            timestamp_ns: 2,
            content: "I will keep answers concise.".into(),
        };
        let result = EvidenceResult {
            kind: "turn".into(),
            turns: vec![turn.clone()],
            error: None,
            truncated: false,
        };
        assert!(
            validate_evidence_response(&[request.clone()], &[result.clone()], &[turn.clone()])
                .is_ok()
        );
        let mut wrong_target = request.clone();
        wrong_target.node_id = "different".into();
        assert!(
            validate_evidence_response(&[wrong_target], &[result.clone()], &[turn.clone()])
                .is_err()
        );
        let mut search = request.clone();
        search.kind = "archive_search".into();
        search.query = "concise".into();
        search.conversation_id = "other".into();
        let mut search_result = result.clone();
        search_result.kind = "archive_search".into();
        assert!(validate_evidence_response(&[search], &[search_result], &[turn.clone()]).is_err());
        let mut altered = result;
        altered.turns[0].content.push('!');
        assert!(
            validate_evidence_response(&[request.clone()], &[altered], &[turn.clone()]).is_err()
        );
        let mut mismatched = request;
        mismatched.kind = "node".into();
        assert!(
            validate_evidence_response(
                &[mismatched],
                &[EvidenceResult {
                    kind: "turn".into(),
                    turns: vec![turn.clone()],
                    error: None,
                    truncated: false
                }],
                &[turn]
            )
            .is_err()
        );
    }

    #[test]
    fn evidence_turn_can_be_checkpoint_serialized_and_restored() {
        let turn = EvidenceTurn {
            conversation_id: "conversation".into(),
            node_id: "assistant-1".into(),
            role: "assistant".into(),
            timestamp_ns: 7,
            content: "Use concise wording.".into(),
        };
        let encoded = serde_json::to_vec(&turn).unwrap();
        assert_eq!(
            serde_json::from_slice::<EvidenceTurn>(&encoded).unwrap(),
            turn
        );
    }

    #[test]
    fn frozen_extraction_class_policy_matrix() {
        let cases = [
            (
                "direct",
                "preference",
                "I prefer concise answers.",
                "The user prefers concise answers.",
                false,
                false,
                None,
            ),
            (
                "correction",
                "decision",
                "we do want to add it now",
                "Add ShipStats now.",
                false,
                true,
                None,
            ),
            (
                "direct",
                "constraint",
                "they'll have to because collision is server-authoritative, right?",
                "Collision handling has to remain server-authoritative.",
                false,
                false,
                None,
            ),
            (
                "direct",
                "decision",
                "Should I add local spawners and transfer spawn logic?",
                "Add local spawners and transfer spawn logic.",
                false,
                false,
                Some("interrogative authority"),
            ),
            (
                "direct",
                "fact",
                "completed through 41",
                "The prompt sequence is completed through 41.",
                false,
                false,
                Some("execution receipt"),
            ),
            (
                "direct",
                "decision",
                "alright, that works",
                "The server returns the updated Player state.",
                false,
                false,
                Some("vague authority turn"),
            ),
        ];
        for (authority, category, source, content, authority_source, grounding_source, rejected) in
            cases
        {
            let result = validate_semantic_authority(
                authority,
                category,
                "Frozen policy case",
                source,
                content,
                authority_source,
                grounding_source,
            );
            match rejected {
                Some(expected) => assert!(
                    result
                        .as_deref()
                        .is_some_and(|reason| reason.contains(expected)),
                    "{source:?}: {result:?}"
                ),
                None => assert!(result.is_none(), "{source:?}: {result:?}"),
            }
        }
    }
}
