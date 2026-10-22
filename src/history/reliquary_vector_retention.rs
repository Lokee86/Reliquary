//! Read-only REL packed-vector retention proofs and marker evidence.

use crate::owner_lifecycle::{HistoryReadFailure, HistorySource, OwnerError};
use crate::owner_maintenance::{
    HistoryRetentionDescriptor, OwnerMaintenanceStore, PrunedPayload, RetentionDomain,
    RetentionTarget,
};
use crate::{CompatibilityProfileId, PackedVectorId};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

pub(crate) fn eligible_packed_vector_profiles(
    packed_vectors: &crate::packed_vector_store::PackedVectorStore,
    memory_vectors: &crate::memory_vector_store::MemoryVectorStore,
    archive_vectors: &crate::archive_vector_store::ArchiveVectorStore,
    vector_generations: &crate::vector_generation_store::VectorGenerationStore,
    compatibility_profiles: &crate::compatibility_profile_store::CompatibilityProfileStore,
) -> Vec<(PackedVectorId, CompatibilityProfileId)> {
    let mut refs = HashMap::<PackedVectorId, (HashSet<CompatibilityProfileId>, bool)>::new();
    for info in memory_vectors.infos() {
        refs.entry(info.packed_vector_id)
            .or_default()
            .0
            .insert(info.compatibility_profile_id);
    }
    let archive_info = archive_vectors.infos();
    for info in &archive_info {
        let generations: Vec<_> = vector_generations
            .generations()
            .iter()
            .filter(|generation| generation.archive_vector_id == info.id)
            .collect();
        // An unowned archive mapping is ambiguous and blocks destructive pruning.
        if generations.is_empty() {
            refs.entry(info.packed_vector_id).or_default().1 = true;
        } else {
            refs.entry(info.packed_vector_id).or_default().0.extend(
                generations
                    .into_iter()
                    .map(|generation| generation.compatibility_profile_id),
            );
        }
    }
    let mut candidates: Vec<_> = packed_vectors
        .infos()
        .into_iter()
        .filter_map(|info| {
            let (profiles, ambiguous) = refs.get(&info.id)?;
            if *ambiguous {
                return None;
            }
            let mut profiles = profiles.iter().copied().collect::<Vec<_>>();
            profiles.sort_by_key(|profile| profile.0);
            let anchor = profiles.first().copied()?;
            if compatibility_profiles.get(anchor).is_none()
                || !compatibility_profiles.is_retired(anchor)
                || profiles.into_iter().skip(1).any(|profile| {
                    compatibility_profiles.get(profile).is_none()
                        || !compatibility_profiles.is_retired(profile)
                })
            {
                return None;
            }
            Some((info.id, anchor))
        })
        .collect();
    candidates.sort_by_key(|(id, profile)| (id.0, profile.0));
    candidates
}

pub(crate) fn packed_vector_retention_marker(
    packed_vectors: &crate::packed_vector_store::PackedVectorStore,
    id: PackedVectorId,
    manifest_digest: [u8; 32],
) -> Result<Vec<u8>, crate::PackedVectorError> {
    packed_vectors.marker_bytes(id, manifest_digest)
}

