use super::extraction::{InsomniaEvidenceTurn, InsomniaExtractionError};
use super::synthesis::SynthesisGroup;
use crate::{GeneralEndpoint, ResolvedTurn};
use serde_json::{Map, Value, json};

pub(super) const METADATA_SYSTEM_PROMPT: &str = r#"You are the metadata-classification pass for Continuity Insomnia.
You receive one authoritative conversation episode plus fixed synthesis groups built deterministically from a validated semantic ledger. The ledger has already decided what durable state exists, and the groups already have final membership, propositions, authority kind, and provenance.

Your ONLY job is to classify every supplied fixed group with category, type, and lifecycle metadata.
You MUST NOT reinterpret, add, remove, merge, split, rewrite, or otherwise modify semantic state or group membership. Group IDs, propositions, and provenance are authoritative inputs, not suggestions.

Category rules:
1. fact: asserted durable state that is not better described below.
2. preference: a durable like/dislike, style choice, or stated preference.
3. decision: a selected choice, adopted implementation/design, or settled course of action. An adopted assistant proposal is normally a decision when it chooses what the project will do, even if the user's assent is terse.
4. instruction: a standing or reusable instruction about how something should be done. Prefer decision for a one-project implementation choice; prefer instruction for a reusable operating rule.
5. relationship: a durable relationship between people/entities when relationship itself is the remembered proposition.
6. constraint: a requirement, prohibition, limit, invariant, or condition that must hold.
7. correction: use only when the durable memory is primarily a correction of factual/status state. Do NOT choose correction merely because the source says "now", "instead", rejects an older approach, or has authority_kind=correction. If the correction establishes a replacement design, location rule, implementation choice, requirement, or policy, classify the resulting state as decision, constraint, or instruction instead.
8. commitment: a future action or obligation the user commits to performing.

Type rules classify what the proposition is primarily ABOUT:
identity, education, employment, location, possession, health, finance, schedule, communication, project, process, product, relationship, or other.
- project: implementation, architecture, code, runtime state, repository state, debugging state, or design decisions belonging to a specific project. This is the default for project-specific technical state.
- process: a reusable workflow, method, or operating practice that applies beyond one particular implementation. Do NOT use process merely because a proposition describes how project code behaves.
- product: durable user-facing product capability, offering, or product-level direction. Prefer project for internal implementation/design details of a particular codebase.
- communication: durable communication preferences or practices.
- location: where durable work/data/repositories belong or are located; a location rule may still have category constraint/instruction.

The exact source quote is evidence for rhetorical force. Use it to distinguish a genuine factual correction from a normalized proposition that merely looks like a fact. When authority_context is present, it is the assistant content the user adopted; classify the durable adopted choice, not the terse assent or the incidental fact that it "works". When grounding_context is present, use it only to resolve what the user's source quote refers to and whether the utterance is correcting prior factual/status context. Metadata classification may use this evidence, but may not change the proposition.

Lifecycle rules:
- current: presently true/applicable state, including standing preferences, decisions, instructions, constraints, and current corrections.
- future: planned, intended, scheduled, or committed future state/action not yet current.
- historical: state explicitly about the past and not asserted as current.

Use the episode only to understand the propositions' subject and temporal framing. Do not import nearby facts into the classification. Return exactly one classification for every required group key."#;

const CATEGORIES: &[&str] = &[
    "fact",
    "preference",
    "decision",
    "instruction",
    "relationship",
    "constraint",
    "correction",
    "commitment",
];

const TYPES: &[&str] = &[
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
];

pub(super) fn classify(
    endpoint: &dyn GeneralEndpoint,
    episode_payload: &Value,
    turns: &[ResolvedTurn],
    evidence_turns: &[InsomniaEvidenceTurn],
    groups: &mut [SynthesisGroup],
) -> Result<(), InsomniaExtractionError> {
    if groups.is_empty() {
        return Ok(());
    }
    let payload = payload(episode_payload, turns, evidence_turns, groups)?;
    let payload = serde_json::to_string(&payload)
        .map_err(|error| InsomniaExtractionError::InvalidOutput(error.to_string()))?;
    let result = endpoint.complete_json(
        METADATA_SYSTEM_PROMPT,
        &payload,
        "insomnia_memory_metadata",
        &schema(groups),
    )?;
    apply(groups, &result)
}

