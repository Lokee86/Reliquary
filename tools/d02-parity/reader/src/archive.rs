use crate::{
    CandidateKind,
    cut::{fields, observe_cut},
    evidence_encoding::{hex, sha256},
};
use serde_json::{Value, json};
use std::path::Path;

mod values;

const SCHEMA: &str = "reliquary-d02-archive-cut-observation-v1";

/// Observe an explicit Archive cut. A successful observation still carries an
/// incomplete parity status because no source expectation has been admitted.
pub fn observe_archive_cut(kind: CandidateKind, path: &Path, cut: u64) -> Value {
    let report = json!({
        "schema_id": SCHEMA,
        "status": "INCOMPLETE",
        "archive_observation_status": "INCOMPLETE",
        "requested_archive_version": cut.to_string(),
        "owner": null,
        "candidate_sha256_before": null,
        "candidate_sha256_after": null,
        "candidate_unchanged": null,
        "archive_head": null,
        "projection": null,
        "projection_sha256": null,
        "reason": "",
    });
    observe_cut(kind, path, report, |observer| {
        let observation = observer.archive_cut(cut)?;
        let projection = values::projection(&observation);
        let mut preimage = b"reliquary:d02:archive-cut:v1\0".to_vec();
        preimage.extend(serde_json::to_vec(&projection).expect("Archive projection is JSON"));
        Ok(fields(json!({
            "archive_head": observation.archive_head.to_string(),
            "projection_sha256": hex(&sha256(&preimage)),
            "projection": projection,
            "archive_observation_status": "OBSERVED",
            "reason": "one_archive_cut_only; other families and admission incomplete",
        })))
    })
}