pub(crate) fn packed_vector_retention_descriptor(
    source: &Result<HistorySource<'_>, HistoryReadFailure>,
    packed_vectors: &crate::packed_vector_store::PackedVectorStore,
    memory_vectors: &crate::memory_vector_store::MemoryVectorStore,
    archive_vectors: &crate::archive_vector_store::ArchiveVectorStore,
    compatibility_profiles: &crate::compatibility_profile_store::CompatibilityProfileStore,
    vector_generations: &crate::vector_generation_store::VectorGenerationStore,
    profile_id: CompatibilityProfileId,
    packed_id: PackedVectorId,
    watermark: u64,
) -> Result<HistoryRetentionDescriptor, OwnerError> {
    if !eligible_packed_vector_profiles(
        packed_vectors,
        memory_vectors,
        archive_vectors,
        vector_generations,
        compatibility_profiles,
    )
    .contains(&(packed_id, profile_id))
        || vector_generations.generations().iter().any(|generation| {
            generation.global_version > watermark
                && archive_vectors
                    .info(generation.archive_vector_id)
                    .is_some_and(|info| info.packed_vector_id == packed_id)
        })
    {
        return Err(OwnerError::Repack(
            "packed vectors are still required by a live or post-watermark reference".into(),
        ));
    }
    let profile = compatibility_profiles
        .get(profile_id)
        .ok_or_else(|| OwnerError::Repack("missing retired vector profile".into()))?;
    let profile_bytes = crate::compatibility_profile_codec::encode_profile(profile)
        .map_err(|error| OwnerError::Repack(error.to_string()))?;
    let retirement = crate::compatibility_profile_codec::encode_retirement(profile_id);
    let mut structure = Sha256::new();
    hash_retention_part(&mut structure, &profile_bytes);
    hash_retention_part(&mut structure, &retirement);
    let mut referenced_profiles = HashSet::from([profile_id]);

    let memory_infos = memory_vectors
        .infos()
        .into_iter()
        .filter(|info| info.packed_vector_id == packed_id)
        .collect::<Vec<_>>();
    for info in memory_infos {
        referenced_profiles.insert(info.compatibility_profile_id);
        let source = source.as_ref().map_err(|error| {
            OwnerError::Repack(format!("memory-vector error: Container({:?})", error.0))
        })?;
        let set = source
            .memory_vector_set(memory_vectors, info.id)
            .map_err(|error| OwnerError::Repack(error.to_string()))?;
        hash_retention_part(&mut structure, b"memory-vector");
        hash_retention_part(&mut structure, &info.id.0);
        hash_retention_part(&mut structure, &info.compatibility_profile_id.0);
        hash_retention_part(&mut structure, &packed_id.0);
        structure.update(info.rows.to_le_bytes());
        for body_id in set.memory_body_ids {
            hash_retention_part(&mut structure, &body_id.0);
        }
    }

    let archive_infos = archive_vectors
        .infos()
        .into_iter()
        .filter(|info| info.packed_vector_id == packed_id)
        .collect::<Vec<_>>();
    for info in archive_infos {
        let source = source.as_ref().map_err(|error| {
            OwnerError::Repack(format!("archive-vector error: Container({:?})", error.0))
        })?;
        let set = source
            .archive_vector_set(archive_vectors, info.id)
            .map_err(|error| OwnerError::Repack(error.to_string()))?;
        hash_retention_part(&mut structure, b"archive-vector");
        hash_retention_part(&mut structure, &info.id.0);
        hash_retention_part(&mut structure, &packed_id.0);
        structure.update(info.rows.to_le_bytes());
        structure.update(info.max_fragment_archive_version.to_le_bytes());
        for fragment_id in set.fragment_ids {
            hash_retention_part(&mut structure, &fragment_id.0);
        }
        for generation in vector_generations
            .generations()
            .iter()
            .filter(|generation| generation.archive_vector_id == info.id)
        {
            referenced_profiles.insert(generation.compatibility_profile_id);
            hash_retention_part(&mut structure, b"vector-generation");
            hash_retention_part(&mut structure, &generation.id.0);
            hash_retention_part(&mut structure, &generation.compatibility_profile_id.0);
            hash_retention_part(&mut structure, &generation.archive_vector_id.0);
            structure.update(generation.source_archive_version.to_le_bytes());
            structure.update(generation.global_version.to_le_bytes());
            structure.update(generation.vector_version.to_le_bytes());
        }
    }
    let mut referenced_profiles = referenced_profiles.into_iter().collect::<Vec<_>>();
    referenced_profiles.sort_by_key(|id| id.0);
    for id in referenced_profiles {
        if !compatibility_profiles.is_retired(id) {
            return Err(OwnerError::Repack(
                "packed vector is referenced by an active profile".into(),
            ));
        }
        let referenced = compatibility_profiles.get(id).ok_or_else(|| {
            OwnerError::Repack("packed vector references an unknown profile".into())
        })?;
        let bytes = crate::compatibility_profile_codec::encode_profile(referenced)
            .map_err(|error| OwnerError::Repack(error.to_string()))?;
        hash_retention_part(&mut structure, &bytes);
        hash_retention_part(
            &mut structure,
            &crate::compatibility_profile_codec::encode_retirement(id),
        );
    }

    Ok(HistoryRetentionDescriptor {
        target: RetentionTarget {
            domain: RetentionDomain::CompatibilityProfile,
            identity: profile_id.0.to_vec(),
        },
        tombstone_global_version: None,
        tombstone_domain_version: None,
        tombstone_digest: Sha256::digest(retirement).into(),
        retained_structure_digest: structure.finalize().into(),
        payloads: vec![PrunedPayload::PackedVectors(packed_id.0)],
    })
}

