use super::candidate_source::{RawSource, source_complete};
use crate::ResolvedTurn;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_candidate_shape(
    authority_kind: &str,
    category: &str,
    memory_type: &str,
    title: &str,
    content: &str,
    source: Option<&ResolvedTurn>,
    source_quote: &str,
    authority_source: &RawSource,
    grounding_source: &RawSource,
) -> Option<String> {
    if !["direct", "correction", "adoption", "retention"].contains(&authority_kind) {
        Some("authority kind is not recognized".into())
    } else if ![
        "fact",
        "preference",
        "decision",
        "instruction",
        "relationship",
        "constraint",
        "correction",
        "commitment",
    ]
    .contains(&category)
    {
        Some("category is not recognized".into())
    } else if ![
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
    .contains(&memory_type)
    {
        Some("type is not recognized".into())
    } else if title.trim().is_empty() || content.trim().is_empty() {
        Some("title and content are required".into())
    } else if source.is_none() {
        Some("source node is outside the authoritative episode".into())
    } else if source.is_some_and(|turn| turn.role != "user") {
        Some("source node is not a user authority turn".into())
    } else if source_quote.is_empty()
        || source.is_some_and(|turn| !turn.content.contains(source_quote))
    {
        Some("source quote is not verbatim from the authority turn".into())
    } else if !source_complete(authority_source) {
        Some("authority-source fields must be all empty or all present".into())
    } else if !source_complete(grounding_source) {
        Some("grounding-source fields must be all empty or all present".into())
    } else {
        None
    }
}
