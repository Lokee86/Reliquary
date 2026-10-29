use super::extraction::{InsomniaCandidate, InsomniaExtractionError, InsomniaRoutingMetadata};
use crate::{
    GeneralEndpoint, MAX_MEMORY_ENTITY_MENTIONS, MAX_MEMORY_ROUTING_TEXT_BYTES,
    MemoryEntityMention, MemoryTextField,
};
use serde_json::{Map, Value, json};

pub(super) const ENRICHMENT_SYSTEM_PROMPT: &str = r#"You are the Entity-mention enrichment pass for Reliquary Insomnia. You receive final durable Memory title/content text. The Memory proposition, wording, ownership, provenance, and lifecycle are already authoritative and MUST NOT be changed.
For each supplied Memory, extract only bounded entity_mentions: exact source-language spans that identify reusable referents. A candidate must carry enough identity in its surface form to be meaningfully recognized again in another Memory without reconstructing the sentence that produced it. Qualifying referents include people, organizations, places, projects, products, repositories, teams, sites, files, software systems, named protocols/standards, durable code/API artifacts, and stable relationship/location descriptions such as "the Vancouver office", "my brother", or "the user" when it clearly denotes the current owner. A referent may be new or first-seen; it does not need to exist already. The requirement is identifiable identity, not prior existence. Do NOT emit anonymous situational descriptions whose identity comes only from the current sentence, including bare references such as "the server", "the application", "the file", "the agent", "the repository", "the system", or "the shader", and context-only phrases such as "the login status", "the relevant repository", "the existing server", or "the requested state". Descriptive owner-local component names MAY qualify when a modifier gives the referent a stable reusable identity, even if the phrase is lowercase: "write server", "devtools window", "API service", "main game package", and "Rails database" qualify when they consistently denote that component. Do NOT emit every salient noun or identifier. If the same words could naturally refer to many unrelated objects in other Memories, omit them unless the phrase itself contains a stable distinguishing identity. Use the smallest complete surface phrase that itself identifies the intended reusable referent. Include a generic head when a named or otherwise distinctive modifier makes the whole phrase a particular durable artifact/component: "Rails API server", "Discord OAuth redirect URI", and "WSL's /mnt mount" qualify. Do not attach context/activity/category heads merely because they surround a valid referent: prefer "Godot" over "Godot tooling" or "Godot collision resources", "GitHub" over "GitHub main", "PostgreSQL" over "PostgreSQL clusters", and the qualifying named technologies inside "Discord OAuth flow/configuration" rather than the flow/configuration phrase. Prefer "Windows Git" over overlapping "Windows" + "Git" at that occurrence; emit a separate standalone "Git" elsewhere. For named compounds such as "Godot project" or "Codex app", the article is not needed. Durable code/API artifacts include named packages, types, functions/APIs, endpoints, files, configuration artifacts, stable UI components/scenes, and other identities that persist beyond one execution. A named code symbol qualifies only when it denotes such a durable identity; a temporary wrapper, local variable/value, incidental method result, runtime role, or runtime-only identifier such as "damageResult" does not. For Markdown inline code such as `game.gd`, return "game.gd" without delimiters. Exclude pronouns, transient runtime roles, generic classes, abstract concepts, generic properties/runtime states, generic architectural lane headings, and task/process labels such as "Prompt 94", "Phase 11A", "player state", or "hitboxes". Never canonicalize, paraphrase, change case, pluralize/singularize, or invent text. Do not resolve identity, assign types/IDs, or merge aliases.
Every emitted entity mention MUST specify whether its exact text came from title or content. Every emitted string MUST be copied verbatim from that field. Each mention also has an occurrence selector: use -1 when all boundary-valid occurrences of that exact text in the field denote the same intended referent; when identical text has mixed meanings, emit one item per intended occurrence using its zero-based order among boundary-valid matches. Example: in "local damage result ... imported `damage` package", the package mention is text "damage" with occurrence 1, not -1. Return exactly one result for every supplied Memory key, including an empty array when nothing qualifies."#;

pub fn enrich(
    endpoint: &dyn GeneralEndpoint,
    candidates: &mut [InsomniaCandidate],
) -> Result<(), InsomniaExtractionError> {
    if candidates.is_empty() {
        return Ok(());
    }
    let payload = json!({
        "memories": candidates.iter().map(|candidate| json!({
            "memory_key": candidate.key,
            "title": candidate.title,
            "content": candidate.content,
        })).collect::<Vec<_>>()
    });
    let payload = serde_json::to_string(&payload)
        .map_err(|error| InsomniaExtractionError::InvalidOutput(error.to_string()))?;
    let result = endpoint.complete_json(
        ENRICHMENT_SYSTEM_PROMPT,
        &payload,
        "insomnia_memory_entity_mentions",
        &schema(candidates),
    )?;
    apply(candidates, &result)
}

fn schema(candidates: &[InsomniaCandidate]) -> Value {
    let mut properties = Map::new();
    let required = candidates
        .iter()
        .map(|candidate| Value::String(candidate.key.clone()))
        .collect::<Vec<_>>();
    for candidate in candidates {
        properties.insert(
            candidate.key.clone(),
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "entity_mentions": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "field": {"type": "string", "enum": ["title", "content"]},
                                "text": {"type": "string"},
                                "occurrence": {"type": "integer", "minimum": -1}
                            },
                            "required": ["field", "text", "occurrence"]
                        }
                    }
                },
                "required": ["entity_mentions"]
            }),
        );
    }
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "memories": {
                "type": "object",
                "additionalProperties": false,
                "properties": properties,
                "required": required
            }
        },
        "required": ["memories"]
    })
}

fn apply(
    candidates: &mut [InsomniaCandidate],
    result: &Value,
) -> Result<(), InsomniaExtractionError> {
    let values = result
        .get("memories")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("Entity enrichment output is missing memories object"))?;
    if values.len() != candidates.len() {
        return Err(invalid("Entity enrichment Memory count mismatch"));
    }
    for candidate in candidates {
        let value = values
            .get(&candidate.key)
            .ok_or_else(|| invalid(format!("Entity enrichment omitted {}", candidate.key)))?;
        let mentions = value
            .get("entity_mentions")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("entity_mentions must be an array"))?;
        if mentions.len() > MAX_MEMORY_ENTITY_MENTIONS {
            return Err(invalid("Entity enrichment exceeds bounded item limit"));
        }
        let mut entity_mentions = Vec::new();
        for mention in mentions {
            append_mentions(candidate, mention, &mut entity_mentions)?;
        }
        entity_mentions.sort_by(|left, right| {
            (left.field, left.start_byte, left.end_byte, &left.text).cmp(&(
                right.field,
                right.start_byte,
                right.end_byte,
                &right.text,
            ))
        });
        entity_mentions.dedup();
        retain_most_specific_mentions(&mut entity_mentions);
        if entity_mentions.len() > MAX_MEMORY_ENTITY_MENTIONS {
            return Err(invalid(
                "expanded Entity mentions exceed bounded item limit",
            ));
        }
        candidate.routing_metadata = Some(InsomniaRoutingMetadata { entity_mentions });
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> InsomniaExtractionError {
    InsomniaExtractionError::InvalidOutput(message.into())
}

#[path = "enrichment/generic.rs"]
mod generic;
#[path = "enrichment/mentions.rs"]
mod mentions;
use generic::is_non_reusable_generic_surface;
use mentions::{append_mentions, retain_most_specific_mentions};