fn payload(
    episode_payload: &Value,
    turns: &[ResolvedTurn],
    evidence_turns: &[InsomniaEvidenceTurn],
    groups: &[SynthesisGroup],
) -> Result<Value, InsomniaExtractionError> {
    let fixed_synthesis_groups = groups
        .iter()
        .map(|group| {
            Ok(json!({
                "group_id": group.group_id,
                "source_node_id": group.source_node_id,
                "source_quotes": [required_turn_content(turns, evidence_turns, &group.source_node_id)?],
                "authority_kind": group.authority_kind,
                "authority_source_node_id": group.authority_source_node_id,
                "authority_context": optional_turn_content(turns, evidence_turns, &group.authority_source_node_id)?,
                "grounding_source_node_id": group.grounding_source_node_id,
                "grounding_context": optional_turn_content(turns, evidence_turns, &group.grounding_source_node_id)?,
                "propositions": group.propositions,
            }))
        })
        .collect::<Result<Vec<_>, InsomniaExtractionError>>()?;
    Ok(json!({
        "authoritative_episode": episode_payload,
        "fixed_synthesis_groups": fixed_synthesis_groups,
    }))
}

fn schema(groups: &[SynthesisGroup]) -> Value {
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
                    "category": {"type": "string", "enum": CATEGORIES},
                    "type": {"type": "string", "enum": TYPES},
                    "lifecycle": {"type": "string", "enum": ["current", "future", "historical"]}
                },
                "required": ["category", "type", "lifecycle"]
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

fn apply(groups: &mut [SynthesisGroup], result: &Value) -> Result<(), InsomniaExtractionError> {
    let classifications = result
        .get("groups")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("metadata output is missing groups object"))?;
    if classifications.len() != groups.len() {
        return Err(invalid(format!(
            "metadata count mismatch: expected {}, got {}",
            groups.len(),
            classifications.len()
        )));
    }
    for group in groups {
        let classified = classifications
            .get(&group.group_id)
            .ok_or_else(|| invalid(format!("metadata omitted group {}", group.group_id)))?;
        let category = required_string(classified, "category")?;
        let memory_type = required_string(classified, "type")?;
        let lifecycle = required_string(classified, "lifecycle")?;
        if !CATEGORIES.contains(&category.as_str()) {
            return Err(invalid(format!(
                "invalid metadata category for {}: {category}",
                group.group_id
            )));
        }
        if !TYPES.contains(&memory_type.as_str()) {
            return Err(invalid(format!(
                "invalid metadata type for {}: {memory_type}",
                group.group_id
            )));
        }
        if !["current", "future", "historical"].contains(&lifecycle.as_str()) {
            return Err(invalid(format!(
                "invalid metadata lifecycle for {}: {lifecycle}",
                group.group_id
            )));
        }
        group.category = category;
        group.memory_type = memory_type;
        group.lifecycle = lifecycle;
    }
    Ok(())
}

fn required_turn_content(
    turns: &[ResolvedTurn],
    evidence_turns: &[InsomniaEvidenceTurn],
    node_id: &str,
) -> Result<String, InsomniaExtractionError> {
    turns
        .iter()
        .find(|turn| turn.node_id == node_id)
        .map(|turn| turn.content.clone())
        .or_else(|| {
            evidence_turns
                .iter()
                .find(|turn| turn.node_id == node_id)
                .map(|turn| turn.content.clone())
        })
        .ok_or_else(|| invalid(format!("metadata source turn is unavailable: {node_id}")))
}

fn optional_turn_content(
    turns: &[ResolvedTurn],
    evidence_turns: &[InsomniaEvidenceTurn],
    node_id: &str,
) -> Result<String, InsomniaExtractionError> {
    if node_id.is_empty() {
        return Ok(String::new());
    }
    turns
        .iter()
        .find(|turn| turn.node_id == node_id)
        .map(|turn| turn.content.clone())
        .or_else(|| {
            evidence_turns
                .iter()
                .find(|turn| turn.node_id == node_id)
                .map(|turn| turn.content.clone())
        })
        .ok_or_else(|| {
            invalid(format!(
                "metadata provenance turn is unavailable: {node_id}"
            ))
        })
}

fn required_string(value: &Value, key: &str) -> Result<String, InsomniaExtractionError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("metadata field {key} is missing")))
}

fn invalid(message: impl Into<String>) -> InsomniaExtractionError {
    InsomniaExtractionError::InvalidOutput(message.into())
}