pub(crate) fn validate_packed_vector_retention(
    source: Result<HistorySource<'_>, HistoryReadFailure>,
    packed_vectors: &crate::packed_vector_store::PackedVectorStore,
    memory_vectors: &crate::memory_vector_store::MemoryVectorStore,
    archive_vectors: &crate::archive_vector_store::ArchiveVectorStore,
    compatibility_profiles: &crate::compatibility_profile_store::CompatibilityProfileStore,
    vector_generations: &crate::vector_generation_store::VectorGenerationStore,
    maintenance: &OwnerMaintenanceStore,
) -> Result<(), OwnerError> {
    let ids = packed_vectors.pruned_ids();
    let candidates = eligible_packed_vector_profiles(
        packed_vectors,
        memory_vectors,
        archive_vectors,
        vector_generations,
        compatibility_profiles,
    );
    let mut authorized = HashSet::new();
    for id in &ids {
        let profile = candidates
            .iter()
            .find(|(candidate, _)| candidate == id)
            .map(|(_, profile)| *profile)
            .ok_or_else(|| {
                OwnerError::Repack(
                    "pruned packed vector has an active, shared, or unknown reference".into(),
                )
            })?;
        let digest = packed_vectors.pruned_manifest_digest(*id).ok_or_else(|| {
            OwnerError::Repack("pruned packed vector lacks manifest digest".into())
        })?;
        let manifest = maintenance.get(&digest).cloned().ok_or_else(|| {
            OwnerError::Repack("pruned packed vector references missing manifest".into())
        })?;
        let descriptor = packed_vector_retention_descriptor(
            &source,
            packed_vectors,
            memory_vectors,
            archive_vectors,
            compatibility_profiles,
            vector_generations,
            profile,
            *id,
            manifest.source_global_version,
        )?;
        if !manifest.authorizes(&descriptor.target, &PrunedPayload::PackedVectors(id.0))
            || !manifest.descriptors.contains(&descriptor)
            || !packed_vectors.marker_matches(*id, digest)
            || !authorized.insert((profile, *id))
        {
            return Err(OwnerError::Repack(
                "packed-vector prune marker is not authorized by its exact profile evidence".into(),
            ));
        }
    }
    let manifests = maintenance
        .manifests()
        .iter()
        .map(|(digest, manifest)| (*digest, manifest.clone()))
        .collect::<Vec<_>>();
    for (digest, manifest) in manifests {
        for descriptor in &manifest.descriptors {
            for payload in &descriptor.payloads {
                if let PrunedPayload::PackedVectors(raw_id) = payload {
                    let id = PackedVectorId(*raw_id);
                    if !packed_vectors.marker_matches(id, digest)
                        || descriptor.target.domain != RetentionDomain::CompatibilityProfile
                        || descriptor.target.identity.len() != 32
                        || !authorized.contains(&(
                            CompatibilityProfileId(
                                descriptor.target.identity.clone().try_into().unwrap(),
                            ),
                            id,
                        ))
                    {
                        return Err(OwnerError::Repack(
                            "manifest authorizes a missing or mismatched packed-vector marker"
                                .into(),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn hash_retention_part(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owner_lifecycle::HistoryReadFailure;

    #[test]
    fn ineligible_retention_descriptor_refuses_before_reading_source() {
        let source: Result<HistorySource<'static>, HistoryReadFailure> =
            Err(HistoryReadFailure(crate::ContainerError::StaleIdentity));
        let packed = crate::packed_vector_store::PackedVectorStore::default();
        let memories = crate::memory_vector_store::MemoryVectorStore::default();
        let archives = crate::archive_vector_store::ArchiveVectorStore::default();
        let profiles = crate::compatibility_profile_store::CompatibilityProfileStore::default();
        let generations = crate::vector_generation_store::VectorGenerationStore::default();

        let error = packed_vector_retention_descriptor(
            &source,
            &packed,
            &memories,
            &archives,
            &profiles,
            &generations,
            CompatibilityProfileId([1; 32]),
            PackedVectorId([2; 32]),
            0,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            OwnerError::Repack(message)
                if message == "packed vectors are still required by a live or post-watermark reference"
        ));
    }

    #[test]
    fn empty_packed_retention_validation_does_not_read_source() {
        let source: Result<HistorySource<'static>, HistoryReadFailure> =
            Err(HistoryReadFailure(crate::ContainerError::StaleIdentity));
        let packed = crate::packed_vector_store::PackedVectorStore::default();
        let memories = crate::memory_vector_store::MemoryVectorStore::default();
        let archives = crate::archive_vector_store::ArchiveVectorStore::default();
        let profiles = crate::compatibility_profile_store::CompatibilityProfileStore::default();
        let generations = crate::vector_generation_store::VectorGenerationStore::default();
        let maintenance = crate::owner_maintenance::OwnerMaintenanceStore::default();

        validate_packed_vector_retention(
            source,
            &packed,
            &memories,
            &archives,
            &profiles,
            &generations,
            &maintenance,
        )
        .unwrap();
    }
}
