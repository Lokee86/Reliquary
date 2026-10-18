//! Archive-owned evidence for explicitly pruned conversation payloads.
//!
//! This module only interprets markers. The facade maintenance operation is
//! responsible for validating the manifest against the source history before
//! publishing a transformed copy.
use crate::archive_codec::{ArchiveRecord, decode_record, encode_conversation_tombstone};
use crate::history_prune_manifest::{
    HistoryPruneManifest, HistoryRetentionDescriptor, PrunedPayload, RetentionDomain,
    RetentionTarget,
};
use crate::{Archive, ArchiveError, Container, ContentId, Node};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

const MARKER_MAGIC: &[u8; 8] = b"CVAPRND1";
const MANIFEST_MAGIC: &[u8; 8] = b"CVAHPR01";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrunedArchiveContent {
    pub(crate) id: ContentId,
    pub(crate) byte_length: u64,
    pub(crate) manifest_digest: [u8; 32],
}

impl PrunedArchiveContent {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(112);
        bytes.extend_from_slice(MARKER_MAGIC);
        bytes.extend_from_slice(&self.id.0);
        bytes.extend_from_slice(&self.byte_length.to_le_bytes());
        bytes.extend_from_slice(&self.manifest_digest);
        let checksum = Sha256::digest(&bytes);
        bytes.extend_from_slice(&checksum);
        bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self, ArchiveError> {
        if bytes.len() != 112 || Sha256::digest(&bytes[..80]).as_slice() != &bytes[80..] {
            return Err(ArchiveError::CorruptRecord("invalid pruned content marker"));
        }
        Ok(Self {
            id: ContentId(bytes[8..40].try_into().unwrap()),
            byte_length: u64::from_le_bytes(bytes[40..48].try_into().unwrap()),
            manifest_digest: bytes[48..80].try_into().unwrap(),
        })
    }
}

#[derive(Default)]
pub(crate) struct ArchivePruneState {
    markers: HashMap<ContentId, PrunedArchiveContent>,
    manifests: BTreeMap<[u8; 32], HistoryPruneManifest>,
}

impl ArchivePruneState {
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<bool, ArchiveError> {
        if payload.starts_with(MARKER_MAGIC) {
            let marker = PrunedArchiveContent::decode(payload)?;
            if self.markers.insert(marker.id, marker).is_some() {
                return Err(ArchiveError::CorruptRecord(
                    "duplicate pruned content marker",
                ));
            }
            return Ok(true);
        }
        if payload.starts_with(MANIFEST_MAGIC) {
            let manifest = HistoryPruneManifest::decode(payload)
                .map_err(|_| ArchiveError::CorruptRecord("invalid prune manifest"))?;
            let digest = manifest
                .digest()
                .map_err(|_| ArchiveError::CorruptRecord("invalid prune manifest digest"))?;
            if self.manifests.insert(digest, manifest).is_some() {
                return Err(ArchiveError::CorruptRecord("duplicate prune manifest"));
            }
            return Ok(true);
        }
        Ok(false)
    }

    pub(crate) fn node_is_pruned(&self, node: &Node) -> bool {
        let Some(marker) = self.markers.get(&node.content_id) else {
            return false;
        };
        self.manifests
            .get(&marker.manifest_digest)
            .is_some_and(|manifest| {
                manifest.authorizes(
                    &RetentionTarget {
                        domain: RetentionDomain::Archive,
                        identity: node.conversation_id.as_bytes().to_vec(),
                    },
                    &PrunedPayload::ArchiveContent(node.content_id.0),
                )
            })
    }

    pub(crate) fn validate_markers(&self) -> Result<(), ArchiveError> {
        for marker in self.markers.values() {
            if !self
                .manifests
                .get(&marker.manifest_digest)
                .is_some_and(|manifest| {
                    manifest.descriptors.iter().any(|descriptor| {
                        descriptor.target.domain == RetentionDomain::Archive
                            && descriptor
                                .payloads
                                .contains(&PrunedPayload::ArchiveContent(marker.id.0))
                    })
                })
            {
                return Err(ArchiveError::CorruptRecord("unbound pruned content marker"));
            }
        }
        Ok(())
    }

