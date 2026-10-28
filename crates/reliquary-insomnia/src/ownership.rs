use super::extraction::{InsomniaEvidenceTurn, InsomniaExtractionError};
use super::synthesis::SynthesisGroup;
use crate::{GeneralEndpoint, ResolvedTurn};
use serde_json::{Map, Value, json};

pub const INSOMNIA_OWNERSHIP_SYSTEM_PROMPT: &str = r#"You are the durable-ownership classification pass for Reliquary Insomnia.
You receive one authoritative conversation episode plus fixed synthesis groups whose semantic content, authority/provenance, category, type, and lifecycle are already final.

Your ONLY job is to classify each fixed group as either user or project.
You MUST NOT reinterpret, add, remove, merge, split, rewrite, or otherwise modify the durable proposition or any metadata.

Ownership means: which durable state owner should retain this proposition after the current project is gone. It is NOT confidentiality, access control, visibility, retrieval relevance, or authority/governance.

Choose user when the proposition is genuinely user-global and should remain true/useful across unrelated projects, such as:
- stable personal identity, background, possessions, location, or personal circumstances;
- durable personal preferences, including default communication/style preferences;
- reusable personal working methods or standing instructions explicitly about how the user generally works;
- personal commitments or schedules that are not specific to the active project.

Choose project when the proposition belongs to the current body of work or should not automatically follow the user into unrelated work, such as:
- implementation, architecture, repository, debugging, runtime, file, toolchain, or project status;
- product/design decisions and constraints for the current project;
- project-specific preferences, methods, plans, commitments, or instructions;
- facts about organizations, clients, vendors, relationships, or other non-user owners that are not clearly user-global. The first implementation intentionally uses project as the conservative non-user sink until additional durable owner kinds are enabled.

Decision test: if carrying the proposition unchanged into an unrelated project would be misleading, inappropriate, or accidental leakage, choose project. If the proposition describes the user independently of the active project and is intended to follow them, choose user.

When ambiguous, choose project. Export to user-global state must be conservative.
Return exactly one ownership value for every required group key."#;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InsomniaOwnership {
    User,
    Project,
}

impl InsomniaOwnership {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
        }
    }
}

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
        INSOMNIA_OWNERSHIP_SYSTEM_PROMPT,
        &payload,
        "insomnia_memory_ownership",
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
                "source_quote": required_turn_content(turns, evidence_turns, &group.source_node_id)?,
                "authority_kind": group.authority_kind,
                "category": group.category,
                "type": group.memory_type,
                "lifecycle": group.lifecycle,
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
                    "ownership": {"type": "string", "enum": ["user", "project"]}
                },
                "required": ["ownership"]
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
        .ok_or_else(|| invalid("ownership output is missing groups object"))?;
    if classifications.len() != groups.len() {
        return Err(invalid(format!(
            "ownership count mismatch: expected {}, got {}",
            groups.len(),
            classifications.len()
        )));
    }
    for group in groups {
        let classified = classifications
            .get(&group.group_id)
            .ok_or_else(|| invalid(format!("ownership omitted group {}", group.group_id)))?;
        group.ownership = match classified.get("ownership").and_then(Value::as_str) {
            Some("user") => InsomniaOwnership::User,
            Some("project") => InsomniaOwnership::Project,
            Some(value) => {
                return Err(invalid(format!(
                    "invalid ownership for {}: {value}",
                    group.group_id
                )));
            }
            None => {
                return Err(invalid(format!(
                    "ownership field is missing for {}",
                    group.group_id
                )));
            }
        };
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
        .ok_or_else(|| invalid(format!("ownership source turn is unavailable: {node_id}")))
}

fn optional_turn_content(
    turns: &[ResolvedTurn],
    evidence_turns: &[InsomniaEvidenceTurn],
    node_id: &str,
) -> Result<String, InsomniaExtractionError> {
    if node_id.is_empty() {
        return Ok(String::new());
    }
    required_turn_content(turns, evidence_turns, node_id)
}

fn invalid(message: impl Into<String>) -> InsomniaExtractionError {
    InsomniaExtractionError::InvalidOutput(message.into())
}
