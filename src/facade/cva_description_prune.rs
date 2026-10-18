//! Retention proofs for terminal Entity and Relationship descriptions.
use crate::history_prune_manifest::*;
use crate::owner_lifecycle::LifecycleTarget;
use crate::{Cva, CvaError};
use sha2::{Digest, Sha256};
impl Cva {
    pub(crate) fn description_retention(
        &self,
        target: &LifecycleTarget,
    ) -> Result<HistoryRetentionDescriptor, CvaError> {
        let (identity, domain, global, local, tombstone, structure, mut payloads) = match target {
            LifecycleTarget::Entity(id) => {
                let retained = self.entities.retention_descriptor(*id)?;
                (
                    id.0,
                    RetentionDomain::Entity,
                    retained.tombstone_global_version,
                    retained.tombstone_entity_version,
                    retained.tombstone_revision,
                    self.entity_description_structure(*id, retained.replacement_id)?,
                    retained
                        .revisions
                        .iter()
                        .map(|r| PrunedPayload::EntityDescription(id.0, r.revision))
                        .collect::<Vec<_>>(),
                )
            }
            LifecycleTarget::Relationship(id) => {
                let retained = self.relationships.retention_descriptor(*id)?;
                (
                    id.0,
                    RetentionDomain::Relationship,
                    retained.tombstone_global_version,
                    retained.tombstone_relationship_version,
                    retained.tombstone_revision,
                    self.relationship_description_structure(*id)?,
                    retained
                        .revisions
                        .iter()
                        .map(|r| PrunedPayload::RelationshipDescription(id.0, r.revision))
                        .collect::<Vec<_>>(),
                )
            }
            _ => {
                return Err(CvaError::Repack(
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
        &self,
        id: crate::EntityId,
        replacement: Option<crate::EntityId>,
    ) -> Result<Vec<u8>, CvaError> {
        let mut bytes = b"Entity retained structure v1".to_vec();
        bytes.push(u8::from(replacement.is_some()));
        if let Some(replacement) = replacement {
            bytes.extend(replacement.0);
        }
        for original in self
            .entities
            .records()
            .iter()
            .filter(|record| record.id == id)
        {
            let mut record = original.clone();
            record.canonical_name = "[pruned]".into();
            record.aliases.clear();
            record.summary.clear();
            let raw = crate::entity_codec::encode_record(&record)?;
            bytes.extend((raw.len() as u64).to_le_bytes());
            bytes.extend(raw);
            bytes.extend(record.global_version.to_le_bytes());
            bytes.extend(record.entity_version.to_le_bytes());
        }
        Ok(bytes)
    }
    fn relationship_description_structure(
        &self,
        id: crate::RelationshipId,
    ) -> Result<Vec<u8>, CvaError> {
        let mut bytes = b"Relationship retained structure v1".to_vec();
        for original in self
            .relationships
            .records()
            .iter()
            .filter(|record| record.id == id)
        {
            let mut record = original.clone();
            record.summary.clear();
            let raw = crate::relationship_codec::encode_record(&record)?;
            bytes.extend((raw.len() as u64).to_le_bytes());
            bytes.extend(raw);
            bytes.extend(record.global_version.to_le_bytes());
            bytes.extend(record.relationship_version.to_le_bytes());
        }
        Ok(bytes)
    }
    pub(crate) fn validate_pruned_descriptions(&mut self) -> Result<(), CvaError> {
        let invalid =
            || CvaError::Repack("pruned description lacks exact terminal owner evidence".into());
        let mut authorized = std::collections::BTreeSet::new();
        for (digest, manifest) in self.archive.prune.manifests() {
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
                if self.description_retention(&target)? != *descriptor {
                    return Err(invalid());
                }
                for payload in &descriptor.payloads {
                    authorized.insert((payload.clone(), *digest));
                }
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for object in self.container.chunks()? {
            let bytes = self.container.read(object)?;
            let payload = if bytes.starts_with(b"CVAEPRN1") {
                let record = crate::entity_codec::decode_record(&bytes)?.ok_or_else(invalid)?;
                PrunedPayload::EntityDescription(record.id.0, record.revision)
            } else if bytes.starts_with(b"CVARPRN1") {
                let record =
                    crate::relationship_codec::decode_record(&bytes)?.ok_or_else(invalid)?;
                PrunedPayload::RelationshipDescription(record.id.0, record.revision)
            } else {
                continue;
            };
            let digest: [u8; 32] = bytes[40..72].try_into().unwrap();
            if !authorized.contains(&(payload.clone(), digest)) || !seen.insert((payload, digest)) {
                return Err(invalid());
            }
        }
        if seen != authorized {
            return Err(invalid());
        }
        Ok(())
    }
}
