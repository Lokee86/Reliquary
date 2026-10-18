use crate::{
    CompatibilityProfileId, Cva, CvaError, PackedVectorError, PackedVectorId, PackedVectorInfo,
    PackedVectorStats,
};
use lodestone_packed::PackedVectors;
use std::collections::{HashMap, HashSet};

impl Cva {
    pub fn put_packed_vectors(
        &mut self,
        packed: PackedVectors,
    ) -> Result<PackedVectorId, PackedVectorError> {
        self.packed_vectors.put(&mut self.container, packed)
    }

    pub fn packed_vectors(
        &mut self,
        id: PackedVectorId,
    ) -> Result<PackedVectors, PackedVectorError> {
        self.packed_vectors.get(&mut self.container, id)
    }

    pub fn packed_vector_infos(&self) -> Vec<PackedVectorInfo> {
        self.packed_vectors.infos()
    }

    pub fn packed_vector_stats(&self) -> PackedVectorStats {
        self.packed_vectors.stats()
    }

    /// Packed matrices whose every persisted mapping belongs to a retired profile.
    /// The returned profile is a deterministic tombstone anchor for manifest evidence.
    pub(crate) fn prunable_packed_vector_profiles(
        &self,
    ) -> Vec<(PackedVectorId, CompatibilityProfileId)> {
        let mut refs = HashMap::<PackedVectorId, (HashSet<CompatibilityProfileId>, bool)>::new();
        for info in self.memory_vectors.infos() {
            refs.entry(info.packed_vector_id)
                .or_default()
                .0
                .insert(info.compatibility_profile_id);
        }
        let archive_info = self.archive_vectors.infos();
        for info in &archive_info {
            let generations: Vec<_> = self
                .vector_generations
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
        let mut candidates: Vec<_> = self
            .packed_vectors
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
                if self.compatibility_profiles.get(anchor).is_none()
                    || !self.compatibility_profiles.is_retired(anchor)
                    || profiles.into_iter().skip(1).any(|profile| {
                        self.compatibility_profiles.get(profile).is_none()
                            || !self.compatibility_profiles.is_retired(profile)
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

    pub(crate) fn packed_vector_prune_marker(
        &self,
        id: PackedVectorId,
        manifest_digest: [u8; 32],
    ) -> Result<Vec<u8>, PackedVectorError> {
        self.packed_vectors.marker_bytes(id, manifest_digest)
    }

    pub(crate) fn packed_vector_retention_descriptor(
        &mut self,
        profile_id: CompatibilityProfileId,
        packed_id: PackedVectorId,
        watermark: u64,
    ) -> Result<crate::history_prune_manifest::HistoryRetentionDescriptor, CvaError> {
        use crate::history_prune_manifest::{
            HistoryRetentionDescriptor, PrunedPayload, RetentionDomain, RetentionTarget,
        };
        use sha2::{Digest, Sha256};

        if !self
            .prunable_packed_vector_profiles()
            .contains(&(packed_id, profile_id))
            || self
                .vector_generations
                .generations()
                .iter()
                .any(|generation| {
                    generation.global_version > watermark
                        && self
                            .archive_vectors
                            .info(generation.archive_vector_id)
                            .is_some_and(|info| info.packed_vector_id == packed_id)
                })
        {
            return Err(CvaError::Repack(
                "packed vectors are still required by a live or post-watermark reference".into(),
            ));
        }
        let profile = self
            .compatibility_profiles
            .get(profile_id)
            .ok_or_else(|| CvaError::Repack("missing retired vector profile".into()))?;
        let profile_bytes = crate::compatibility_profile_codec::encode_profile(profile)
            .map_err(|error| CvaError::Repack(error.to_string()))?;
        let retirement = crate::compatibility_profile_codec::encode_retirement(profile_id);
        let mut structure = Sha256::new();
        hash_retention_part(&mut structure, &profile_bytes);
        hash_retention_part(&mut structure, &retirement);
        let mut referenced_profiles = HashSet::from([profile_id]);

        let memory_infos = self
            .memory_vectors
            .infos()
            .into_iter()
            .filter(|info| info.packed_vector_id == packed_id)
            .collect::<Vec<_>>();
        for info in memory_infos {
            referenced_profiles.insert(info.compatibility_profile_id);
            let set = self
                .memory_vectors
                .get(&mut self.container, info.id)
                .map_err(|error| CvaError::Repack(error.to_string()))?;
            hash_retention_part(&mut structure, b"memory-vector");
            hash_retention_part(&mut structure, &info.id.0);
            hash_retention_part(&mut structure, &info.compatibility_profile_id.0);
            hash_retention_part(&mut structure, &packed_id.0);
            structure.update(info.rows.to_le_bytes());
            for body_id in set.memory_body_ids {
                hash_retention_part(&mut structure, &body_id.0);
            }
        }

        let archive_infos = self
            .archive_vectors
            .infos()
            .into_iter()
            .filter(|info| info.packed_vector_id == packed_id)
            .collect::<Vec<_>>();
        for info in archive_infos {
            let set = self
                .archive_vectors
                .get(&mut self.container, info.id)
                .map_err(|error| CvaError::Repack(error.to_string()))?;
            hash_retention_part(&mut structure, b"archive-vector");
            hash_retention_part(&mut structure, &info.id.0);
            hash_retention_part(&mut structure, &packed_id.0);
            structure.update(info.rows.to_le_bytes());
            structure.update(info.max_fragment_archive_version.to_le_bytes());
            for fragment_id in set.fragment_ids {
                hash_retention_part(&mut structure, &fragment_id.0);
            }
            for generation in self
                .vector_generations
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
            if !self.compatibility_profiles.is_retired(id) {
                return Err(CvaError::Repack(
                    "packed vector is referenced by an active profile".into(),
                ));
            }
            let referenced = self.compatibility_profiles.get(id).ok_or_else(|| {
                CvaError::Repack("packed vector references an unknown profile".into())
            })?;
            let bytes = crate::compatibility_profile_codec::encode_profile(referenced)
                .map_err(|error| CvaError::Repack(error.to_string()))?;
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

    pub(crate) fn validate_pruned_packed_vectors(&mut self) -> Result<(), CvaError> {
        use crate::history_prune_manifest::{PrunedPayload, RetentionDomain};
        let ids = self.packed_vectors.pruned_ids();
        let candidates = self.prunable_packed_vector_profiles();
        let mut authorized = HashSet::new();
        for id in &ids {
            let profile = candidates
                .iter()
                .find(|(candidate, _)| candidate == id)
                .map(|(_, profile)| *profile)
                .ok_or_else(|| {
                    CvaError::Repack(
                        "pruned packed vector has an active, shared, or unknown reference".into(),
                    )
                })?;
            let digest = self
                .packed_vectors
                .prune_manifest_digest(*id)
                .ok_or_else(|| {
                    CvaError::Repack("pruned packed vector lacks manifest digest".into())
                })?;
            let manifest = self
                .archive
                .prune
                .manifest(&digest)
                .cloned()
                .ok_or_else(|| {
                    CvaError::Repack("pruned packed vector references missing manifest".into())
                })?;
            let descriptor = self.packed_vector_retention_descriptor(
                profile,
                *id,
                manifest.source_global_version,
            )?;
            if !manifest.authorizes(&descriptor.target, &PrunedPayload::PackedVectors(id.0))
                || !manifest.descriptors.contains(&descriptor)
                || !self.packed_vectors.marker_matches(*id, digest)
                || !authorized.insert((profile, *id))
            {
                return Err(CvaError::Repack(
                    "packed-vector prune marker is not authorized by its exact profile evidence"
                        .into(),
                ));
            }
        }
        let manifests = self
            .archive
            .prune
            .manifests()
            .iter()
            .map(|(digest, manifest)| (*digest, manifest.clone()))
            .collect::<Vec<_>>();
        for (digest, manifest) in manifests {
            for descriptor in &manifest.descriptors {
                for payload in &descriptor.payloads {
                    if let PrunedPayload::PackedVectors(raw_id) = payload {
                        let id = crate::PackedVectorId(*raw_id);
                        if !self.packed_vectors.marker_matches(id, digest)
                            || descriptor.target.domain != RetentionDomain::CompatibilityProfile
                            || descriptor.target.identity.len() != 32
                            || !authorized.contains(&(
                                CompatibilityProfileId(
                                    descriptor.target.identity.clone().try_into().unwrap(),
                                ),
                                id,
                            ))
                        {
                            return Err(CvaError::Repack(
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
}

fn hash_retention_part(hash: &mut sha2::Sha256, bytes: &[u8]) {
    use sha2::Digest;
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
