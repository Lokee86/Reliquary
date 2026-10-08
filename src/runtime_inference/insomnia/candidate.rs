use super::candidate_policy::validate_semantic_authority;
use super::candidate_shape::validate_candidate_shape;
use super::candidate_source::{RawSource, optional, parse_source, required_string, trim_source};
use super::contract::MAX_INSOMNIA_CANDIDATES;
use super::extraction::{InsomniaCandidate, InsomniaExtractionError, InsomniaRejection};
use super::ownership::InsomniaOwnership;
#[cfg(test)]
use crate::EpisodeId;
use crate::ResolvedTurn;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Clone)]
pub(super) struct RawCandidate {
    authority_kind: String,
    category: String,
    memory_type: String,
    temporal_status: String,
    ownership: InsomniaOwnership,
    title: String,
    content: String,
    source_node_id: String,
    source_quote: String,
    authority_source: RawSource,
    grounding_source: RawSource,
    semantic_key: String,
}

pub(super) fn parse_candidates(
    value: &Value,
) -> Result<Vec<RawCandidate>, InsomniaExtractionError> {
    let candidates = value
        .get("candidates")
        .and_then(Value::as_array)
        .ok_or_else(|| InsomniaExtractionError::InvalidOutput("missing candidates array".into()))?;
    if candidates.len() > MAX_INSOMNIA_CANDIDATES {
        return Err(InsomniaExtractionError::InvalidOutput(format!(
            "{} candidates exceeds maximum {}",
            candidates.len(),
            MAX_INSOMNIA_CANDIDATES
        )));
    }
    candidates.iter().map(parse_candidate).collect()
}

fn parse_candidate(value: &Value) -> Result<RawCandidate, InsomniaExtractionError> {
    Ok(RawCandidate {
        authority_kind: required_string(value, "authority_kind")?,
        category: required_string(value, "category")?,
        memory_type: required_string(value, "type")?,
        temporal_status: required_string(value, "temporal_status")?,
        ownership: match required_string(value, "ownership")?.as_str() {
            "user" => InsomniaOwnership::User,
            "project" => InsomniaOwnership::Project,
            value => {
                return Err(InsomniaExtractionError::InvalidOutput(format!(
                    "invalid candidate ownership: {value}"
                )));
            }
        },
        title: required_string(value, "title")?,
        content: required_string(value, "content")?,
        source_node_id: required_string(value, "source_node_id")?,
        source_quote: required_string(value, "source_quote")?,
        authority_source: parse_source(value, "authority_source")?,
        grounding_source: parse_source(value, "grounding_source")?,
        semantic_key: value
            .get("semantic_key")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_owned(),
    })
}

#[cfg(test)]
pub(super) fn validate_candidates(
    episode_id: EpisodeId,
    turns: &[ResolvedTurn],
    raw: Vec<RawCandidate>,
) -> (Vec<InsomniaCandidate>, Vec<InsomniaRejection>) {
    validate_candidates_for_source(episode_id.0, turns, raw)
}

pub(super) fn validate_candidates_for_source(
    episode_id: [u8; 32],
    turns: &[ResolvedTurn],
    raw: Vec<RawCandidate>,
) -> (Vec<InsomniaCandidate>, Vec<InsomniaRejection>) {
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let mut seen = HashSet::new();
    for raw in raw {
        let authority_kind = canonical_label(&raw.authority_kind);
        let category = canonical_label(&raw.category);
        let memory_type = canonical_label(&raw.memory_type);
        let temporal_status = canonical_label(&raw.temporal_status);
        let source_node_id = raw.source_node_id.trim().to_owned();
        let source_quote = raw.source_quote.trim().to_owned();
        let source = turns.iter().find(|turn| turn.node_id == source_node_id);
        let authority_source = trim_source(&raw.authority_source);
        let grounding_source = trim_source(&raw.grounding_source);
        let mut reason = validate_candidate_shape(
            &authority_kind,
            &category,
            &memory_type,
            raw.title.trim(),
            raw.content.trim(),
            source,
            &source_quote,
            &authority_source,
            &grounding_source,
        );
        if reason.is_none()
            && !matches!(
                temporal_status.as_str(),
                "current" | "future" | "historical"
            )
        {
            reason = Some("temporal status is not recognized".to_owned());
        }
        if reason.is_none() {
            reason = validate_semantic_authority(
                &authority_kind,
                &category,
                raw.title.trim(),
                &source_quote,
                raw.content.trim(),
                !authority_source.node_id.is_empty(),
                !grounding_source.node_id.is_empty(),
            );
        }
        let key = candidate_key(
            episode_id,
            &source_node_id,
            &source_quote,
            &authority_source,
            &grounding_source,
            &raw.semantic_key,
        );
        if reason.is_none() && !seen.insert(key.clone()) {
            reason = Some("duplicate candidate authority anchor".to_owned());
        }
        if let Some(reason) = reason {
            rejected.push(InsomniaRejection {
                candidate_key: Some(key),
                reason,
            });
            continue;
        }
        accepted.push(InsomniaCandidate {
            key,
            authority_kind,
            category,
            memory_type,
            temporal_status,
            ownership: raw.ownership,
            title: raw.title.trim().to_owned(),
            content: raw.content.trim().to_owned(),
            source_node_id,
            source_quote,
            authority_source_conversation_id: optional(&authority_source.conversation_id),
            authority_source_node_id: optional(&authority_source.node_id),
            authority_source_quote: optional(&authority_source.quote),
            grounding_source_conversation_id: optional(&grounding_source.conversation_id),
            grounding_source_node_id: optional(&grounding_source.node_id),
            grounding_source_quote: optional(&grounding_source.quote),
            routing_metadata: None,
        });
    }
    (accepted, rejected)
}

fn canonical_label(value: &str) -> String {
    value.trim().to_lowercase().replace('-', "_")
}

fn candidate_key(
    episode_id: [u8; 32],
    source_node_id: &str,
    source_quote: &str,
    authority_source: &RawSource,
    grounding_source: &RawSource,
    semantic_key: &str,
) -> String {
    let mut hash = Sha256::new();
    hash.update(b"continuity-insomnia-candidate-v3\0");
    hash.update(episode_id);
    for value in [
        source_node_id,
        source_quote,
        &authority_source.conversation_id,
        &authority_source.node_id,
        &authority_source.quote,
        &grounding_source.conversation_id,
        &grounding_source.node_id,
        &grounding_source.quote,
        semantic_key,
    ] {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    format!("claim-{}", hex(&hash.finalize()[..16]))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}
