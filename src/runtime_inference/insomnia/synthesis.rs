use super::candidate::parse_candidates;
use super::contract::MAX_INSOMNIA_CANDIDATES;
use super::extraction::{
    InsomniaCandidate, InsomniaEvidenceTurn, InsomniaExtractionError, InsomniaRejection,
};
use super::ledger::LedgerEntry;
use super::ownership::InsomniaOwnership;
use crate::ResolvedTurn;
use serde_json::{Map, Value, json};

pub(super) const SYNTHESIS_SYSTEM_PROMPT: &str = r#"You are the wording pass for Reliquary Insomnia.
You receive one authoritative conversation episode plus deterministic synthesis groups built from a validated authority/disposition ledger. Every group already has final semantic ownership: source, authority kind, category, type, lifecycle, assistant-authority provenance, grounding provenance, and retained propositions.

Your ONLY job is to write one concise durable Memory title and content body for every supplied group.
Rules:
1. Return exactly one wording result for every required group key. Do not add, drop, merge, split, rename, or reorder groups.
2. Preserve every proposition in the group. You may consolidate wording, but may not omit a retained proposition or add a proposition not present in the group.
3. Preserve modality exactly: current, future, historical, uncertainty, and correction semantics must not drift.
4. Strip conversational/checkpoint phrasing that is not part of the retained propositions. Do not introduce prompt/phase/step numbers, receipts, implementation chatter, rationale, architecture, paths, consequences, or contextual facts absent from the group.
5. Grounding, authority, metadata, and durable ownership are already final and are not fields you can edit.
6. Keep content compact and durable. The title should identify the remembered state; the body should state the grouped propositions naturally without editorial commentary.

The episode is supplied only to help preserve referent wording when necessary. It is not permission to reinterpret the groups."#;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SynthesisGroup {
    pub group_id: String,
    pub source_node_id: String,
    pub authority_kind: String,
    pub category: String,
    pub memory_type: String,
    pub lifecycle: String,
    pub ownership: InsomniaOwnership,
    pub authority_source_node_id: String,
    pub grounding_source_node_id: String,
    pub propositions: Vec<String>,
}

pub(super) fn build_groups(
    entries: &[LedgerEntry],
) -> Result<Vec<SynthesisGroup>, InsomniaExtractionError> {
    let mut groups = Vec::<SynthesisGroup>::new();
    for entry in entries.iter().filter(|entry| entry.disposition == "retain") {
        if let Some(group) = groups.iter_mut().find(|group| same_signature(group, entry)) {
            group.propositions.push(entry.proposition.clone());
            continue;
        }
        if groups.len() == MAX_INSOMNIA_CANDIDATES {
            return Err(invalid(format!(
                "retained ledger requires more than {MAX_INSOMNIA_CANDIDATES} synthesis groups"
            )));
        }
        groups.push(SynthesisGroup {
            group_id: format!("g{:03}", groups.len()),
            source_node_id: entry.source_node_id.clone(),
            authority_kind: entry.authority_kind.clone(),
            category: entry.category.clone(),
            memory_type: entry.memory_type.clone(),
            lifecycle: entry.lifecycle.clone(),
            ownership: InsomniaOwnership::Project,
            authority_source_node_id: entry.authority_source_node_id.clone(),
            grounding_source_node_id: entry.grounding_source_node_id.clone(),
            propositions: vec![entry.proposition.clone()],
        });
    }
    Ok(groups)
}

pub(super) fn payload(episode_payload: &Value, groups: &[SynthesisGroup]) -> Value {
    json!({
        "authoritative_episode": episode_payload,
        "synthesis_groups": groups.iter().map(group_json).collect::<Vec<_>>()
    })
}

pub(super) fn schema(groups: &[SynthesisGroup]) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for group in groups {
        required.push(Value::String(group.group_id.clone()));
        properties.insert(
            group.group_id.clone(),
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "title": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["title", "content"]
            }),
        );
    }
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "groups": {
                "type": "object",
                "additionalProperties": false,
                "properties": properties,
                "required": required
            }
        },
        "required": ["groups"]
    })
}