    pub(crate) fn manifest(&self, digest: &[u8; 32]) -> Option<&HistoryPruneManifest> {
        self.manifests.get(digest)
    }

    pub(crate) fn manifests(&self) -> &BTreeMap<[u8; 32], HistoryPruneManifest> {
        &self.manifests
    }
}

impl Archive {
    pub(crate) fn retention_descriptor(
        &self,
        container: &mut Container,
        conversation_id: &str,
        watermark: u64,
        lengths: &HashMap<ContentId, u64>,
    ) -> Result<HistoryRetentionDescriptor, ArchiveError> {
        let tombstone = self
            .conversation_tombstone(conversation_id)
            .filter(|value| value.global_version <= watermark)
            .ok_or(ArchiveError::MissingConversation)?;
        let mut structure = Sha256::new();
        let mut payloads = std::collections::BTreeSet::new();
        for version in self
            .record_versions
            .iter()
            .filter(|version| version.global_version <= watermark)
        {
            let raw = container.read(version.record)?;
            let record = decode_record(&raw)?;
            let relevant = match &record {
                ArchiveRecord::Node(node) => node.conversation_id == conversation_id,
                ArchiveRecord::IngestedTurn(turn) => turn.node.conversation_id == conversation_id,
                ArchiveRecord::Branch(branch) => branch.conversation_id == conversation_id,
                ArchiveRecord::ConversationMetadata(metadata) => {
                    metadata.conversation_id == conversation_id
                }
                ArchiveRecord::ConversationTombstone(value) => {
                    value.conversation_id == conversation_id
                }
                ArchiveRecord::Fragment(fragment) => fragment.conversation_id == conversation_id,
                ArchiveRecord::Episode(episode) => episode.conversation_id == conversation_id,
                ArchiveRecord::File(file) => tombstone.closure.file_ids.contains(&file.id),
                ArchiveRecord::FileMemoryLink(link) => {
                    tombstone.closure.file_ids.contains(&link.file_id)
                }
                _ => false,
            };
            if !relevant {
                continue;
            }
            structure.update(version.global_version.to_le_bytes());
            structure.update(version.archive_version.to_le_bytes());
            hash_bytes(&mut structure, &raw);
            let node = match &record {
                ArchiveRecord::Node(node) => Some(node),
                ArchiveRecord::IngestedTurn(turn) => Some(&turn.node),
                _ => None,
            };
            if let Some(node) = node {
                if !lengths.contains_key(&node.content_id) {
                    return Err(ArchiveError::MissingContent);
                }
                payloads.insert(PrunedPayload::ArchiveContent(node.content_id.0));
            }
        }
        Ok(HistoryRetentionDescriptor {
            target: RetentionTarget {
                domain: RetentionDomain::Archive,
                identity: conversation_id.as_bytes().to_vec(),
            },
            tombstone_global_version: Some(tombstone.global_version),
            tombstone_domain_version: Some(tombstone.archive_version),
            tombstone_digest: Sha256::digest(encode_conversation_tombstone(tombstone)?).into(),
            retained_structure_digest: structure.finalize().into(),
            payloads: payloads.into_iter().collect(),
        })
    }

    pub(crate) fn prune_history_digest(
        &self,
        container: &mut Container,
        watermark: u64,
    ) -> Result<[u8; 32], ArchiveError> {
        crate::history_prune_manifest::owner_history_digest(container, watermark)
            .map_err(|_| ArchiveError::CorruptRecord("invalid source history fingerprint"))
    }

