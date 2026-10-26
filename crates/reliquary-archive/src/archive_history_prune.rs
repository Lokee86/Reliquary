//! Archive-owned evidence for explicitly pruned conversation payloads.
//!
//! This module only interprets markers. The facade maintenance operation is
//! responsible for validating the manifest against the source history before
//! publishing a transformed copy.
use crate::{Archive, ArchiveError, ContentId, Node};
use reliquary_container::Container;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const MARKER_MAGIC: &[u8; 8] = b"CVAPRND1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrunedArchiveContent {
    pub id: ContentId,
    pub byte_length: u64,
    pub manifest_digest: [u8; 32],
}

impl PrunedArchiveContent {
    pub fn encode(&self) -> Vec<u8> {
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

    pub fn markers(&self) -> &HashMap<ContentId, PrunedArchiveContent> {
        &self.markers
    }
}

impl Archive {
    pub fn node_content(
        &self,
        container: &mut Container,
        node: &Node,
    ) -> Result<String, ArchiveError> {
        if self.prune.node_is_pruned(node) {
            return Err(ArchiveError::PayloadUnavailable(
                crate::archive_error::ArchivePayloadUnavailable::Pruned,
            ));
        }
        self.content(container, node.content_id)
    }
}

impl Archive {
    /// Read the immutable source markers that describe pruned Archive content.
    pub fn pruned_content_markers(&self) -> &HashMap<ContentId, PrunedArchiveContent> {
        self.prune.markers()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_codec_checks_checksum_without_maintenance_state() {
        let marker = PrunedArchiveContent {
            id: ContentId([5; 32]),
            byte_length: 17,
            manifest_digest: [3; 32],
        };
        let bytes = marker.encode();
        assert_eq!(PrunedArchiveContent::decode(&bytes).unwrap(), marker);

        let mut damaged = bytes;
        damaged[40] ^= 1;
        assert!(PrunedArchiveContent::decode(&damaged).is_err());
    }
}
