//! Authenticated transformed-copy pruning for Phylactery-owned terminal payloads.
use crate::owner_lifecycle::LifecycleTarget;
use crate::owner_maintenance::{
    HistoryPruneManifest, HistoryRetentionDescriptor, PrunedPayload, RetentionDomain,
    RetentionTarget,
};
use crate::{
    CompatibilityProfileId, EntityId, MemoryBodyId, Phylactery, PhylacteryError, RelationshipId,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;

impl Phylactery {
    /// Write a separately validated copy with explicitly selected terminal PHY payloads removed.
    pub fn prune_tombstoned_history(
        &mut self,
        output_path: impl AsRef<Path>,
        request: crate::HistoryPruneRequest,
    ) -> Result<crate::HistoryPruneReport, PhylacteryError> {
        let output_path = output_path.as_ref();
        if output_path == self.container.path() || output_path.exists() {
            return Err(PhylacteryError::Repack(
                "prune output must be a new path separate from the source".into(),
            ));
        }
        self.container.sync()?;
        self.container.validate_path_identity()?;
        if !self.owner_maintenance.is_empty() {
            return Err(PhylacteryError::Repack(
                "additional pruning of an already pruned owner is unsupported".into(),
            ));
        }
        let owner_uuid = self.owner_uuid().ok_or_else(|| {
            PhylacteryError::Repack("pruning requires a durable owner UUID".into())
        })?;
        let watermark = self.latest_global_version();
        let mut memories = BTreeSet::new();
        let mut entities = BTreeSet::new();
        let mut relationships = BTreeSet::new();
        let mut profiles = HashSet::new();
        for target in request.targets {
            let changed = match target {
                LifecycleTarget::Memory(id) => memories.insert(id),
                LifecycleTarget::Entity(id) => entities.insert(id),
                LifecycleTarget::Relationship(id) => relationships.insert(id),
                LifecycleTarget::CompatibilityProfile(id) => profiles.insert(id),
                _ => {
                    return Err(PhylacteryError::Repack(
                        "target has no PHY retention contract".into(),
                    ));
                }
            };
            if !changed {
                return Err(PhylacteryError::Repack("duplicate prune target".into()));
            }
        }
        if memories.is_empty()
            && entities.is_empty()
            && relationships.is_empty()
            && profiles.is_empty()
        {
            return Err(PhylacteryError::Repack(
                "pruning requires explicitly selected terminal targets".into(),
            ));
        }

        let mut selected_bodies = HashMap::<crate::ObjectRef, (MemoryBodyId, u64)>::new();
        let mut descriptors = Vec::new();
        for id in &memories {
            let retained = self.memories.retention_descriptor(*id)?;
            let mut bodies = Vec::new();
            for body in retained.prunable_body_ids {
                if self
                    .memories
                    .records()
                    .iter()
                    .any(|record| record.body_id == body && record.id != *id)
                {
                    return Err(PhylacteryError::Repack(
                        "Memory body is shared outside the selected tombstone".into(),
                    ));
                }
                let object = self.memories.prune_body_chunk(body).ok_or_else(|| {
                    PhylacteryError::Repack("Memory body is not independently removable".into())
                })?;
                let raw = self.container.read(object)?;
                let Some((decoded, bytes)) = crate::memory_codec::decode_body(&raw)? else {
                    return Err(PhylacteryError::Repack(
                        "Memory body is retained inside grouped publication evidence".into(),
                    ));
                };
                if decoded != body
                    || selected_bodies
                        .insert(object, (body, bytes.len() as u64))
                        .is_some()
                {
                    return Err(PhylacteryError::Repack(
                        "invalid or duplicate Memory body".into(),
                    ));
                }
                bodies.push(body);
            }
            descriptors.push(crate::memory_prune_admission::descriptor(
                &self.memories,
                *id,
                &bodies,
            )?);
        }
        for id in &entities {
            let _ = self.entities.retention_descriptor(*id)?;
            descriptors.push(self.entity_retention(*id)?);
        }
        for id in &relationships {
            let _ = self.relationships.retention_descriptor(*id)?;
            descriptors.push(self.relationship_retention(*id)?);
        }
        for profile in &profiles {
            descriptors.push(self.profile_retention(*profile)?);
        }
        descriptors.sort_by(|a, b| (&a.target, &a.payloads).cmp(&(&b.target, &b.payloads)));
        let manifest = HistoryPruneManifest {
            owner_uuid,
            source_global_version: watermark,
            source_history_digest: crate::owner_history_proof::owner_history_digest(
                &mut self.container,
                watermark,
                &self.graph,
            )
            .map_err(PhylacteryError::Repack)?,
            descriptors,
        };
        let manifest_bytes = manifest.encode().map_err(PhylacteryError::Repack)?;
        let manifest_digest = manifest.digest().map_err(PhylacteryError::Repack)?;
        let mut description_objects = HashMap::new();
        let mut packed_objects = HashMap::new();
        let selected_entities = entities.iter().copied().collect::<HashSet<_>>();
        let selected_relationships = relationships.iter().copied().collect::<HashSet<_>>();
        let mut packed_ids = HashSet::new();
        for profile in &profiles {
            for descriptor in manifest.descriptors.iter().filter(|d| {
                d.target.domain == RetentionDomain::CompatibilityProfile
                    && d.target.identity == profile.0
            }) {
                for payload in &descriptor.payloads {
                    if let PrunedPayload::PackedVectors(raw) = payload {
                        packed_ids.insert(crate::PackedVectorId(*raw));
                    }
                }
            }
        }
        let entity_plan = crate::entity_description_prune::prepare_description_prune(
            &self.entities,
            &mut self.container,
            &selected_entities,
            manifest_digest,
        )?;
        description_objects.extend(entity_plan.replacements);
        let mut description_discard = entity_plan.discard;
        let relationship_plan = crate::relationship_description_prune::prepare_description_prune(
            &self.relationships,
            &mut self.container,
            &selected_relationships,
            manifest_digest,
        )?;
        description_objects.extend(relationship_plan.replacements);
        description_discard.extend(relationship_plan.discard);
        for object in self.container.chunks()? {
            let raw = self.container.read(object)?;
            if let Some(decoded) = crate::packed_vector_codec::decode_object(&raw)?
                && packed_ids.contains(&decoded.id)
            {
                if packed_objects.insert(decoded.id, object).is_some() {
                    return Err(PhylacteryError::Repack(
                        "duplicate packed-vector object identity".into(),
                    ));
                }
            }
        }
        if packed_objects.len() != packed_ids.len() {
            return Err(PhylacteryError::Repack(
                "selected packed payload is missing from source".into(),
            ));
        }
        let packed_markers = packed_ids
            .iter()
            .map(|id| Ok((*id, self.packed_vectors.marker_bytes(*id, manifest_digest)?)))
            .collect::<Result<HashMap<_, _>, PhylacteryError>>()?;
        let packed_by_object = packed_objects
            .iter()
            .map(|(id, object)| (*object, *id))
            .collect::<HashMap<_, _>>();
        let source_file_bytes = fs::metadata(self.container.path())
            .map_err(|e| PhylacteryError::Repack(e.to_string()))?
            .len();
        let source_objects = self.container.chunks()?;
        let mut output = self.container.create_empty_like(output_path)?;
        let output_guard = output.read_source()?;
        let copy = (|| {
            let mut relocated = BTreeMap::new();
            for object in &source_objects {
                if description_discard.contains(object) {
                    continue;
                }
                let raw = self.container.read(*object)?;
                let replacement = if let Some(bytes) = description_objects.get(object) {
                    Some(bytes.clone())
                } else if let Some((body, length)) = selected_bodies.get(object) {
                    Some(
                        crate::memory_store::history_prune::PrunedMemoryBody::encode(
                            *body,
                            *length,
                            manifest_digest,
                        ),
                    )
                } else if let Some(id) = packed_by_object.get(object) {
                    Some(packed_markers[id].clone())
                } else {
                    None
                };
                let payload = if let Some(replacement) = replacement {
                    replacement
                } else {
                    crate::storage_reclamation::relocate_payload(&raw, &relocated)
                        .map_err(PhylacteryError::Repack)?
                };
                let destination = output.append(&payload)?;
                relocated.insert(*object, destination);
            }
            output.append(&manifest_bytes)?;
            output.sync()?;
            Ok::<_, PhylacteryError>(())
        })();
        if let Err(error) = copy {
            drop(output);
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path);
            }
            return Err(error);
        }
        let reopened = Phylactery::open_from_container(output).map_err(|error| {
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path);
            }
            error
        })?;
        if phy_clocks(self) != phy_clocks(&reopened) || self.owner_uuid() != reopened.owner_uuid() {
            drop(reopened);
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path);
            }
            return Err(PhylacteryError::Repack(
                "pruned PHY changed owner or semantic clocks".into(),
            ));
        }
        drop(reopened);
        let output_file_bytes = fs::metadata(output_path)
            .map_err(|e| PhylacteryError::Repack(e.to_string()))?
            .len();
        Ok(crate::HistoryPruneReport {
            pruned_conversations: 0,
            pruned_content_objects: 0,
            pruned_memory_bodies: selected_bodies.len(),
            pruned_packed_vectors: packed_ids.len(),
            destroyed_body_bytes: selected_bodies.values().map(|(_, len)| *len).sum(),
            destroyed_packed_bytes: packed_ids
                .iter()
                .filter_map(|id| self.packed_vectors.info(*id))
                .map(|info| info.byte_len)
                .sum(),
            source_file_bytes,
            output_file_bytes,
            manifest_digest,
        })
    }

    pub(crate) fn validate_prune_evidence(&mut self) -> Result<(), PhylacteryError> {
        let invalid = || PhylacteryError::Repack("PHY prune evidence mismatch".into());
        crate::owner_history_proof::validate_source_history(
            &self.owner_maintenance,
            &mut self.container,
            &self.graph,
        )
        .map_err(PhylacteryError::Repack)?;
        for manifest in self.owner_maintenance.manifests().values() {
            if manifest
                .descriptors
                .iter()
                .any(|descriptor| descriptor.target.domain == RetentionDomain::Archive)
            {
                return Err(invalid());
            }
        }

        crate::memory_prune_admission::validate(&self.memories, &self.owner_maintenance)?;
        let mut authorized = BTreeSet::new();
        let mut seen = BTreeSet::new();
        let manifests = self
            .owner_maintenance
            .manifests()
            .iter()
            .map(|(digest, manifest)| (*digest, manifest.clone()))
            .collect::<Vec<_>>();
        for (digest, manifest) in &manifests {
            for descriptor in &manifest.descriptors {
                match descriptor.target.domain {
                    RetentionDomain::Entity => {
                        let id = EntityId(fixed_identity(&descriptor.target.identity)?);
                        let expected = self.entity_retention(id)?;
                        if expected != *descriptor {
                            return Err(invalid());
                        }
                    }
                    RetentionDomain::Relationship => {
                        let id = RelationshipId(fixed_identity(&descriptor.target.identity)?);
                        let expected = self.relationship_retention(id)?;
                        if expected != *descriptor {
                            return Err(invalid());
                        }
                    }
                    RetentionDomain::CompatibilityProfile => {
                        let profile =
                            CompatibilityProfileId(fixed_identity(&descriptor.target.identity)?);
                        let expected = self.profile_retention(profile)?;
                        if expected != *descriptor {
                            return Err(invalid());
                        }
                    }
                    RetentionDomain::Memory => {}
                    RetentionDomain::Archive => return Err(invalid()),
                }
                for payload in &descriptor.payloads {
                    authorized.insert((payload.clone(), *digest));
                }
            }
        }
        for object in self.container.chunks()? {
            let bytes = self.container.read(object)?;
            let compact = crate::entity_description_proof::compact_description_proof(&bytes)?;
            let relationship_compact =
                crate::relationship_description_proof::compact_description_proof(&bytes)?;
            let payload = if let Some((id, revision, _)) = &relationship_compact {
                if !self
                    .relationships
                    .pruned_revisions
                    .contains(&(*id, *revision))
                {
                    return Err(invalid());
                }
                Some(PrunedPayload::RelationshipDescription(id.0, *revision))
            } else if let Some((id, revision, _)) = &compact {
                if !self.entities.pruned_revisions.contains(&(*id, *revision)) {
                    return Err(invalid());
                }
                Some(PrunedPayload::EntityDescription(id.0, *revision))
            } else {
                None
            };
            if let Some(payload) = payload {
                let digest = if let Some(digest) = compact
                    .map(|(_, _, proof)| proof.manifest_digest)
                    .or_else(|| relationship_compact.map(|(_, _, proof)| proof.manifest_digest))
                {
                    digest
                } else {
                    return Err(invalid());
                };
                if !authorized.contains(&(payload.clone(), digest))
                    || !seen.insert((payload, digest))
                {
                    return Err(invalid());
                }
            }
        }
        if self.packed_vectors.pruned_ids().iter().any(|id| {
            let Some(digest) = self.packed_vectors.prune_manifest_digest(*id) else {
                return true;
            };
            !self.owner_maintenance.get(&digest).is_some_and(|manifest| {
                manifest.descriptors.iter().any(|descriptor| {
                    descriptor.target.domain == RetentionDomain::CompatibilityProfile
                        && descriptor
                            .payloads
                            .contains(&PrunedPayload::PackedVectors(id.0))
                })
            })
        }) {
            return Err(invalid());
        }
        for (digest, manifest) in self.owner_maintenance.manifests() {
            for descriptor in &manifest.descriptors {
                for payload in &descriptor.payloads {
                    match payload {
                        PrunedPayload::EntityDescription(..)
                        | PrunedPayload::RelationshipDescription(..) => {
                            if !seen.contains(&(payload.clone(), *digest)) {
                                return Err(invalid());
                            }
                        }
                        PrunedPayload::PackedVectors(raw) => {
                            let id = crate::PackedVectorId(*raw);
                            if descriptor.target.domain != RetentionDomain::CompatibilityProfile
                                || !self.packed_vectors.marker_matches(id, *digest)
                            {
                                return Err(invalid());
                            }
                        }
                        PrunedPayload::MemoryBody(_) => {}
                        PrunedPayload::ArchiveContent(_) | PrunedPayload::ProfileProbes(_) => {
                            return Err(invalid());
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn entity_retention(
        &self,
        id: EntityId,
    ) -> Result<HistoryRetentionDescriptor, PhylacteryError> {
        let retained = self.entities.retention_descriptor(id)?;
        let mut structure = b"Entity retained structure v1".to_vec();
        structure.push(u8::from(retained.replacement_id.is_some()));
        if let Some(replacement) = retained.replacement_id {
            structure.extend(replacement.0);
        }
        for original in self
            .entities
            .records()
            .iter()
            .filter(|record| record.id == id)
        {
            let raw = crate::entity_retained_structure::encode_record_structure(original)?;
            append_part(&mut structure, &raw);
            structure.extend(original.global_version.to_le_bytes());
            structure.extend(original.entity_version.to_le_bytes());
        }
        let mut payloads = retained
            .revisions
            .iter()
            .map(|r| PrunedPayload::EntityDescription(id.0, r.revision))
            .collect::<Vec<_>>();
        payloads.sort();
        Ok(HistoryRetentionDescriptor {
            target: RetentionTarget {
                domain: RetentionDomain::Entity,
                identity: id.0.to_vec(),
            },
            tombstone_global_version: Some(retained.tombstone_global_version),
            tombstone_domain_version: Some(retained.tombstone_entity_version),
            tombstone_digest: Sha256::digest(
                [
                    id.0.as_slice(),
                    &retained.tombstone_revision.to_le_bytes(),
                    &retained.tombstone_global_version.to_le_bytes(),
                    &retained.tombstone_entity_version.to_le_bytes(),
                ]
                .concat(),
            )
            .into(),
            retained_structure_digest: Sha256::digest(structure).into(),
            payloads,
        })
    }

    fn relationship_retention(
        &self,
        id: RelationshipId,
    ) -> Result<HistoryRetentionDescriptor, PhylacteryError> {
        let retained = self.relationships.retention_descriptor(id)?;
        let mut structure = b"Relationship retained structure v1".to_vec();
        for original in self
            .relationships
            .records()
            .iter()
            .filter(|record| record.id == id)
        {
            let record = original;
            let raw = crate::relationship_retained_structure::encode_record_structure(record)?;
            append_part(&mut structure, &raw);
            structure.extend(record.global_version.to_le_bytes());
            structure.extend(record.relationship_version.to_le_bytes());
        }
        let mut payloads = retained
            .revisions
            .iter()
            .map(|r| PrunedPayload::RelationshipDescription(id.0, r.revision))
            .collect::<Vec<_>>();
        payloads.sort();
        Ok(HistoryRetentionDescriptor {
            target: RetentionTarget {
                domain: RetentionDomain::Relationship,
                identity: id.0.to_vec(),
            },
            tombstone_global_version: Some(retained.tombstone_global_version),
            tombstone_domain_version: Some(retained.tombstone_relationship_version),
            tombstone_digest: Sha256::digest(
                [
                    id.0.as_slice(),
                    &retained.tombstone_revision.to_le_bytes(),
                    &retained.tombstone_global_version.to_le_bytes(),
                    &retained.tombstone_relationship_version.to_le_bytes(),
                ]
                .concat(),
            )
            .into(),
            retained_structure_digest: Sha256::digest(structure).into(),
            payloads,
        })
    }

    fn profile_retention(
        &mut self,
        profile_id: CompatibilityProfileId,
    ) -> Result<HistoryRetentionDescriptor, PhylacteryError> {
        if !self.compatibility_profiles.is_retired(profile_id) {
            return Err(PhylacteryError::Repack(
                "packed vector is referenced by an active profile".into(),
            ));
        }
        let profile = self
            .compatibility_profiles
            .get(profile_id)
            .ok_or_else(|| PhylacteryError::Repack("unknown compatibility profile".into()))?;
        let mut packed_ids = self
            .memory_vectors
            .infos()
            .into_iter()
            .filter(|info| info.compatibility_profile_id == profile_id)
            .map(|info| info.packed_vector_id)
            .collect::<Vec<_>>();
        packed_ids.sort_by_key(|id| id.0);
        packed_ids.dedup();
        if packed_ids.is_empty() {
            return Err(PhylacteryError::Repack(
                "retired profile has no packed payload".into(),
            ));
        }
        let all_refs = self.memory_vectors.infos();
        if packed_ids.iter().any(|id| {
            all_refs.iter().any(|info| {
                info.packed_vector_id == *id && info.compatibility_profile_id != profile_id
            })
        }) {
            return Err(PhylacteryError::Repack(
                "packed vector is shared by multiple profiles".into(),
            ));
        }
        let mut structure = Sha256::new();
        let profile_bytes = crate::compatibility_profile_codec::encode_profile(profile)
            .map_err(|e| PhylacteryError::Repack(e.to_string()))?;
        hash_part(&mut structure, &profile_bytes);
        hash_part(
            &mut structure,
            &crate::compatibility_profile_codec::encode_retirement(profile_id),
        );
        let mut payloads = Vec::new();
        for packed_id in packed_ids {
            let infos = all_refs
                .iter()
                .filter(|info| info.packed_vector_id == packed_id)
                .copied()
                .collect::<Vec<_>>();
            for info in infos {
                let set = self.memory_vectors.get(&mut self.container, info.id)?;
                hash_part(&mut structure, b"memory-vector");
                hash_part(&mut structure, &info.id.0);
                hash_part(&mut structure, &info.compatibility_profile_id.0);
                hash_part(&mut structure, &packed_id.0);
                structure.update(info.rows.to_le_bytes());
                for body in set.memory_body_ids {
                    hash_part(&mut structure, &body.0);
                }
            }
            payloads.push(PrunedPayload::PackedVectors(packed_id.0));
        }
        payloads.sort();
        Ok(HistoryRetentionDescriptor {
            target: RetentionTarget {
                domain: RetentionDomain::CompatibilityProfile,
                identity: profile_id.0.to_vec(),
            },
            tombstone_global_version: None,
            tombstone_domain_version: None,
            tombstone_digest: Sha256::digest(
                crate::compatibility_profile_codec::encode_retirement(profile_id),
            )
            .into(),
            retained_structure_digest: structure.finalize().into(),
            payloads,
        })
    }
}

fn fixed_identity(value: &[u8]) -> Result<[u8; 32], PhylacteryError> {
    value
        .try_into()
        .map_err(|_| PhylacteryError::Repack("invalid maintenance target identity".into()))
}

fn append_part(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend((bytes.len() as u64).to_le_bytes());
    out.extend(bytes);
}

fn hash_part(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn phy_clocks(owner: &Phylactery) -> (u64, u64, u64, u64, u64) {
    (
        owner.latest_global_version(),
        owner.memory_version(),
        owner.entity_version(),
        owner.relationship_version(),
        owner.graph_version(),
    )
}
