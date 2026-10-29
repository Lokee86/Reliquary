//! Pure policy for naming derived Memory communities.

use reliquary_graph::{
    COMMUNITY_NAMING_CONTRACT_VERSION, DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES,
    MAX_COMMUNITY_SEMANTIC_NAME_BYTES,
};
use reliquary_memory_core::{Memory, MemoryId};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use reliquary_vectors::compatibility_vector::cosine;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub enum DreamCommunityNameEvaluationError {
    Payload(String),
    Inference(GeneralEndpointError),
    InvalidOutput(String),
}

pub struct DreamCommunityNameEvaluator<E> {
    endpoint: E,
    system_prompt: String,
}

impl<E: GeneralEndpoint> DreamCommunityNameEvaluator<E> {
    pub fn new(endpoint: E) -> Self {
        Self::with_system_prompt(endpoint, DREAM_COMMUNITY_NAMING_SYSTEM_PROMPT)
    }

    pub fn with_system_prompt(endpoint: E, system_prompt: impl Into<String>) -> Self {
        Self {
            endpoint,
            system_prompt: system_prompt.into(),
        }
    }

    pub fn model(&self) -> &str {
        self.endpoint.model()
    }

    pub fn generate_name(
        &self,
        memories: &[Memory],
    ) -> Result<String, DreamCommunityNameEvaluationError> {
        let payload =
            build_payload(memories).map_err(DreamCommunityNameEvaluationError::Payload)?;
        let schema = dream_community_naming_schema();
        for attempt in 0..=DREAM_COMMUNITY_NAMING_RETRY_LIMIT {
            match self.endpoint.complete_json(
                &self.system_prompt,
                &payload,
                "dream_community_name",
                &schema,
            ) {
                Ok(output) => match parse_name(&output) {
                    Ok(name) => return Ok(name),
                    Err(error) if attempt == DREAM_COMMUNITY_NAMING_RETRY_LIMIT => {
                        return Err(DreamCommunityNameEvaluationError::InvalidOutput(error));
                    }
                    Err(_) => {}
                },
                Err(error) if error.is_backpressure() => {
                    return Err(DreamCommunityNameEvaluationError::Inference(error));
                }
                Err(error) if attempt == DREAM_COMMUNITY_NAMING_RETRY_LIMIT => {
                    return Err(DreamCommunityNameEvaluationError::Inference(error));
                }
                Err(_) => {}
            }
        }
        unreachable!("Dream community naming retry loop always returns")
    }
}

pub const DREAM_COMMUNITY_NAMING_SYSTEM_PROMPT: &str = r#"You name one derived Memory-Web community from representative Memories selected from its semantic sub-centroids.

Return one concise semantic name that describes the shared subject of the representative Memories.

Rules:
- Use a concrete noun phrase, normally 2-6 words.
- Name the subject, not the fact that these are memories or a community.
- Prefer specific domain language present in the evidence.
- Do not invent entities, projects, people, or themes absent from the evidence.
- Avoid generic labels such as Miscellaneous, General, Other, Notes, or Memories.
- Do not add explanation, punctuation-only decoration, quotation marks, or a trailing period.
- Output only the schema fields."#;

const MAX_REPRESENTATIVE_TEXT_BYTES: usize = 2_048;
const REPRESENTATIVES_PER_SUBCENTROID: usize = 2;
pub const DREAM_COMMUNITY_NAMING_RETRY_LIMIT: usize = 2;

pub fn dream_community_naming_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["name"],
        "properties": {"name": {"type": "string", "minLength": 1,
            "maxLength": MAX_COMMUNITY_SEMANTIC_NAME_BYTES}}
    })
}

pub fn build_payload(memories: &[Memory]) -> Result<String, String> {
    serde_json::to_string(&json!({
        "contract_version": COMMUNITY_NAMING_CONTRACT_VERSION,
        "representative_memories": memories.iter().enumerate().map(|(index, memory)| json!({
            "ordinal": index + 1,
            "title": truncate_utf8(&memory.title, MAX_REPRESENTATIVE_TEXT_BYTES / 4),
            "content": truncate_utf8(&memory.content, MAX_REPRESENTATIVE_TEXT_BYTES),
        })).collect::<Vec<_>>()
    }))
    .map_err(|error| error.to_string())
}

pub fn representative_ids(
    members: &[MemoryId],
    centroids: &[Vec<f32>],
    vectors: &HashMap<MemoryId, Vec<f32>>,
) -> Vec<MemoryId> {
    let member_set: HashSet<_> = members.iter().copied().collect();
    let mut selected = Vec::with_capacity(DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES);
    for centroid in centroids {
        let mut ranked = ranked_members_for_centroid(members, vectors, centroid);
        let mut admitted = 0;
        for (memory_id, _) in ranked.drain(..) {
            if !selected.contains(&memory_id) {
                selected.push(memory_id);
                admitted += 1;
            }
            if admitted == REPRESENTATIVES_PER_SUBCENTROID
                || selected.len() == DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES
            {
                break;
            }
        }
        if selected.len() == DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES {
            break;
        }
    }
    if selected.len() < DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES {
        let mut remaining: Vec<_> = vectors
            .iter()
            .filter(|(id, _)| member_set.contains(id))
            .filter_map(|(id, vector)| {
                Some((
                    *id,
                    centroids
                        .iter()
                        .filter_map(|c| cosine(c, vector))
                        .max_by(f64::total_cmp)?,
                ))
            })
            .collect();
        sort_ranked(&mut remaining);
        for (id, _) in remaining {
            if selected.len() == DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES {
                break;
            }
            if !selected.contains(&id) {
                selected.push(id);
            }
        }
    }
    for id in members {
        if selected.len() == DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES {
            break;
        }
        if !selected.contains(id) {
            selected.push(*id);
        }
    }
    selected.sort_by_key(|id| id.0);
    selected
}

pub fn parse_name(output: &Value) -> Result<String, String> {
    let raw = output
        .get("name")
        .and_then(Value::as_str)
        .ok_or("missing string field name")?;
    let name = raw.trim();
    if name.is_empty() {
        return Err("name is empty".into());
    }
    if name.len() > MAX_COMMUNITY_SEMANTIC_NAME_BYTES {
        return Err("name is too long".into());
    }
    if name.chars().any(char::is_control) {
        return Err("name contains control characters".into());
    }
    Ok(name.to_owned())
}

fn ranked_members_for_centroid(
    members: &[MemoryId],
    vectors: &HashMap<MemoryId, Vec<f32>>,
    centroid: &[f32],
) -> Vec<(MemoryId, f64)> {
    let mut ranked: Vec<_> = members
        .iter()
        .filter_map(|id| Some((*id, cosine(centroid, vectors.get(id)?)?)))
        .collect();
    sort_ranked(&mut ranked);
    ranked
}

fn sort_ranked(ranked: &mut [(MemoryId, f64)]) {
    ranked.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.0.cmp(&right.0.0))
    });
}

fn truncate_utf8(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

#[cfg(test)]
#[path = "community_naming_tests.rs"]
mod tests;
