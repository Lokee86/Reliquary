//! Common ordered Archive publication bindings from the historical accepted owner.
use super::{ArtifactSink, BatchWriter, ToolResult, VerifiedCopy, lower_hex};
use crate::Cva;
use crate::container::publication_envelope::{PublicationClock, decode};
use crate::historical_fingerprint::{canonical_value, hash_field};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const COMPARABLE_DOMAIN: &[u8] = b"reliquary:d02:archive-publication-bindings:v1\0";
const PREFIX_DOMAIN: &[u8] = b"reliquary:f03:archive-binding-prefixes:v1\0";

/// The ordered rows are exactly the normalized D02 binding projection. Each
/// row at A remains a member of every retained Archive history prefix >= A.
pub(super) fn observe(
    owner: &Cva,
    copy: &VerifiedCopy,
    sink: &mut ArtifactSink<'_>,
) -> ToolResult<Value> {
    let head = owner.archive_version();
    let versions = owner.archive.record_versions();
    if versions.len() as u64 != head {
        return Err("Archive publication count differs from its retained head".into());
    }
    let mut bindings = BatchWriter::new(sink, copy, "archive-bindings");
    let mut prefixes = Vec::with_capacity(versions.len() + 1);
    let mut prefix_hash = Sha256::new();
    prefix_hash.update(PREFIX_DOMAIN);
    prefixes.push(prefix_row(0, &prefix_hash));

    // The D02 JSON head digest has a known cut and one compact JSON array.
    // Stream the identical preimage without materializing duplicate rows.
    let mut comparable_hash = Sha256::new();
    comparable_hash.update(COMPARABLE_DOMAIN);
    comparable_hash.update(format!(r#"{{"archive_version":"{head}","bindings":["#));
    let mut previous_global = 0;
    for (index, &version) in versions.iter().enumerate() {
        let local = index as u64 + 1;
        if version.archive_version != local || version.global_version <= previous_global {
            return Err(
                "Archive accepted order is not a dense monotonic publication history".into(),
            );
        }
        previous_global = version.global_version;

        let payload = owner
            .container
            .read(version.record)
            .map_err(|_| "Archive binding carrier is unavailable")?;
        let envelope = decode(&payload)
            .map_err(|_| "Archive binding envelope is invalid")?
            .ok_or("Archive publication lacks an authenticated clock mode")?;
        if envelope.global_version != version.global_version {
            return Err("Archive carrier clock differs from accepted replay".into());
        }
        let time = owner.container.transaction_time_ns(version.global_version);
        let mode = match envelope.clock {
            PublicationClock::Allocate {
                transaction_time_ns,
            } => {
                if transaction_time_ns != time {
                    return Err("Archive allocation time differs from accepted global clock".into());
                }
                "allocate"
            }
            PublicationClock::BindReserved => "bind_reserved",
        };
        let (_, first) = owner
            .archive
            .read_publication(&owner.container, version)
            .map_err(|_| "Archive first-applying publication is unavailable")?;
        if first.archive_version > local || first.global_version > version.global_version {
            return Err("Archive publication points past its first-applying boundary".into());
        }
        let row = json!({
            "archive_version": local.to_string(),
            "global_version": version.global_version.to_string(),
            "transaction_time_ns": time.map(|value| value.to_string()),
            "clock_mode": mode,
            "first_applying_archive_version": first.archive_version.to_string(),
            "first_applying_global_version": first.global_version.to_string(),
        });
        let bytes = canonical_value(&row)?;
        if index > 0 {
            comparable_hash.update(b",");
        }
        comparable_hash.update(&bytes);
        hash_field(&mut prefix_hash, &bytes);
        prefixes.push(prefix_row(local, &prefix_hash));
        bindings.push(row)?;
    }
    comparable_hash.update(b"]}");
    let binding_stream = bindings.finish()?;
    let mut cuts = BatchWriter::new(sink, copy, "archive-binding-cuts");
    for cut in prefixes {
        cuts.push(cut)?;
    }
    Ok(json!({
        "schema_id": "reliquary-d02-archive-publication-bindings-v1",
        "cut_coverage": "exhaustive_ordered_prefixes",
        "archive_head": head.to_string(),
        "binding_projection_sha256": lower_hex(&comparable_hash.finalize()),
        "bindings": binding_stream,
        "cuts": cuts.finish()?,
    }))
}

fn prefix_row(cut: u64, hash: &Sha256) -> Value {
    json!({
        "archive_version": cut.to_string(),
        "binding_rows": cut.to_string(),
        "prefix_sha256": lower_hex(&hash.clone().finalize()),
        "availability": "retained",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_head_streaming_preimage_matches_common_projection() {
        let mut hash = Sha256::new();
        hash.update(COMPARABLE_DOMAIN);
        let head = 0;
        hash.update(format!(r#"{{"archive_version":"{head}","bindings":["#));
        hash.update(b"]}");
        let mut canonical = Sha256::new();
        canonical.update(COMPARABLE_DOMAIN);
        canonical.update(canonical_value(&json!({"archive_version":"0","bindings":[]})).unwrap());
        assert_eq!(hash.finalize(), canonical.finalize());
    }
}
