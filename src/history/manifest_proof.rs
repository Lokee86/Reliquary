//! Canonical encoding of the prepared historical reconstruction proof.

use crate::MemoryId;
use crate::history::manifest_encoding::{HistoryProofError, Value};
use crate::history::source_proof::source_key;
use crate::owner_lifecycle::OwnerError;
use std::collections::BTreeSet;

pub(crate) struct ImportScopeProof {
    pub(crate) start_digest: [u8; 32],
    pub(crate) base_cut: u64,
    pub(crate) import_key: String,
    pub(crate) source_nodes: BTreeSet<(String, String)>,
    pub(crate) episodes: BTreeSet<[u8; 32]>,
    pub(crate) recipient_cohort: BTreeSet<MemoryId>,
}

pub(crate) struct ManifestProofParts {
    pub(crate) owner: [u8; 16],
    pub(crate) import_id: [u8; 32],
    pub(crate) corpus_digest: [u8; 32],
    pub(crate) global_version: u64,
    pub(crate) archive_version: u64,
    pub(crate) activity_cut: u64,
    pub(crate) memory_version: u64,
    pub(crate) graph_version: u64,
    pub(crate) memory_graph_version: u64,
    pub(crate) vector_version: u64,
    pub(crate) community_generation: Option<u64>,
    pub(crate) graph: Value,
    pub(crate) replacement: Value,
    pub(crate) order: Value,
    pub(crate) last_source: Option<(String, String)>,
    pub(crate) source_records: Vec<Value>,
    pub(crate) memory_records: Vec<Value>,
    pub(crate) cohort_records: Vec<Value>,
    pub(crate) community: Value,
    pub(crate) vector: Value,
    pub(crate) base_descriptor: Vec<u8>,
    pub(crate) base_digest: [u8; 32],
    pub(crate) semantic_descriptor: Result<String, OwnerError>,
    pub(crate) semantic_digest: Result<[u8; 32], OwnerError>,
    pub(crate) historical_policy: Value,
    pub(crate) historical_digest: [u8; 32],
    pub(crate) scope: Option<ImportScopeProof>,
}

pub(crate) struct EncodedManifestProof {
    pub(crate) value: Value,
    pub(crate) digest: [u8; 32],
}

