//! Non-semantic proof metadata for explicit transformed-copy history pruning.
//! A decoded manifest is evidence, never authorization: domain owners must validate
//! every descriptor against their own tombstones and surviving references.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAGIC: &[u8; 8] = b"CVAHPR01";
const MAX_MANIFEST_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub(crate) enum RetentionDomain {
    Archive,
    Memory,
    Entity,
    Relationship,
    CompatibilityProfile,
}

/// Persisted identity representation, confined to the maintenance format boundary.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetentionTarget {
    pub domain: RetentionDomain,
    pub identity: Vec<u8>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub(crate) enum PrunedPayload {
    ArchiveContent([u8; 32]),
    MemoryBody([u8; 32]),
    EntityDescription([u8; 32], u64),
    RelationshipDescription([u8; 32], u64),
    ProfileProbes([u8; 32]),
    PackedVectors([u8; 32]),
}

/// Domain-produced retention proof. Coordinates refer to existing clocks only.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HistoryRetentionDescriptor {
    pub target: RetentionTarget,
    pub tombstone_global_version: Option<u64>,
    pub tombstone_domain_version: Option<u64>,
    pub tombstone_digest: [u8; 32],
    pub retained_structure_digest: [u8; 32],
    pub payloads: Vec<PrunedPayload>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HistoryPruneManifest {
    pub owner_uuid: [u8; 16],
    pub source_global_version: u64,
    /// Exact pre-transform history fingerprint, excluding physical locations.
    pub source_history_digest: [u8; 32],
    pub descriptors: Vec<HistoryRetentionDescriptor>,
}

impl HistoryPruneManifest {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.descriptors.is_empty() || self.source_history_digest == [0; 32] {
            return Err("prune manifest lacks retention evidence".into());
        }
        let mut previous: Option<&RetentionTarget> = None;
        for descriptor in &self.descriptors {
            if descriptor.target.identity.is_empty()
                || previous.is_some_and(|target| target >= &descriptor.target)
                || descriptor.tombstone_digest == [0; 32]
                || descriptor.retained_structure_digest == [0; 32]
                || descriptor.payloads.is_empty()
                || descriptor.tombstone_global_version == Some(0)
                || descriptor.tombstone_domain_version == Some(0)
                || descriptor
                    .tombstone_global_version
                    .is_some_and(|version| version > self.source_global_version)
                || descriptor
                    .payloads
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
            {
                return Err("invalid or noncanonical prune retention descriptor".into());
            }
            previous = Some(&descriptor.target);
        }
        Ok(())
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let data = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        if data.len() > MAX_MANIFEST_BYTES {
            return Err("prune manifest exceeds maintenance metadata bound".into());
        }
        let mut bytes = Vec::with_capacity(16 + data.len() + 32);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&(data.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&data);
        let checksum = Sha256::digest(&bytes);
        bytes.extend_from_slice(&checksum);
        Ok(bytes)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 48 || bytes.get(..8) != Some(MAGIC.as_slice()) {
            return Err("invalid prune manifest framing".into());
        }
        let length = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let length = usize::try_from(length).map_err(|_| "prune manifest size overflow")?;
        if length > MAX_MANIFEST_BYTES || length.checked_add(48) != Some(bytes.len()) {
            return Err("invalid prune manifest size".into());
        }
        let end = 16 + length;
        if Sha256::digest(&bytes[..end]).as_slice() != &bytes[end..] {
            return Err("prune manifest checksum mismatch".into());
        }
        let manifest: Self =
            serde_json::from_slice(&bytes[16..end]).map_err(|error| error.to_string())?;
        manifest.validate()?;
        // One durable representation avoids ambiguous manifest identities.
        if manifest.encode()?.as_slice() != bytes {
            return Err("noncanonical prune manifest encoding".into());
        }
        Ok(manifest)
    }

    pub(crate) fn digest(&self) -> Result<[u8; 32], String> {
        Ok(Sha256::digest(self.encode()?).into())
    }

    pub(crate) fn authorizes(&self, target: &RetentionTarget, payload: &PrunedPayload) -> bool {
        self.descriptors
            .binary_search_by(|descriptor| descriptor.target.cmp(target))
            .ok()
            .is_some_and(|index| {
                self.descriptors[index]
                    .payloads
                    .binary_search(payload)
                    .is_ok()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> HistoryPruneManifest {
        HistoryPruneManifest {
            owner_uuid: [1; 16],
            source_global_version: 19,
            source_history_digest: [2; 32],
            descriptors: vec![HistoryRetentionDescriptor {
                target: RetentionTarget {
                    domain: RetentionDomain::Archive,
                    identity: b"conversation".to_vec(),
                },
                tombstone_global_version: Some(19),
                tombstone_domain_version: Some(8),
                tombstone_digest: [3; 32],
                retained_structure_digest: [4; 32],
                payloads: vec![PrunedPayload::ArchiveContent([5; 32])],
            }],
        }
    }

    #[test]
    fn manifest_roundtrip_preserves_exact_target_and_payload_evidence() {
        let manifest = fixture();
        let bytes = manifest.encode().unwrap();
        let reopened = HistoryPruneManifest::decode(&bytes).unwrap();
        assert_eq!(reopened, manifest);
        assert_eq!(reopened.digest().unwrap(), manifest.digest().unwrap());
        assert!(reopened.authorizes(
            &manifest.descriptors[0].target,
            &PrunedPayload::ArchiveContent([5; 32])
        ));
        assert!(!reopened.authorizes(
            &manifest.descriptors[0].target,
            &PrunedPayload::ArchiveContent([6; 32])
        ));
    }

    #[test]
    fn missing_corrupt_and_trailing_metadata_is_not_prune_authorization() {
        let bytes = fixture().encode().unwrap();
        for end in 0..bytes.len() {
            assert!(HistoryPruneManifest::decode(&bytes[..end]).is_err());
        }
        for index in 0..bytes.len() {
            let mut damaged = bytes.clone();
            damaged[index] ^= 1;
            assert!(HistoryPruneManifest::decode(&damaged).is_err());
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(HistoryPruneManifest::decode(&trailing).is_err());
    }

    #[test]
    fn descriptors_require_unique_targets_ordered_payloads_and_accepted_tombstones() {
        let mut manifest = fixture();
        manifest.descriptors.push(manifest.descriptors[0].clone());
        assert!(manifest.encode().is_err());
        let mut manifest = fixture();
        manifest.descriptors[0]
            .payloads
            .push(PrunedPayload::ArchiveContent([5; 32]));
        assert!(manifest.encode().is_err());
        let mut manifest = fixture();
        manifest.descriptors[0].tombstone_global_version = Some(20);
        assert!(manifest.encode().is_err());
    }
}
