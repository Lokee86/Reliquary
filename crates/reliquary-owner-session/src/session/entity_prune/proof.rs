//! Exhaustive retained Entity revision proof, source and reopened candidate.
use super::*;
use std::collections::BTreeSet;
pub(super) fn descriptor(
    perception: &PerceptionStore,
    id: EntityId,
    redacted: bool,
) -> Result<HistoryRetentionDescriptor, String> {
    let tombstone = perception
        .entity_tombstone(id)
        .ok_or("missing terminal Entity tombstone")?;
    let mut structure = Sha256::new();
    structure.update(b"reliquary-entity-retained-structure-v2\0");
    structure.update(id.0);
    let mut payloads = Vec::new();
    for revision in 1..=tombstone.revision {
        let entity = perception
            .entity_revision(id, revision)
            .ok_or("incomplete Entity revision chain")?;
        let digest = if redacted {
            if !entity.summary.is_empty() {
                return Err("redacted Entity still retains description".into());
            }
            perception
                .description_prune(id, revision)
                .ok_or("missing Entity revision prune marker")?
                .summary_digest
        } else {
            if entity.summary.is_empty() || perception.description_prune(id, revision).is_some() {
                return Err("Entity source revision has no removable description".into());
            }
            Sha256::digest(entity.summary.as_bytes()).into()
        };
        structure.update(revision.to_le_bytes());
        hash_text(&mut structure, &entity.canonical_name);
        structure.update((entity.aliases.len() as u64).to_le_bytes());
        for alias in &entity.aliases {
            hash_text(&mut structure, alias)
        }
        hash_text(&mut structure, &entity.kind);
        hash_text(&mut structure, &entity.mutation_id);
        structure.update(entity.created_at_ns.to_le_bytes());
        structure.update(entity.updated_at_ns.to_le_bytes());
        structure.update(digest);
        payloads.push(PrunedPayload::EntityDescription(id.0, revision));
    }
    let mut tombstone_hash = Sha256::new();
    tombstone_hash.update(b"reliquary-entity-tombstone-v1\0");
    tombstone_hash.update(id.0);
    tombstone_hash.update(tombstone.revision.to_le_bytes());
    tombstone_hash.update(tombstone.perception_version.to_le_bytes());
    Ok(HistoryRetentionDescriptor {
        target: RetentionTarget {
            domain: RetentionDomain::Entity,
            identity: id.0.to_vec(),
        },
        tombstone_global_version: None,
        tombstone_domain_version: Some(tombstone.perception_version),
        tombstone_digest: tombstone_hash.finalize().into(),
        retained_structure_digest: structure.finalize().into(),
        payloads,
    })
}
pub(crate) fn validate_entity_prune(
    perception: &PerceptionStore,
    maintenance: &OwnerMaintenanceStore,
) -> Result<(), OwnerSessionError> {
    let invalid = || OwnerSessionError::Record("Entity prune retention evidence mismatch".into());
    let mut authorized = BTreeSet::new();
    for (manifest_digest, manifest) in maintenance.manifests() {
        for d in &manifest.descriptors {
            if d.target.domain != RetentionDomain::Entity {
                continue;
            }
            let raw: [u8; 32] = d
                .target
                .identity
                .as_slice()
                .try_into()
                .map_err(|_| invalid())?;
            let id = EntityId(raw);
            if descriptor(perception, id, true).map_err(|_| invalid())? != *d {
                return Err(invalid());
            }
            for payload in &d.payloads {
                let PrunedPayload::EntityDescription(target, revision) = payload else {
                    return Err(invalid());
                };
                if *target != id.0
                    || perception
                        .description_prune(id, *revision)
                        .is_none_or(|p| p.manifest_digest != *manifest_digest)
                    || !authorized.insert((id, *revision))
                {
                    return Err(invalid());
                }
            }
        }
    }
    for marker in perception.description_prunes() {
        if !authorized.contains(&(marker.id, marker.revision)) {
            return Err(invalid());
        }
    }
    Ok(())
}
