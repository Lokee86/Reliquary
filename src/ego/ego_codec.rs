#[path = "ego_compact_codec.rs"]
pub(crate) mod compact;
#[path = "ego_codec_scalar.rs"]
mod scalar;

use crate::{EgoAnchorId, EgoAnchorPriority, EgoError, EgoIdentityId};
use scalar::{invalid, read_array, read_u64};
pub(crate) use scalar::{validate_anchor_text, validate_text};

const IDENTITY_ACTIVE_MAGIC: [u8; 8] = *b"CVAEIDA1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum EgoRecord {
    Identity {
        ego_version: u64,
        id: EgoIdentityId,
        revision: u64,
        deleted: bool,
        activate: bool,
        name: String,
        text: String,
    },
    IdentityActive {
        ego_version: u64,
        id: EgoIdentityId,
    },
    Personality {
        ego_version: u64,
        revision: u64,
        source_memory_version: u64,
        text: String,
    },
    Anchor {
        ego_version: u64,
        id: EgoAnchorId,
        revision: u64,
        deleted: bool,
        priority: EgoAnchorPriority,
        text: String,
    },
    Synthesis {
        ego_version: u64,
        revision: u64,
        source_memory_version: u64,
        text: String,
    },
}

pub(crate) fn record_ego_version(record: &EgoRecord) -> u64 {
    match record {
        EgoRecord::Identity { ego_version, .. }
        | EgoRecord::IdentityActive { ego_version, .. }
        | EgoRecord::Personality { ego_version, .. }
        | EgoRecord::Anchor { ego_version, .. }
        | EgoRecord::Synthesis { ego_version, .. } => *ego_version,
    }
}

pub(crate) fn encode_activation(ego_version: u64, id: EgoIdentityId) -> Vec<u8> {
    let mut bytes = IDENTITY_ACTIVE_MAGIC.to_vec();
    bytes.extend_from_slice(&ego_version.to_le_bytes());
    bytes.extend_from_slice(&id.0);
    bytes
}
pub(crate) fn decode(bytes: &[u8]) -> Result<Option<EgoRecord>, EgoError> {
    if !bytes.starts_with(&IDENTITY_ACTIVE_MAGIC) {
        return Ok(None);
    }
    let mut cursor = 8;
    let ego_version = read_u64(bytes, &mut cursor)?;
    let id = EgoIdentityId(read_array::<16>(bytes, &mut cursor)?);
    if cursor != bytes.len() {
        return Err(invalid("activation record has trailing bytes"));
    }
    Ok(Some(EgoRecord::IdentityActive { ego_version, id }))
}
