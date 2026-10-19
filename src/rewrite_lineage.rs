//! Stable history-generation identity used by rewrite lineage and Freshness baselines.

use sha2::{Digest, Sha256};
use std::fmt;

pub(crate) const REWRITE_GENESIS_MAGIC: &[u8; 8] = b"CVARGN01";

/// A history generation is distinct from owner identity: preserving an owner
/// across a fresh rewrite still creates a new generation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HistoryGenerationId(pub [u8; 16]);

impl HistoryGenerationId {
    /// Generation zero is the implicit generation for pre-lineage files.
    pub const ZERO: Self = Self([0; 16]);

    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl Default for HistoryGenerationId {
    fn default() -> Self {
        Self::ZERO
    }
}

impl fmt::Display for HistoryGenerationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// First durable lineage record for an intentionally rewritten owner history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewriteGenesis {
    pub destination_owner: [u8; 16],
    pub generation: HistoryGenerationId,
    pub predecessor_owner: [u8; 16],
    pub predecessor_generation: HistoryGenerationId,
    pub source_head_digest: [u8; 32],
    pub policy_digest: [u8; 32],
    pub created_at_unix_millis: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RewriteLineageStore {
    genesis: Option<RewriteGenesis>,
}

impl RewriteLineageStore {
    pub(crate) fn ingest(&mut self, payload: &[u8], owner: [u8; 16]) -> Result<bool, String> {
        if !payload.starts_with(REWRITE_GENESIS_MAGIC) {
            return Ok(false);
        }
        let genesis = RewriteGenesis::decode(payload)?;
        if genesis.destination_owner != owner {
            return Err("RewriteGenesis destination owner does not match the file owner".into());
        }
        if genesis.predecessor_owner == genesis.destination_owner
            && genesis.generation == genesis.predecessor_generation
        {
            return Err("PreserveOwner rewrite must advance the history generation".into());
        }
        if self.genesis.is_some() {
            return Err("multiple RewriteGenesis records in one history".into());
        }
        self.genesis = Some(genesis);
        Ok(true)
    }

    pub(crate) fn generation(&self) -> HistoryGenerationId {
        self.genesis
            .as_ref()
            .map_or(HistoryGenerationId::ZERO, |value| value.generation)
    }

    pub(crate) fn rewrite_genesis(&self) -> Option<&RewriteGenesis> {
        self.genesis.as_ref()
    }
}

impl RewriteGenesis {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut out = REWRITE_GENESIS_MAGIC.to_vec();
        out.extend_from_slice(&self.destination_owner);
        out.extend_from_slice(self.generation.as_bytes());
        out.extend_from_slice(&self.predecessor_owner);
        out.extend_from_slice(self.predecessor_generation.as_bytes());
        out.extend_from_slice(&self.source_head_digest);
        out.extend_from_slice(&self.policy_digest);
        out.extend_from_slice(&self.created_at_unix_millis.to_le_bytes());
        let checksum = Sha256::digest(&out);
        out.extend_from_slice(&checksum);
        out
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        const LEN: usize = 8 + 16 + 16 + 16 + 16 + 32 + 32 + 8 + 32;
        if bytes.len() != LEN || bytes.get(..8) != Some(REWRITE_GENESIS_MAGIC.as_slice()) {
            return Err("invalid RewriteGenesis frame length or magic".into());
        }
        let content_len = LEN - 32;
        if Sha256::digest(&bytes[..content_len]).as_slice() != &bytes[content_len..] {
            return Err("RewriteGenesis checksum mismatch".into());
        }
        let mut cursor = 8;
        fn take<const N: usize>(bytes: &[u8], cursor: &mut usize) -> [u8; N] {
            let value = bytes[*cursor..*cursor + N]
                .try_into()
                .expect("fixed frame length");
            *cursor += N;
            value
        }
        let destination_owner = take(bytes, &mut cursor);
        let generation = HistoryGenerationId::from_bytes(take(bytes, &mut cursor));
        let predecessor_owner = take(bytes, &mut cursor);
        let predecessor_generation = HistoryGenerationId::from_bytes(take(bytes, &mut cursor));
        let source_head_digest = take(bytes, &mut cursor);
        let policy_digest = take(bytes, &mut cursor);
        let created_at_unix_millis = u64::from_le_bytes(take(bytes, &mut cursor));
        if generation == HistoryGenerationId::ZERO {
            return Err("RewriteGenesis cannot create generation zero".into());
        }
        Ok(Self {
            destination_owner,
            generation,
            predecessor_owner,
            predecessor_generation,
            source_head_digest,
            policy_digest,
            created_at_unix_millis,
        })
    }
}
