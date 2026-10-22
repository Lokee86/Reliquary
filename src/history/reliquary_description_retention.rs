//! Read-only REL terminal-description retention proofs.

use crate::entity_store::EntityStore;
use crate::owner_lifecycle::{HistoryReadFailure, HistorySource, LifecycleTarget, OwnerError};
use crate::owner_maintenance::{
    HistoryRetentionDescriptor, OwnerMaintenanceStore, PrunedPayload, RetentionDomain,
    RetentionTarget,
};
use crate::relationship_store::RelationshipStore;
use sha2::{Digest, Sha256};

pub(crate) fn description_retention(
    entities: &EntityStore,
    relationships: &RelationshipStore,
    target: &LifecycleTarget,
) -> Result<HistoryRetentionDescriptor, OwnerError> {
    let (identity, domain, global, local, tombstone, structure, mut payloads) = match target {
        LifecycleTarget::Entity(id) => {
            let retained = entities.retention_descriptor(*id)?;
            (
                id.0,
                RetentionDomain::Entity,
                retained.tombstone_global_version,
                retained.tombstone_entity_version,
                retained.tombstone_revision,
                entity_description_structure(entities, *id, retained.replacement_id)?,
                retained
                    .revisions
                    .iter()
                    .map(|revision| PrunedPayload::EntityDescription(id.0, revision.revision))
                    .collect::<Vec<_>>(),
            )
        }
        LifecycleTarget::Relationship(id) => {
            let retained = relationships.retention_descriptor(*id)?;
            (
                id.0,
                RetentionDomain::Relationship,
                retained.tombstone_global_version,
                retained.tombstone_relationship_version,
                retained.tombstone_revision,
                relationship_description_structure(relationships, *id)?,
                retained
                    .revisions
                    .iter()
                    .map(|revision| PrunedPayload::RelationshipDescription(id.0, revision.revision))
                    .collect::<Vec<_>>(),
            )
        }
        _ => {
            return Err(OwnerError::Repack(
                "invalid description retention target".into(),
            ));
        }
    };
    payloads.sort();
    Ok(HistoryRetentionDescriptor {
        target: RetentionTarget {
            domain,
            identity: identity.to_vec(),
        },
        tombstone_global_version: Some(global),
        tombstone_domain_version: Some(local),
        tombstone_digest: Sha256::digest(
            [
                identity.as_slice(),
                &tombstone.to_le_bytes(),
                &global.to_le_bytes(),
                &local.to_le_bytes(),
            ]
            .concat(),
        )
        .into(),
        retained_structure_digest: Sha256::digest(&structure).into(),
        payloads,
    })
}

fn entity_description_structure(
    entities: &EntityStore,
    id: crate::EntityId,
    replacement: Option<crate::EntityId>,
) -> Result<Vec<u8>, OwnerError> {
    let mut bytes = b"Entity retained structure v1".to_vec();
    bytes.push(u8::from(replacement.is_some()));
    if let Some(replacement) = replacement {
        bytes.extend(replacement.0);
    }
    for original in entities.records().iter().filter(|record| record.id == id) {
        let raw = crate::entity_retained_structure::encode_record_structure(original)?;
        bytes.extend((raw.len() as u64).to_le_bytes());
        bytes.extend(raw);
        bytes.extend(original.global_version.to_le_bytes());
        bytes.extend(original.entity_version.to_le_bytes());
    }
    Ok(bytes)
}

fn relationship_description_structure(
    relationships: &RelationshipStore,
    id: crate::RelationshipId,
) -> Result<Vec<u8>, OwnerError> {
    let mut bytes = b"Relationship retained structure v1".to_vec();
    for record in relationships
        .records()
        .iter()
        .filter(|record| record.id == id)
    {
        let raw = crate::relationship_retained_structure::encode_record_structure(record)?;
        bytes.extend((raw.len() as u64).to_le_bytes());
        bytes.extend(raw);
        bytes.extend(record.global_version.to_le_bytes());
        bytes.extend(record.relationship_version.to_le_bytes());
    }
    Ok(bytes)
}

pub(crate) fn validate_description_retention(
    source: Result<HistorySource<'_>, HistoryReadFailure>,
    entities: &EntityStore,
    relationships: &RelationshipStore,
    maintenance: &OwnerMaintenanceStore,
) -> Result<(), OwnerError> {
    let invalid =
        || OwnerError::Repack("pruned description lacks exact terminal owner evidence".into());
    let mut authorized = std::collections::BTreeSet::new();
    for (digest, manifest) in maintenance.manifests() {
        for descriptor in &manifest.descriptors {
            let id: [u8; 32] = match descriptor.target.domain {
                RetentionDomain::Entity | RetentionDomain::Relationship => descriptor
                    .target
                    .identity
                    .as_slice()
                    .try_into()
                    .map_err(|_| invalid())?,
                _ => continue,
            };
            let target = match descriptor.target.domain {
                RetentionDomain::Entity => LifecycleTarget::Entity(crate::EntityId(id)),
                _ => LifecycleTarget::Relationship(crate::RelationshipId(id)),
            };
            if description_retention(entities, relationships, &target)? != *descriptor {
                return Err(invalid());
            }
            for payload in &descriptor.payloads {
                authorized.insert((payload.clone(), *digest));
            }
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    let source = source.map_err(|error| OwnerError::Container(error.0))?;
    let mut cursor = source.cursor().map_err(history_read_error)?;
    while let Some(frame) = cursor.next_frame().map_err(history_read_error)? {
        let bytes = frame.payload;
        let compact = crate::entity_description_proof::compact_description_proof(&bytes)
            .map_err(OwnerError::Entities)?;
        let relationship_compact =
            crate::relationship_description_proof::compact_description_proof(&bytes)
                .map_err(OwnerError::Relationships)?;
        let payload = if let Some((id, revision, _)) = &relationship_compact {
            if !relationships.pruned_revisions.contains(&(*id, *revision)) {
                return Err(invalid());
            }
            PrunedPayload::RelationshipDescription(id.0, *revision)
        } else if let Some((id, revision, _)) = &compact {
            if !entities.pruned_revisions.contains(&(*id, *revision)) {
                return Err(invalid());
            }
            PrunedPayload::EntityDescription(id.0, *revision)
        } else {
            continue;
        };
        let digest: [u8; 32] = match compact
            .map(|(_, _, proof)| proof.manifest_digest)
            .or_else(|| relationship_compact.map(|(_, _, proof)| proof.manifest_digest))
        {
            Some(digest) => digest,
            None => return Err(invalid()),
        };
        if !authorized.contains(&(payload.clone(), digest)) || !seen.insert((payload, digest)) {
            return Err(invalid());
        }
    }
    if seen != authorized {
        return Err(invalid());
    }
    Ok(())
}

fn history_read_error(error: HistoryReadFailure) -> OwnerError {
    OwnerError::Container(error.0)
}
