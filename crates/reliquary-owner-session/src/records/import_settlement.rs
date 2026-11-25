//! Epoch-scoped, durable snapshot of accepted historical Freshness state.
//! The owner coordinator derives these fields; callers cannot supply a
//! settlement to complete an import.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;

pub const MAGIC: &[u8; 8] = b"RELIFRS1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportFreshnessAuthority {
    pub state_digest: String,
    pub memory_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportFreshnessSettlement {
    pub owner_uuid: [u8; 16],
    pub import_id: String,
    pub start_digest: String,
    pub receipt_digest: String,
    pub prefix_digest: String,
    pub rel_turn: u64,
    pub global_version: u64,
    pub memory_version: u64,
    pub graph_version: u64,
    pub freshness_state_digest: String,
    pub freshness_memory_count: u64,
    /// Original completed-epoch authority when reconciliation rebases the
    /// settlement onto a destination that already has independent Freshness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_source_authority: Option<ImportFreshnessAuthority>,
}

impl ImportFreshnessSettlement {
    pub fn reconciliation_authority(&self) -> ImportFreshnessAuthority {
        self.replay_source_authority
            .clone()
            .unwrap_or_else(|| ImportFreshnessAuthority {
                state_digest: self.freshness_state_digest.clone(),
                memory_count: self.freshness_memory_count,
            })
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let body = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        let len = u32::try_from(body.len()).map_err(|_| "import settlement too large")?;
        let mut bytes = Vec::with_capacity(12 + body.len());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&len.to_le_bytes());
        bytes.extend_from_slice(&body);
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.get(..8) != Some(MAGIC.as_slice()) {
            return Err("unsupported import settlement marker".into());
        }
        let length = bytes.get(8..12).ok_or("truncated import settlement")?;
        let count = u32::from_le_bytes(length.try_into().unwrap()) as usize;
        let body = bytes
            .get(12..)
            .filter(|body| body.len() == count && count <= 4096)
            .ok_or("invalid import settlement length")?;
        serde_json::from_slice(body).map_err(|e| e.to_string())
    }

    pub fn digest(&self) -> Result<String, String> {
        Ok(format!("{:x}", Sha256::digest(self.encode()?)))
    }
}

pub fn hex_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
const PREFIX_DOMAIN: &[u8] = b"reliquary-import-physical-prefix-v1\0";

pub fn prefix_hasher() -> Sha256 {
    let mut hash = Sha256::new();
    hash.update(PREFIX_DOMAIN);
    hash
}

pub fn push_frame(hash: &mut Sha256, payload: &[u8]) {
    hash.update((payload.len() as u64).to_le_bytes());
    hash.update(payload);
}

pub(super) const PREFIX_BUFFER_BYTES: usize = 64 * 1024;

pub(super) fn push_streamed_frame(
    hash: &mut Sha256,
    prefix: &[u8],
    frame: &mut reliquary_container::FramePayloadReader<'_>,
    buffer: &mut [u8],
) -> Result<(), String> {
    hash.update(frame.object().length.to_le_bytes());
    hash.update(prefix);
    loop {
        let count = frame.read(buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            return Ok(());
        }
        hash.update(&buffer[..count]);
    }
}

pub fn source_prefix_digest(container: &reliquary_container::Container) -> Result<String, String> {
    let mut hash = prefix_hasher();
    let mut cursor = container.physical_cursor().map_err(|e| e.to_string())?;
    let mut buffer = vec![0; PREFIX_BUFFER_BYTES];
    while let Some(mut frame) = cursor.next_frame().map_err(|e| e.to_string())? {
        push_streamed_frame(&mut hash, &[], &mut frame, &mut buffer)?;
    }
    Ok(format!("{:x}", hash.finalize()))
}
