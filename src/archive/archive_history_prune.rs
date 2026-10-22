//! Archive-owned evidence for explicitly pruned conversation payloads.
//!
//! This module only interprets markers. The facade maintenance operation is
//! responsible for validating the manifest against the source history before
//! publishing a transformed copy.
use crate::archive_codec::{ArchiveRecord, encode_conversation_tombstone};
use crate::{Archive, ArchiveError, Container, ContentId, Node};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const MARKER_MAGIC: &[u8; 8] = b"CVAPRND1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ArchiveRetentionProof {
    pub(crate) conversation_id: String,
    pub(crate) source_global_version: u64,
    pub(crate) tombstone_global_version: u64,
    pub(crate) tombstone_archive_version: u64,
    pub(crate) tombstone_digest: [u8; 32],
    pub(crate) retained_structure_digest: [u8; 32],
    pub(crate) payloads: Vec<(ContentId, u64)>,
}

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
        Ok(false)
    }

    pub(crate) fn node_is_pruned(&self, node: &Node) -> bool {
        self.markers.contains_key(&node.content_id)
    }

    pub(crate) fn markers(&self) -> &HashMap<ContentId, PrunedArchiveContent> {
        &self.markers
    }
}

impl Archive {
    pub(crate) fn pruned_content_markers(&self) -> impl Iterator<Item = &PrunedArchiveContent> {
        self.prune.markers.values()
    }

    pub(crate) fn retention_proof(
        &self,
        container: &mut Container,
        conversation_id: &str,
        watermark: u64,
        lengths: &HashMap<ContentId, u64>,
    ) -> Result<ArchiveRetentionProof, ArchiveError> {
        let tombstone = self
            .conversation_tombstone(conversation_id)
            .filter(|value| value.global_version <= watermark)
            .ok_or(ArchiveError::MissingConversation)?;
        let mut structure = Sha256::new();
        let mut payloads = std::collections::BTreeSet::<[u8; 32]>::new();
        for version in self
            .record_versions
            .iter()
            .filter(|version| version.global_version <= watermark)
        {
            let (record, _) = self.read_publication(container, *version)?;
            let stored = container.read(version.record)?;
            let raw = if crate::container::publication_envelope::decode(&stored)?.is_some() {
                crate::archive_codec::publication::comparison_carrier(&record)?
            } else {
                stored
            };
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
                payloads.insert(node.content_id.0);
            }
        }
        Ok(ArchiveRetentionProof {
            conversation_id: conversation_id.to_owned(),
            source_global_version: watermark,
            tombstone_global_version: tombstone.global_version,
            tombstone_archive_version: tombstone.archive_version,
            tombstone_digest: Sha256::digest(encode_conversation_tombstone(tombstone)?).into(),
            retained_structure_digest: structure.finalize().into(),
            payloads: payloads
                .into_iter()
                .map(|raw_id| {
                    let id = ContentId(raw_id);
                    lengths
                        .get(&id)
                        .copied()
                        .map(|length| (id, length))
                        .ok_or(ArchiveError::MissingContent)
                })
                .collect::<Result<_, _>>()?,
        })
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

    #[test]
    fn content_marker_is_checksummed_and_requires_manifest_binding() {
        let marker = PrunedArchiveContent {
            id: ContentId([5; 32]),
            byte_length: 17,
            manifest_digest: [6; 32],
        };
        let bytes = marker.encode();
        assert_eq!(PrunedArchiveContent::decode(&bytes).unwrap(), marker);

        let mut damaged = bytes;
        damaged[40] ^= 1;
        assert!(PrunedArchiveContent::decode(&damaged).is_err());

        let mut state = ArchivePruneState::default();
        assert!(state.ingest(&marker.encode()).unwrap());
        assert_eq!(state.markers().get(&marker.id), Some(&marker));
    }
}
