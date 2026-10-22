//! Reconstruct and validate PHY prune evidence from immutable owner stores.

use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::memory_vector_store::MemoryVectorStore;
use crate::owner_lifecycle::{HistorySource, OwnerPhylacteryError};
use crate::owner_maintenance::{
    HistoryRetentionDescriptor, OwnerMaintenanceStore, PrunedPayload, RetentionDomain,
    RetentionTarget,
};
use crate::packed_vector_store::PackedVectorStore;
use crate::relationship_store::RelationshipStore;
use crate::{CompatibilityProfileId, EntityId, RelationshipId};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(crate) fn entity_retention(
    entities: &EntityStore,
    id: EntityId,
) -> Result<HistoryRetentionDescriptor, OwnerPhylacteryError> {
    let retained = entities.retention_descriptor(id)?;
    let mut structure = b"Entity retained structure v1".to_vec();
    structure.push(u8::from(retained.replacement_id.is_some()));
    if let Some(replacement) = retained.replacement_id {
        structure.extend(replacement.0);
    }
    for original in entities.records().iter().filter(|record| record.id == id) {
        let raw = crate::entity_retained_structure::encode_record_structure(original)?;
        encode_part(&mut structure, &raw);
        structure.extend(original.global_version.to_le_bytes());
        structure.extend(original.entity_version.to_le_bytes());
    }
    let mut payloads = retained
        .revisions
        .iter()
        .map(|revision| PrunedPayload::EntityDescription(id.0, revision.revision))
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

pub(crate) fn relationship_retention(
    relationships: &RelationshipStore,
    id: RelationshipId,
) -> Result<HistoryRetentionDescriptor, OwnerPhylacteryError> {
    let retained = relationships.retention_descriptor(id)?;
    let mut structure = b"Relationship retained structure v1".to_vec();
    for record in relationships
        .records()
        .iter()
        .filter(|record| record.id == id)
    {
        let raw = crate::relationship_retained_structure::encode_record_structure(record)?;
        encode_part(&mut structure, &raw);
        structure.extend(record.global_version.to_le_bytes());
        structure.extend(record.relationship_version.to_le_bytes());
    }
    let mut payloads = retained
        .revisions
        .iter()
        .map(|revision| PrunedPayload::RelationshipDescription(id.0, revision.revision))
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

pub(crate) fn profile_retention(
    source: &HistorySource<'_>,
    profiles: &CompatibilityProfileStore,
    memory_vectors: &MemoryVectorStore,
    packed_vectors: &PackedVectorStore,
    profile_id: CompatibilityProfileId,
) -> Result<HistoryRetentionDescriptor, OwnerPhylacteryError> {
    if !profiles.is_retired(profile_id) {
        return Err(OwnerPhylacteryError::Repack(
            "packed vector is referenced by an active profile".into(),
        ));
    }
    let profile = profiles
        .get(profile_id)
        .ok_or_else(|| OwnerPhylacteryError::Repack("unknown compatibility profile".into()))?;
    let mut packed_ids = memory_vectors
        .infos()
        .into_iter()
        .filter(|info| info.compatibility_profile_id == profile_id)
        .map(|info| info.packed_vector_id)
        .collect::<Vec<_>>();
    packed_ids.sort_by_key(|id| id.0);
    packed_ids.dedup();
    if packed_ids.is_empty() {
        return Err(OwnerPhylacteryError::Repack(
            "retired profile has no packed payload".into(),
        ));
    }
    let all_refs = memory_vectors.infos();
    if packed_ids.iter().any(|id| {
        all_refs
            .iter()
            .any(|info| info.packed_vector_id == *id && info.compatibility_profile_id != profile_id)
    }) {
        return Err(OwnerPhylacteryError::Repack(
            "packed vector is shared by multiple profiles".into(),
        ));
    }
    let mut structure = Sha256::new();
    let profile_bytes = crate::compatibility_profile_codec::encode_profile(profile)
        .map_err(|error| OwnerPhylacteryError::Repack(error.to_string()))?;
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
            let set = source
                .memory_vector_set(memory_vectors, info.id)
                .map_err(OwnerPhylacteryError::MemoryVectors)?;
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
        tombstone_digest: Sha256::digest(crate::compatibility_profile_codec::encode_retirement(
            profile_id,
        ))
        .into(),
        retained_structure_digest: structure.finalize().into(),
        payloads,
    })
}