    pub(crate) fn validate_prune_history(
        &self,
        container: &mut Container,
    ) -> Result<(), ArchiveError> {
        let invalid =
            || ArchiveError::CorruptRecord("prune evidence does not match source history");
        let mut all_authorized = std::collections::HashSet::<ContentId>::new();
        for (digest, manifest) in &self.prune.manifests {
            if Some(manifest.owner_uuid) != container.owner_uuid()
                || manifest.source_global_version > container.latest_version()
                || self.prune_history_digest(container, manifest.source_global_version)?
                    != manifest.source_history_digest
            {
                return Err(invalid());
            }
            let mut lengths = HashMap::new();
            let mut selected = std::collections::BTreeSet::new();
            for descriptor in &manifest.descriptors {
                if descriptor.target.domain != RetentionDomain::Archive {
                    continue;
                }
                let conversation =
                    std::str::from_utf8(&descriptor.target.identity).map_err(|_| invalid())?;
                if !selected.insert(conversation) {
                    return Err(invalid());
                }
                for payload in &descriptor.payloads {
                    let PrunedPayload::ArchiveContent(raw_id) = payload else {
                        return Err(invalid());
                    };
                    let id = ContentId(*raw_id);
                    let marker = self.prune.markers.get(&id).ok_or_else(invalid)?;
                    if marker.manifest_digest != *digest
                        || lengths
                            .insert(id, marker.byte_length)
                            .is_some_and(|old| old != marker.byte_length)
                        || !all_authorized.insert(id)
                    {
                        return Err(invalid());
                    }
                }
            }
            for descriptor in &manifest.descriptors {
                if descriptor.target.domain != RetentionDomain::Archive {
                    continue;
                }
                let conversation =
                    std::str::from_utf8(&descriptor.target.identity).map_err(|_| invalid())?;
                if self.retention_descriptor(
                    container,
                    conversation,
                    manifest.source_global_version,
                    &lengths,
                )? != *descriptor
                {
                    return Err(invalid());
                }
            }
            for version in self
                .record_versions
                .iter()
                .filter(|version| version.global_version <= manifest.source_global_version)
            {
                let record = decode_record(&container.read(version.record)?)?;
                let protected = match &record {
                    ArchiveRecord::Node(node) => {
                        lengths.contains_key(&node.content_id)
                            && !selected.contains(node.conversation_id.as_str())
                    }
                    ArchiveRecord::IngestedTurn(turn) => {
                        (lengths.contains_key(&turn.node.content_id)
                            && !selected.contains(turn.node.conversation_id.as_str()))
                            || turn
                                .attachments
                                .iter()
                                .any(|file| lengths.contains_key(&file.content_id))
                    }
                    ArchiveRecord::File(file) => lengths.contains_key(&file.content_id),
                    _ => false,
                };
                if protected {
                    return Err(invalid());
                }
            }
        }
        if all_authorized.len() != self.prune.markers.len() {
            return Err(invalid());
        }
        Ok(())
    }

    pub(crate) fn node_content(
        &self,
        container: &mut Container,
        node: &Node,
    ) -> Result<String, ArchiveError> {
        if self.prune.node_is_pruned(node) {
            return Err(ArchiveError::PayloadUnavailable(
                crate::owner_lifecycle::PayloadUnavailable::Pruned,
            ));
        }
        self.content(container, node.content_id)
    }
}

fn hash_bytes(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_prune_manifest::HistoryRetentionDescriptor;

    #[test]
    fn content_marker_is_checksummed_and_requires_manifest_binding() {
        let manifest = HistoryPruneManifest {
            owner_uuid: [1; 16],
            source_global_version: 2,
            source_history_digest: [2; 32],
            descriptors: vec![HistoryRetentionDescriptor {
                target: RetentionTarget {
                    domain: RetentionDomain::Archive,
                    identity: b"retired-conversation".to_vec(),
                },
                tombstone_global_version: Some(2),
                tombstone_domain_version: Some(1),
                tombstone_digest: [3; 32],
                retained_structure_digest: [4; 32],
                payloads: vec![PrunedPayload::ArchiveContent([5; 32])],
            }],
        };
        let digest = manifest.digest().unwrap();
        let marker = PrunedArchiveContent {
            id: ContentId([5; 32]),
            byte_length: 17,
            manifest_digest: digest,
        };
        let bytes = marker.encode();
        assert_eq!(PrunedArchiveContent::decode(&bytes).unwrap(), marker);

        let mut damaged = bytes;
        damaged[40] ^= 1;
        assert!(PrunedArchiveContent::decode(&damaged).is_err());

        let mut state = ArchivePruneState::default();
        assert!(state.ingest(&marker.encode()).unwrap());
        assert!(state.validate_markers().is_err());
        assert!(state.ingest(&manifest.encode().unwrap()).unwrap());
        assert!(state.validate_markers().is_ok());
    }
}
