//! Test-only Vector Generation history and the immutable closure it references.
use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::{Cva, VectorGeneration};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct VectorHistory {
    owner: [u8; 16],
    generations: Vec<(VectorGeneration, Option<i64>)>,
    profiles: BTreeMap<[u8; 32], Value>,
    mappings: BTreeMap<[u8; 32], ([u8; 32], Value)>,
    matrices: BTreeMap<[u8; 32], Value>,
}

pub(crate) struct VectorCut {
    pub(crate) fingerprint: HistoricalFingerprint,
    pub(crate) current: BTreeMap<[u8; 32], VectorGeneration>,
}

impl VectorHistory {
    pub(crate) fn read(cva: &mut Cva) -> Result<Self, String> {
        let mut history = Self {
            owner: cva.owner_uuid().ok_or("Vector oracle needs owner UUID")?,
            generations: Vec::new(),
            profiles: BTreeMap::new(),
            mappings: BTreeMap::new(),
            matrices: BTreeMap::new(),
        };
        for generation in cva.vector_generations.generations() {
            if generation.source_archive_version == 0
                || generation.source_archive_version > cva.archive.archive_version()
            {
                return Err(crate::VectorGenerationError::SourceArchiveVersion.to_string());
            }
            crate::vector_generation_validation::validate_generation_reference(
                cva.archive.archive_version(),
                &cva.packed_vectors,
                &cva.archive_vectors,
                &cva.compatibility_profiles,
                generation,
            )
            .map_err(|e| e.to_string())?;
            history.generations.push((
                *generation,
                cva.container.transaction_time_ns(generation.global_version),
            ));
            let profile = cva
                .compatibility_profiles
                .get(generation.compatibility_profile_id)
                .ok_or("missing historical profile")?;
            let crate::CompatibilityProfile {
                id,
                dimensions,
                normalization,
                probe_suite_version,
                compatibility_policy_version,
                references,
            } = profile;
            history.profiles.entry(id.0).or_insert_with(|| json!({
                "id": id.0, "dimensions": dimensions, "normalization": normalization.tag(),
                "probe_suite_version": probe_suite_version, "compatibility_policy_version": compatibility_policy_version,
                "references": references.iter().map(|reference| {
                    let crate::CompatibilityProbeReference {mode, vector} = reference;
                    json!({"mode": mode.tag(), "vector_bits": vector.iter().map(|v| v.to_bits()).collect::<Vec<_>>()})
                }).collect::<Vec<_>>()
            }));
            if history
                .mappings
                .contains_key(&generation.archive_vector_id.0)
            {
                continue;
            }
            let mapping = cva
                .archive_vectors
                .get(&mut cva.container, generation.archive_vector_id)
                .map_err(|e| e.to_string())?;
            let crate::ArchiveVectorSet {
                id,
                packed_vector_id,
                fragment_ids,
            } = mapping;
            history.mappings.insert(
                id.0,
                (
                    packed_vector_id.0,
                    json!({
                        "id": id.0, "packed_vector_id": packed_vector_id.0,
                        "fragment_ids": fragment_ids.iter().map(|id| id.0).collect::<Vec<_>>()
                    }),
                ),
            );
            if history.matrices.contains_key(&packed_vector_id.0) {
                continue;
            }
            let info = cva
                .packed_vectors
                .info(packed_vector_id)
                .ok_or("missing historical matrix")?;
            let crate::PackedVectorInfo {
                id,
                schema,
                count,
                byte_len,
            } = info;
            let payload = if cva.packed_vectors.is_pruned(id) {
                Value::Null
            } else {
                let packed = cva
                    .packed_vectors
                    .get(&mut cva.container, id)
                    .map_err(|e| e.to_string())?;
                json!(hex(packed.bytes()))
            };
            let proof = cva.packed_vectors.pruned_manifest_digest(id);
            if payload.is_null() && proof.is_none() {
                return Err("pruned matrix lacks retained proof".into());
            }
            history.matrices.insert(
                id.0,
                json!({
                    "id": id.0, "dimensions": schema.dimensions, "scalar": schema.scalar.tag(),
                    "count": count, "byte_len": byte_len, "payload_hex": payload,
                    "pruned": proof.is_some(), "prune_manifest_digest": proof
                }),
            );
        }
        if history.generations.len() as u64 != cva.vector_version() {
            return Err("Vector oracle publication count differs from accepted head".into());
        }
        Ok(history)
    }

    pub(crate) fn at(&self, cut: u64) -> Result<VectorCut, String> {
        if cut > self.generations.len() as u64 {
            return Err("Vector cut exceeds retained head".into());
        }
        let mut projection = OwnerProjection::new(self.owner, "vector_generation", cut, None)?;
        let mut current = BTreeMap::new();
        let mut profiles = BTreeSet::new();
        let mut mappings = BTreeSet::new();
        let mut matrices = BTreeSet::new();
        for (generation, time) in self.generations.iter().take(cut as usize) {
            let VectorGeneration {
                id,
                compatibility_profile_id,
                archive_vector_id,
                source_archive_version,
                global_version,
                vector_version,
            } = *generation;
            projection.insert("generation", vector_version.to_be_bytes(), json!({
                "id": id.0, "compatibility_profile_id": compatibility_profile_id.0,
                "archive_vector_id": archive_vector_id.0, "source_archive_version": source_archive_version,
                "global_version": global_version, "vector_version": vector_version, "time_ns": time
            }))?;
            current.insert(compatibility_profile_id.0, *generation);
            profiles.insert(compatibility_profile_id.0);
            mappings.insert(archive_vector_id.0);
            matrices.insert(self.mappings[&archive_vector_id.0].0);
        }
        for id in profiles {
            projection.insert("profile", id, self.profiles[&id].clone())?;
        }
        for id in mappings {
            projection.insert("archive_vectors", id, self.mappings[&id].1.clone())?;
        }
        for id in matrices {
            projection.insert("packed_vectors", id, self.matrices[&id].clone())?;
        }
        Ok(VectorCut {
            fingerprint: projection.fingerprint()?,
            current,
        })
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 15) as usize] as char);
    }
    out
}

#[path = "historical_vector_generation_fingerprint_tests.rs"]
mod tests;
