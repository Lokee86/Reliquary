//! Exhaustive Relationship revision evidence for copied redacted descriptions.
use super::*;
use std::collections::BTreeSet;
pub(super) fn descriptor(
    perception: &PerceptionStore,
    id: RelationshipId,
    redacted: bool,
) -> Result<HistoryRetentionDescriptor, String> {
    let tomb = perception
        .relationship_tombstone(id)
        .ok_or("missing terminal Relationship tombstone")?;
    let mut hash = Sha256::new();
    hash.update(b"reliquary-relationship-retained-structure-v1\0");
    hash.update(id.0);
    let mut payloads = Vec::new();
    for revision in 1..=tomb.revision {
        let record = perception
            .relationship_revision(id, revision)
            .ok_or("incomplete Relationship revision history")?;
        let summary = if redacted {
            if !record.summary.is_empty() {
                return Err("Relationship description was retained".into());
            }
            perception
                .relationship_description_prune(id, revision)
                .ok_or("Relationship prune proof missing")?
                .summary_digest
        } else {
            if record.summary.is_empty()
                || perception
                    .relationship_description_prune(id, revision)
                    .is_some()
            {
                return Err("Relationship revision has no removable description".into());
            }
            Sha256::digest(record.summary.as_bytes()).into()
        };
        hash.update(revision.to_le_bytes());
        hash.update(record.relationship_version.to_le_bytes());
        hash_text(&mut hash, &record.kind);
        hash.update((record.participants.len() as u64).to_le_bytes());
        for participant in &record.participants {
            hash_text(&mut hash, &participant.entity.owner_id);
            hash.update(participant.entity.entity_id.0);
            match &participant.role {
                Some(role) => {
                    hash.update([1]);
                    hash_text(&mut hash, role)
                }
                None => hash.update([0]),
            }
        }
        hash.update((record.evidence.len() as u64).to_le_bytes());
        for mem in &record.evidence {
            hash_text(&mut hash, &mem.owner_id);
            hash.update(mem.memory_id);
        }
        hash_text(&mut hash, &record.mutation_id);
        hash.update(record.created_at_ns.to_le_bytes());
        hash.update(record.updated_at_ns.to_le_bytes());
        hash.update(summary);
        payloads.push(PrunedPayload::RelationshipDescription(id.0, revision));
    }
    let mut tombstone = Sha256::new();
    tombstone.update(b"reliquary-relationship-tombstone-v1\0");
    tombstone.update(id.0);
    tombstone.update(tomb.revision.to_le_bytes());
    tombstone.update(tomb.relationship_version.to_le_bytes());
    Ok(HistoryRetentionDescriptor {
        target: RetentionTarget {
            domain: RetentionDomain::Relationship,
            identity: id.0.to_vec(),
        },
        tombstone_global_version: None,
        tombstone_domain_version: Some(tomb.relationship_version),
        tombstone_digest: tombstone.finalize().into(),
        retained_structure_digest: hash.finalize().into(),
        payloads,
    })
}
pub(crate) fn validate_relationship_prune(
    perception: &PerceptionStore,
    maintenance: &OwnerMaintenanceStore,
) -> Result<(), OwnerSessionError> {
    let invalid =
        || OwnerSessionError::Record("Relationship prune retention evidence mismatch".into());
    let mut selected = BTreeSet::new();
    for (digest, manifest) in maintenance.manifests() {
        for descriptor in &manifest.descriptors {
            if descriptor.target.domain != RetentionDomain::Relationship {
                continue;
            }
            let bytes: [u8; 32] = descriptor
                .target
                .identity
                .as_slice()
                .try_into()
                .map_err(|_| invalid())?;
            let id = RelationshipId(bytes);
            if self::descriptor(perception, id, true).map_err(|_| invalid())? != *descriptor {
                return Err(invalid());
            }
            for payload in &descriptor.payloads {
                let PrunedPayload::RelationshipDescription(raw, rev) = payload else {
                    return Err(invalid());
                };
                if *raw != id.0
                    || perception
                        .relationship_description_prune(id, *rev)
                        .is_none_or(|p| p.manifest_digest != *digest)
                    || !selected.insert((id, *rev))
                {
                    return Err(invalid());
                }
            }
        }
    }
    if perception
        .relationship_description_prunes()
        .any(|proof| !selected.contains(&(proof.id, proof.revision)))
    {
        return Err(invalid());
    }
    Ok(())
}