pub(super) fn materialize_input(
    episode: &super::input::InsomniaSourceInput,
    turns: &[ResolvedTurn],
    evidence_turns: &[InsomniaEvidenceTurn],
    groups: &[SynthesisGroup],
    wording: &Value,
) -> Result<(Vec<InsomniaCandidate>, Vec<InsomniaRejection>), InsomniaExtractionError> {
    let words = wording
        .get("groups")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("synthesis output is missing groups object"))?;
    if words.len() != groups.len() {
        return Err(invalid(format!(
            "synthesis group count mismatch: expected {}, got {}",
            groups.len(),
            words.len()
        )));
    }
    let mut candidates = Vec::with_capacity(groups.len());
    for group in groups {
        let word = words
            .get(&group.group_id)
            .ok_or_else(|| invalid(format!("synthesis omitted group {}", group.group_id)))?;
        let title = required_string(word, "title")?;
        let content = required_string(word, "content")?;
        if title.trim().is_empty() || content.trim().is_empty() {
            return Err(invalid(format!(
                "synthesis wording is empty for {}",
                group.group_id
            )));
        }
        let (source_conversation_id, source_quote, source_role) =
            source_text(episode, turns, evidence_turns, &group.source_node_id)?;
        if source_role != "user" || source_conversation_id != episode.conversation_id {
            return Err(invalid(format!(
                "synthesis group source is not an authoritative episode user turn: {}",
                group.source_node_id
            )));
        }
        let authority = optional_source_text(
            episode,
            turns,
            evidence_turns,
            &group.authority_source_node_id,
        )?;
        let grounding = optional_source_text(
            episode,
            turns,
            evidence_turns,
            &group.grounding_source_node_id,
        )?;
        candidates.push(json!({
            "authority_kind": group.authority_kind,
            "category": group.category,
            "type": group.memory_type,
            "temporal_status": group.lifecycle,
            "ownership": group.ownership.as_str(),
            "title": title,
            "content": content,
            "source_node_id": group.source_node_id,
            "source_quote": source_quote,
            "authority_source_conversation_id": authority.as_ref().map_or("", |source| source.0.as_str()),
            "authority_source_node_id": group.authority_source_node_id,
            "authority_source_quote": authority.as_ref().map_or("", |source| source.1.as_str()),
            "grounding_source_conversation_id": grounding.as_ref().map_or("", |source| source.0.as_str()),
            "grounding_source_node_id": group.grounding_source_node_id,
            "grounding_source_quote": grounding.as_ref().map_or("", |source| source.1.as_str()),
            "semantic_key": semantic_key(group)
        }));
    }
    let raw = parse_candidates(&json!({"candidates": candidates}))?;
    Ok(super::candidate::validate_candidates_for_source(
        episode.key,
        turns,
        raw,
    ))
}

fn same_signature(group: &SynthesisGroup, entry: &LedgerEntry) -> bool {
    group.source_node_id == entry.source_node_id
        && group.authority_kind == entry.authority_kind
        && group.category == entry.category
        && group.memory_type == entry.memory_type
        && group.lifecycle == entry.lifecycle
        && group.authority_source_node_id == entry.authority_source_node_id
        && group.grounding_source_node_id == entry.grounding_source_node_id
}

fn semantic_key(group: &SynthesisGroup) -> String {
    let propositions = group.propositions.join("\u{1f}");
    format!(
        "{}\u{1e}{}\u{1e}{}\u{1e}{}\u{1e}{}\u{1e}{}\u{1e}{}\u{1e}{}",
        group.source_node_id,
        group.authority_kind,
        group.category,
        group.memory_type,
        group.lifecycle,
        group.authority_source_node_id,
        group.grounding_source_node_id,
        propositions
    )
}

fn group_json(group: &SynthesisGroup) -> Value {
    json!({
        "group_id": group.group_id,
        "source_node_id": group.source_node_id,
        "authority_kind": group.authority_kind,
        "category": group.category,
        "type": group.memory_type,
        "lifecycle": group.lifecycle,
        "ownership": group.ownership.as_str(),
        "authority_source_node_id": group.authority_source_node_id,
        "grounding_source_node_id": group.grounding_source_node_id,
        "propositions": group.propositions
    })
}

fn source_text(
    episode: &super::input::InsomniaSourceInput,
    turns: &[ResolvedTurn],
    evidence_turns: &[InsomniaEvidenceTurn],
    node_id: &str,
) -> Result<(String, String, String), InsomniaExtractionError> {
    if let Some(turn) = turns.iter().find(|turn| turn.node_id == node_id) {
        return Ok((
            episode.conversation_id.clone(),
            turn.content.clone(),
            turn.role.clone(),
        ));
    }
    if let Some(turn) = evidence_turns.iter().find(|turn| turn.node_id == node_id) {
        return Ok((
            turn.conversation_id.clone(),
            turn.content.clone(),
            turn.role.clone(),
        ));
    }
    Err(invalid(format!(
        "selected provenance turn is unavailable: {node_id}"
    )))
}

fn optional_source_text(
    episode: &super::input::InsomniaSourceInput,
    turns: &[ResolvedTurn],
    evidence_turns: &[InsomniaEvidenceTurn],
    node_id: &str,
) -> Result<Option<(String, String, String)>, InsomniaExtractionError> {
    if node_id.is_empty() {
        Ok(None)
    } else {
        source_text(episode, turns, evidence_turns, node_id).map(Some)
    }
}

fn required_string(value: &Value, key: &str) -> Result<String, InsomniaExtractionError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("synthesis field {key} is missing")))
}

fn invalid(message: impl Into<String>) -> InsomniaExtractionError {
    InsomniaExtractionError::InvalidOutput(message.into())
}

#[cfg(test)]
pub(super) fn materialize(
    episode: &crate::Episode,
    turns: &[ResolvedTurn],
    evidence: &[InsomniaEvidenceTurn],
    groups: &[SynthesisGroup],
    wording: &Value,
) -> Result<(Vec<InsomniaCandidate>, Vec<InsomniaRejection>), InsomniaExtractionError> {
    materialize_input(
        &super::input::InsomniaSourceInput::native(episode),
        turns,
        evidence,
        groups,
        wording,
    )
}
