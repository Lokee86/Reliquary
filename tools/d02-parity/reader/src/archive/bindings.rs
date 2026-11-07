use crate::{
    CandidateKind,
    cut::{fields, observe_cut},
    evidence_encoding::{hex, sha256},
};
use serde_json::{Value, json};
use std::path::Path;

const SCHEMA: &str = "reliquary-d02-archive-binding-cut-observation-v1";

/// Require a complete accepted native and physical binding prefix at the
/// requested local cut. The result is still only an Archive observation.
pub fn observe_archive_binding_cut(kind: CandidateKind, path: &Path, cut: u64) -> Value {
    let report = json!({
        "schema_id": SCHEMA,
        "status": "INCOMPLETE",
        "archive_observation_status": "INCOMPLETE",
        "archive_binding_observation_status": "INCOMPLETE",
        "requested_archive_version": cut.to_string(),
        "owner": null,
        "candidate_sha256_before": null,
        "candidate_sha256_after": null,
        "candidate_unchanged": null,
        "archive_head": null,
        "projection": null,
        "projection_sha256": null,
        "binding_projection": null,
        "binding_projection_sha256": null,
        "reason": "",
    });
    observe_cut(kind, path, report, |observer| {
        let observation = observer.archive_cut(cut)?;
        let mut observed = super::projection_fields(&observation);
        let bindings = observer.archive_publication_bindings_at_cut(cut)?;
        let projection = json!({
            "archive_version": cut.to_string(),
            "bindings": bindings.iter().map(|binding| json!({
                "archive_version": binding.archive_version.to_string(),
                "global_version": binding.global_version.to_string(),
                "transaction_time_ns": binding.transaction_time_ns.map(|time| time.to_string()),
                "clock_mode": if binding.bound_reservation { "bind_reserved" } else { "allocate" },
                "first_applying_archive_version": binding.first_applying_archive_version.to_string(),
                "first_applying_global_version": binding.first_applying_global_version.to_string(),
            })).collect::<Vec<_>>(),
        });
        let mut preimage = b"reliquary:d02:archive-publication-bindings:v1\0".to_vec();
        preimage
            .extend(serde_json::to_vec(&projection).expect("Archive binding projection is JSON"));
        observed.extend(fields(json!({
            "binding_projection": projection,
            "binding_projection_sha256": hex(&sha256(&preimage)),
            "archive_binding_observation_status": "OBSERVED",
            "reason": "archive_cut_and_binding_prefix_only; other families and admission incomplete",
        })));
        Ok(observed)
    })
}
