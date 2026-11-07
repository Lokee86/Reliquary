//! Exhaustive activity-cut proofs from the accepted Freshness owner.
use super::{ArtifactSink, BatchWriter, ToolResult, VerifiedCopy, lower_hex};
use crate::Cva;
use crate::historical_freshness_fingerprint::freshness_at_turn;
use serde_json::{Value, json};

const INTRA_SETTLEMENT: &str =
    "import settlement cut does not retain an intra-settlement trajectory";

pub(super) fn observe(
    owner: &Cva,
    copy: &VerifiedCopy,
    sink: &mut ArtifactSink<'_>,
) -> ToolResult<Value> {
    let mut stream = BatchWriter::new(sink, copy, "freshness-activity");
    // Stored scores settle at the query turn even between accepted journal
    // events. Query the native proof at each activity instead of reusing a
    // nearby state or interpolating from the retained event boundaries.
    for activity in 0..=owner.rel_turn_count() {
        let state = match freshness_at_turn(owner, activity) {
            Ok(cut) => {
                let proof = cut.fingerprint;
                json!({
                    "availability": "supported",
                    "proof": {
                        "domain": proof.domain,
                        "local_cut": proof.local_cut.to_string(),
                        "event_cut": proof.event_cut.map(|coordinate| coordinate.to_string()),
                        "rows": proof.rows.to_string(),
                        "sha256": lower_hex(&proof.sha256),
                    },
                })
            }
            Err(error) if error == INTRA_SETTLEMENT => json!({
                "availability": "unavailable",
                "reason": "intra_settlement_trajectory_not_retained",
            }),
            Err(_) => return Err(format!("Freshness activity cut {activity} failed")),
        };
        stream.push(json!({ "activity": activity.to_string(), "state": state }))?;
    }
    stream.finish()
}
