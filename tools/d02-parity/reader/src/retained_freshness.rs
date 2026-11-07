use crate::{
    CandidateKind,
    cut::{fields, observe_cut},
    evidence_encoding::{hex, sha256},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;

pub fn observe_retained_freshness(kind: CandidateKind, path: &Path) -> Value {
    let report = json!({
        "schema_id": "reliquary-d02-retained-freshness-v1",
        "status": "INCOMPLETE",
        "observation_status": "INCOMPLETE",
        "owner": null,
        "candidate_sha256_before": null,
        "candidate_sha256_after": null,
        "candidate_unchanged": null,
        "projection": null,
        "reason": "",
    });
    observe_cut(kind, path, report, |observer| {
        let (authority, snapshot) = observer.retained_freshness_authority()?;
        let proof_sha = sha256(authority.source_proof());
        if proof_sha != authority.source_sha256
            || proof_sha != snapshot.evidence_digest
            || authority.import_id != snapshot.import_id
            || snapshot.owner_uuid != observer.observation().owner_id
        {
            return Err("current_freshness_authority_identity_differs".into());
        }
        let mut records = Sha256::new();
        records.update(b"reliquary:d02:retained-freshness-anchors:v1\0");
        for (identity, record) in &snapshot.records {
            records.update(identity.0);
            records.update(record.score.to_le_bytes());
            for anchor in [record.admitted_at_turn, record.accounted_turn] {
                records.update([u8::from(anchor.is_some())]);
                if let Some(value) = anchor {
                    records.update(value.to_le_bytes());
                }
            }
        }
        Ok(fields(json!({
            "observation_status": "OBSERVED",
            "reason": "retained_freshness_authority_and_settlement_anchor_population",
            "projection": {
                "owner_uuid": hex(&snapshot.owner_uuid), "import_id": snapshot.import_id,
                "source_payload_sha256": hex(&proof_sha),
                "base_cut": snapshot.base_cut.to_string(), "settlement_cut": snapshot.settlement_cut.to_string(),
                "global_version": authority.global_version.to_string(),
                "archive_version": authority.archive_version.to_string(),
                "graph_version": authority.graph_version.to_string(),
                "memory_graph_version": authority.memory_graph_version.to_string(),
                "journal_entries_included": snapshot.journal_entries_included.to_string(),
                "records": snapshot.records.len().to_string(), "records_sha256": hex(&records.finalize()),
            },
        })))
    })
}
