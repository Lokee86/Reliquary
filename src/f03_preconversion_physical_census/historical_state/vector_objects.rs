//! Stable decoded vector object witnesses independent of physical locations.
use super::{ToolResult, lower_hex, sha256_bytes};
use crate::Container;
use crate::archive_vector_store::ArchiveVectorStore;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::memory_vector_store::MemoryVectorStore;
use crate::packed_vector_store::PackedVectorStore;
use serde_json::json;

pub(super) fn observe(
    container: &Container,
    packed: &PackedVectorStore,
    memory: &MemoryVectorStore,
    profiles: &CompatibilityProfileStore,
    archive: Option<&ArchiveVectorStore>,
) -> ToolResult<HistoricalFingerprint> {
    let owner = container
        .owner_uuid()
        .ok_or("vector objects need typed owner UUID")?;
    let mut evidence = OwnerProjection::new(owner, "vector_objects", 0, None)?;
    for info in packed.infos() {
        let pruned = packed.is_pruned(info.id);
        let payload_sha256 = if pruned {
            None
        } else {
            let payload = packed
                .get(container, info.id)
                .map_err(|_| "invalid retained vector matrix")?;
            Some(sha256_bytes(payload.bytes()))
        };
        evidence.insert("packed_matrix", info.id.0, json!({
            "dimensions": info.schema.dimensions.to_string(),
            "scalar": info.schema.scalar.tag(),
            "rows": info.count.to_string(),
            "byte_length": info.byte_len.to_string(),
            "payload_sha256": payload_sha256,
            "pruned": pruned,
            "prune_manifest_sha256": packed.pruned_manifest_digest(info.id).map(|hash| lower_hex(&hash)),
        }))?;
    }
    for info in memory.infos() {
        let set = memory
            .get(container, info.id)
            .map_err(|_| "invalid retained Memory-vector binding")?;
        evidence.insert("memory_vector_set", set.id.0, json!({
            "compatibility_profile_id": lower_hex(&set.compatibility_profile_id.0),
            "packed_vector_id": lower_hex(&set.packed_vector_id.0),
            "memory_body_ids": set.memory_body_ids.iter().map(|id| lower_hex(&id.0)).collect::<Vec<_>>(),
        }))?;
    }
    for profile in profiles.profiles() {
        evidence.insert("compatibility_profile", profile.id.0, json!({
            "dimensions": profile.dimensions.to_string(),
            "normalization": profile.normalization.tag(),
            "probe_suite_version": profile.probe_suite_version.to_string(),
            "compatibility_policy_version": profile.compatibility_policy_version.to_string(),
            "retired": profiles.is_retired(profile.id),
            "references": profile.references.iter().map(|r| json!({
                "mode": r.mode.tag(),
                "float32_le": r.vector.iter().map(|f| lower_hex(&f.to_bits().to_le_bytes())).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        }))?;
    }
    if let Some(archive) = archive {
        for info in archive.infos() {
            let set = archive
                .get(container, info.id)
                .map_err(|_| "invalid retained Archive-vector binding")?;
            evidence.insert("archive_vector_set", set.id.0, json!({
                "packed_vector_id": lower_hex(&set.packed_vector_id.0),
                "fragment_ids": set.fragment_ids.iter().map(|id| lower_hex(&id.0)).collect::<Vec<_>>(),
                "max_fragment_archive_version": info.max_fragment_archive_version.to_string(),
            }))?;
        }
    }
    evidence.fingerprint()
}
