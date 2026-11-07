use crate::{
    CandidateKind,
    evidence_encoding::{hex, sha256},
};
use reliquary_owner_session::ReadOnlyCurrentOwnerObserver;
use serde_json::{Value, json};
use std::path::Path;

mod values;

const SCHEMA: &str = "reliquary-d02-archive-cut-observation-v1";

/// Observe an explicit Archive cut. A successful observation still carries an
/// incomplete parity status because no source expectation has been admitted.
pub fn observe_archive_cut(kind: CandidateKind, path: &Path, cut: u64) -> Value {
    let mut report = json!({
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
    let observer = match ReadOnlyCurrentOwnerObserver::open(path) {
        Ok(observer) => observer,
        Err(reason) => {
            report["reason"] = json!(reason);
            return report;
        }
    };
    let observation = observer.observation();
    report["owner"] = json!({
        "kind": observation.kind.as_str(),
        "owner_uuid": hex(&observation.owner_id),
        "scope": format!("{:?}", observation.scope).to_ascii_lowercase(),
        "history_generation_sha256": hex(&observation.history_generation),
    });
    report["candidate_sha256_before"] = json!(hex(&observation.file_sha256_before));
    let archive = if kind.matches(observation.kind) {
        observer.archive_cut(cut)
    } else {
        Err("requested_candidate_kind_mismatch".into())
    };
    match observer.finalize() {
        Ok(finalization) => {
            report["candidate_sha256_after"] = json!(hex(&finalization.file_sha256_after));
            report["candidate_unchanged"] = json!(finalization.candidate_unchanged);
            if !finalization.candidate_unchanged {
                report["reason"] = json!("candidate_changed_during_observation");
                return report;
            }
        }
        Err(reason) => {
            report["reason"] = json!(reason);
            return report;
        }
    }
    match archive {
        Ok(observation) => {
            let projection = values::projection(&observation);
            let mut preimage = b"reliquary:d02:archive-cut:v1\0".to_vec();
            preimage.extend(serde_json::to_vec(&projection).expect("Archive projection is JSON"));
            report["archive_head"] = json!(observation.archive_head.to_string());
            report["projection_sha256"] = json!(hex(&sha256(&preimage)));
            report["projection"] = projection;
            report["archive_observation_status"] = json!("OBSERVED");
            report["reason"] =
                json!("one_archive_cut_only; other families and admission incomplete");
        }
        Err(reason) => report["reason"] = json!(reason),
    }
    report
}
