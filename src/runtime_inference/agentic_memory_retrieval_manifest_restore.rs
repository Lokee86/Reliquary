//! Strict restoration of a trusted host's original V1 manifest, never model arguments.
use super::*;
use crate::{AgenticMemoryError, GraphDirection, GraphRelationKind, MemoryId};
fn invalid() -> AgenticMemoryError {
    AgenticMemoryError::Handoff("invalid retained manifest".into())
}
fn text(value: &Value) -> Result<String, AgenticMemoryError> {
    value.as_str().map(str::to_owned).ok_or_else(invalid)
}
fn array(value: &Value) -> Result<&Vec<Value>, AgenticMemoryError> {
    value.as_array().ok_or_else(invalid)
}
fn memory(value: &Value) -> Result<MemoryRef, AgenticMemoryError> {
    let owner_id = text(&value["owner_id"])?;
    let hex = text(&value["memory_id"])?;
    if owner_id.is_empty()
        || hex.len() != 64
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid());
    }
    let mut id = [0; 32];
    for (index, byte) in id.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).map_err(|_| invalid())?;
    }
    Ok(MemoryRef {
        owner_id,
        memory_id: MemoryId(id),
    })
}
fn memories(value: &Value) -> Result<std::collections::BTreeSet<MemoryRef>, AgenticMemoryError> {
    array(value)?.iter().map(memory).collect()
}
impl PreparedAgenticMemoryResult {
    /// Restore only an original manifest retained by the trusted receiver. The digest must
    /// come from that receiver's acknowledgment, not from model/provider output.
    /// Canonical round-trip equality rejects extra fields, duplicates and unknown versions.
    pub fn restore_manifest(value: &Value, digest: &str) -> Result<Self, AgenticMemoryError> {
        let payload = &value["payload"];
        let operation = match payload["operation"].as_str() {
            Some("memory_query") => AgenticMemoryOperation::Query,
            Some("refine_query") => AgenticMemoryOperation::Refinement,
            Some("traverse_graph") => AgenticMemoryOperation::Traversal,
            _ => return Err(invalid()),
        };
        let outcome = match payload["outcome"]["status"].as_str() {
            Some("no_matches") => AgenticMemoryOutcome::NoMatches,
            Some("already_provided") => {
                AgenticMemoryOutcome::AlreadyProvided(memories(&payload["outcome"]["memories"])?)
            }
            Some("memories") => AgenticMemoryOutcome::Memories(
                array(&payload["outcome"]["memories"])?
                    .iter()
                    .map(|body| {
                        Ok(AgenticMemoryContent {
                            memory: memory(&body["memory"])?,
                            title: text(&body["title"])?,
                            content: text(&body["content"])?,
                        })
                    })
                    .collect::<Result<_, AgenticMemoryError>>()?,
            ),
            _ => return Err(invalid()),
        };
        let anchors = array(&payload["anchors"])?
            .iter()
            .map(|report| {
                let relations = array(&report["relations"])?
                    .iter()
                    .map(|edge| {
                        let kind = match edge["kind"].as_str() {
                            Some("Topical") => GraphRelationKind::Topical,
                            Some("Factual") => GraphRelationKind::Factual,
                            Some("Causal") => GraphRelationKind::Causal,
                            Some("Recurrent") => GraphRelationKind::Recurrent,
                            Some("References") => GraphRelationKind::References,
                            Some("DuplicateOf") => GraphRelationKind::DuplicateOf,
                            Some("Supersedes") => GraphRelationKind::Supersedes,
                            Some("StructuralParent") => GraphRelationKind::StructuralParent,
                            _ => return Err(invalid()),
                        };
                        let direction = match edge["direction"].as_str() {
                            Some("Incoming") => GraphDirection::Incoming,
                            Some("Outgoing") => GraphDirection::Outgoing,
                            _ => return Err(invalid()),
                        };
                        Ok(AgenticMemoryRelationEvidence {
                            neighbor: memory(&edge["neighbor"])?,
                            kind,
                            direction,
                        })
                    })
                    .collect::<Result<_, AgenticMemoryError>>()?;
                Ok(AgenticMemoryAnchorReport {
                    anchor: memory(&report["anchor"])?,
                    unseen_neighbors: usize::try_from(
                        report["unseen_neighbors"].as_u64().ok_or_else(invalid)?,
                    )
                    .map_err(|_| invalid())?,
                    declined_neighbors: usize::try_from(
                        report["declined_neighbors"].as_u64().ok_or_else(invalid)?,
                    )
                    .map_err(|_| invalid())?,
                    relations,
                    approved: memories(&report["approved"])?,
                })
            })
            .collect::<Result<_, AgenticMemoryError>>()?;
        let owner_cuts = value["owner_cuts"]
            .as_object()
            .ok_or_else(invalid)?
            .iter()
            .map(|(owner, cut)| Ok((owner.clone(), cut.as_u64().ok_or_else(invalid)?)))
            .collect::<Result<_, AgenticMemoryError>>()?;
        let restored = Self {
            delivery_id: text(&value["delivery_id"])?,
            payload: AgenticMemoryPayload {
                operation,
                outcome,
                anchors,
            },
            delivered: memories(&value["delivered"])?,
            owner_cuts,
        };
        if restored.delivery_id.is_empty()
            || restored.canonical_manifest() != *value
            || restored.manifest_digest() != digest
        {
            return Err(invalid());
        }
        Ok(restored)
    }
}
