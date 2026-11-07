use crate::evidence_encoding::{hex, sha256};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(crate) fn commitment(
    domain: &str,
    family: &str,
    key: &[u8],
    mut value: Value,
    availability: &str,
) -> Result<Value, String> {
    let coordinate: BTreeMap<_, _> = [
        "global_version",
        "archive_version",
        "first_applying_archive_version",
        "first_applying_global_version",
        "memory_version",
        "entity_version",
        "relationship_version",
        "graph_version",
        "ego_version",
        "vector_version",
        "source_turn",
        "rel_turn",
        "transaction_time_ns",
    ]
    .into_iter()
    .filter_map(|name| {
        let number = value.get(name)?.as_number()?;
        (number.is_u64() || number.is_i64()).then(|| (name, number.to_string()))
    })
    .collect();
    value.sort_all_objects();
    let canonical = serde_json::to_vec(&value).map_err(|_| "retained_row_encoding_failed")?;
    Ok(json!({
        "projection_domain": domain,
        "family": family,
        "stable_key_sha256": hex(&sha256(key)),
        "canonical_value_sha256": hex(&sha256(&canonical)),
        "canonical_byte_length": canonical.len().to_string(),
        "origin_coordinate": coordinate,
        "availability": availability,
    }))
}
