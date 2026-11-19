use crate::{CandidateKind, evidence_encoding::hex};
use reliquary_owner_session::ReadOnlyCurrentOwnerObserver;
use serde_json::{Value, json};
use std::path::Path;

pub(crate) fn observe_cut(
    kind: CandidateKind,
    path: &Path,
    mut report: Value,
    observe: impl FnOnce(
        &ReadOnlyCurrentOwnerObserver,
    ) -> Result<serde_json::Map<String, Value>, String>,
) -> Value {
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
    let observed = if kind.matches(observation.kind) {
        observe(&observer)
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
    match observed {
        Ok(fields) => report
            .as_object_mut()
            .expect("cut report is an object")
            .extend(fields),
        Err(reason) => report["reason"] = json!(reason),
    }
    report
}

pub(crate) fn fields(value: Value) -> serde_json::Map<String, Value> {
    let Value::Object(fields) = value else {
        unreachable!("projection fields are an object")
    };
    fields
}