pub(crate) fn validate_phylactery_prune_evidence(
    maintenance: &OwnerMaintenanceStore,
    source: &HistorySource<'_>,
    graph: &GraphStore,
    memories: &MemoryStore,
    entities: &EntityStore,
    relationships: &RelationshipStore,
    profiles: &CompatibilityProfileStore,
    memory_vectors: &MemoryVectorStore,
    packed_vectors: &PackedVectorStore,
) -> Result<(), OwnerPhylacteryError> {
    let invalid = || OwnerPhylacteryError::Repack("PHY prune evidence mismatch".into());
    crate::history::validate_source_history(maintenance, source, graph)
        .map_err(OwnerPhylacteryError::Repack)?;
    for manifest in maintenance.manifests().values() {
        if manifest
            .descriptors
            .iter()
            .any(|descriptor| descriptor.target.domain == RetentionDomain::Archive)
        {
            return Err(invalid());
        }
    }

    crate::history::memory_retention::validate(memories, maintenance)?;
    let mut authorized = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let manifests = maintenance
        .manifests()
        .iter()
        .map(|(digest, manifest)| (*digest, manifest.clone()))
        .collect::<Vec<_>>();
    for (digest, manifest) in &manifests {
        for descriptor in &manifest.descriptors {
            match descriptor.target.domain {
                RetentionDomain::Entity => {
                    let id = EntityId(fixed_identity(&descriptor.target.identity)?);
                    if entity_retention(entities, id)? != *descriptor {
                        return Err(invalid());
                    }
                }
                RetentionDomain::Relationship => {
                    let id = RelationshipId(fixed_identity(&descriptor.target.identity)?);
                    if relationship_retention(relationships, id)? != *descriptor {
                        return Err(invalid());
                    }
                }
                RetentionDomain::CompatibilityProfile => {
                    let profile =
                        CompatibilityProfileId(fixed_identity(&descriptor.target.identity)?);
                    if profile_retention(source, profiles, memory_vectors, packed_vectors, profile)?
                        != *descriptor
                    {
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
    let mut cursor = source.cursor().map_err(|error| error.0)?;
    while let Some(frame) = cursor.next_frame().map_err(|error| error.0)? {
        let bytes = frame.payload;
        let compact = crate::entity_description_proof::compact_description_proof(&bytes)?;
        let relationship_compact =
            crate::relationship_description_proof::compact_description_proof(&bytes)?;
        let payload = if let Some((id, revision, _)) = &relationship_compact {
            if !relationships.pruned_revisions.contains(&(*id, *revision)) {
                return Err(invalid());
            }
            Some(PrunedPayload::RelationshipDescription(id.0, *revision))
        } else if let Some((id, revision, _)) = &compact {
            if !entities.pruned_revisions.contains(&(*id, *revision)) {
                return Err(invalid());
            }
            Some(PrunedPayload::EntityDescription(id.0, *revision))
        } else {
            None
        };
        if let Some(payload) = payload {
            let digest = compact
                .map(|(_, _, proof)| proof.manifest_digest)
                .or_else(|| relationship_compact.map(|(_, _, proof)| proof.manifest_digest))
                .ok_or_else(invalid)?;
            if !authorized.contains(&(payload.clone(), digest)) || !seen.insert((payload, digest)) {
                return Err(invalid());
            }
        }
    }
    if packed_vectors.pruned_ids().iter().any(|id| {
        let Some(digest) = packed_vectors.pruned_manifest_digest(*id) else {
            return true;
        };
        !maintenance.get(&digest).is_some_and(|manifest| {
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
    for (digest, manifest) in maintenance.manifests() {
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
                            || !packed_vectors.marker_matches(id, *digest)
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

fn fixed_identity(value: &[u8]) -> Result<[u8; 32], OwnerPhylacteryError> {
    value
        .try_into()
        .map_err(|_| OwnerPhylacteryError::Repack("invalid maintenance target identity".into()))
}

fn encode_part(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend((bytes.len() as u64).to_le_bytes());
    out.extend(bytes);
}

fn hash_part(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