pub(crate) fn build(parts: ManifestProofParts) -> Result<EncodedManifestProof, OwnerError> {
    let token = Value::map([
        ("global_version", Value::Unsigned(parts.global_version)),
        ("archive_version", Value::Unsigned(parts.archive_version)),
        ("activity_cut", Value::Unsigned(parts.activity_cut)),
        ("memory_version", Value::Unsigned(parts.memory_version)),
        ("graph_version", Value::Unsigned(parts.graph_version)),
        (
            "memory_graph_version",
            Value::Unsigned(parts.memory_graph_version),
        ),
        ("vector_version", Value::Unsigned(parts.vector_version)),
        (
            "community_generation",
            parts
                .community_generation
                .map_or(Value::Null, Value::Unsigned),
        ),
        (
            "dream_proof_digest",
            Value::bytes(
                parts
                    .graph
                    .digest("reliquary:historical-dream-input:v1")
                    .map_err(proof_error)?,
            ),
        ),
        (
            "freshness_proof_digest",
            Value::bytes(
                parts
                    .replacement
                    .digest("reliquary:historical-freshness-input:v1")
                    .map_err(proof_error)?,
            ),
        ),
    ]);

    let activity_order_digest = parts
        .order
        .digest("reliquary:historical-activity-order:v1")
        .map_err(proof_error)?;
    let semantic_descriptor = parts.semantic_descriptor?;
    let semantic_digest = parts.semantic_digest?;
    let mut value = Value::map([
        ("schema_version", Value::Unsigned(2)),
        ("policy_version", Value::Unsigned(3)),
        ("owner_uuid", Value::bytes(parts.owner)),
        ("import_id", Value::bytes(parts.import_id)),
        ("corpus_digest", Value::bytes(parts.corpus_digest)),
        (
            "archive_cut",
            Value::map([
                ("archive_version", Value::Unsigned(parts.archive_version)),
                ("activity_count", Value::Unsigned(parts.activity_cut)),
                (
                    "last_source",
                    parts
                        .last_source
                        .map_or(Value::Null, |(conversation, node)| {
                            source_key(&conversation, &node)
                        }),
                ),
                ("activity_order_digest", Value::bytes(activity_order_digest)),
            ]),
        ),
        ("source_records", Value::Array(parts.source_records)),
        ("memory_records", Value::Array(parts.memory_records)),
        ("cohort_records", Value::Array(parts.cohort_records)),
        ("graph_proof", parts.graph),
        ("community_proof", parts.community),
        ("vector_proof", parts.vector),
        (
            "scoring_policy",
            Value::map([
                ("base_descriptor", Value::bytes(parts.base_descriptor)),
                ("base_digest", Value::bytes(parts.base_digest)),
                ("score_min", Value::Signed(-100)),
                ("score_max", Value::Signed(100)),
                (
                    "semantic_descriptor",
                    Value::bytes(semantic_descriptor.as_bytes()),
                ),
                ("semantic_digest", Value::bytes(semantic_digest)),
                ("historical_descriptor", parts.historical_policy),
                ("historical_digest", Value::bytes(parts.historical_digest)),
                (
                    "event_order",
                    Value::text("admission-then-position-event-id-v1"),
                ),
            ]),
        ),
        ("replacement_proof", parts.replacement),
        ("input_token", token),
    ]);

    if let Some(scope) = parts.scope {
        let Value::Map(fields) = &mut value else {
            unreachable!("historical manifest root is a map");
        };
        fields.extend([
            (
                "import_start_digest".to_owned(),
                Value::bytes(scope.start_digest),
            ),
            (
                "base_activity_cut".to_owned(),
                Value::Unsigned(scope.base_cut),
            ),
            ("import_key".to_owned(), Value::text(scope.import_key)),
            (
                "import_source_nodes".to_owned(),
                Value::Array(
                    scope
                        .source_nodes
                        .into_iter()
                        .map(|(conversation, node)| source_key(&conversation, &node))
                        .collect(),
                ),
            ),
            (
                "import_episode_ids".to_owned(),
                Value::Array(scope.episodes.into_iter().map(Value::bytes).collect()),
            ),
            (
                "import_recipient_cohort".to_owned(),
                Value::Array(
                    scope
                        .recipient_cohort
                        .into_iter()
                        .map(|id| Value::bytes(id.0))
                        .collect(),
                ),
            ),
        ]);
    }

    let digest = value
        .digest("reliquary:historical-manifest:v1")
        .map_err(proof_error)?;
    Ok(EncodedManifestProof { value, digest })
}

fn proof_error(error: HistoryProofError) -> OwnerError {
    OwnerError::Freshness(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts() -> ManifestProofParts {
        ManifestProofParts {
            owner: [0; 16],
            import_id: [0; 32],
            corpus_digest: [0; 32],
            global_version: 0,
            archive_version: 0,
            activity_cut: 0,
            memory_version: 0,
            graph_version: 0,
            memory_graph_version: 0,
            vector_version: 0,
            community_generation: None,
            graph: Value::Null,
            replacement: Value::Null,
            order: Value::Null,
            last_source: None,
            source_records: Vec::new(),
            memory_records: Vec::new(),
            cohort_records: Vec::new(),
            community: Value::Null,
            vector: Value::Null,
            base_descriptor: Vec::new(),
            base_digest: [0; 32],
            semantic_descriptor: Ok(String::new()),
            semantic_digest: Ok([0; 32]),
            historical_policy: Value::Null,
            historical_digest: [0; 32],
            scope: None,
        }
    }

    #[test]
    fn deferred_semantic_failures_preserve_error_message_and_descriptor_precedence() {
        let mut input = parts();
        input.semantic_descriptor = Err(OwnerError::Freshness("descriptor rejected".into()));
        input.semantic_digest = Err(OwnerError::Freshness("digest rejected".into()));
        let error = build(input).err().expect("descriptor must reject");
        assert_eq!(error.to_string(), "Freshness error: descriptor rejected");

        let mut input = parts();
        input.semantic_digest = Err(OwnerError::Freshness("digest rejected".into()));
        let error = build(input).err().expect("digest must reject");
        assert_eq!(error.to_string(), "Freshness error: digest rejected");
    }
}
