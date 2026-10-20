#[path = "ego_compact_codec.rs"]
pub(crate) mod compact;
#[path = "ego_codec_scalar.rs"]
mod scalar;

use crate::{EgoAnchorId, EgoAnchorPriority, EgoError, EgoIdentityId};
use scalar::{
    invalid, priority_byte, priority_from_byte, read_array, read_byte, read_string, read_u64,
    write_string, write_u64,
};
pub(crate) use scalar::{validate_anchor_text, validate_text};

const LEGACY_IDENTITY_MAGIC: [u8; 8] = *b"CVAEIDN1";
const IDENTITY_MAGIC: [u8; 8] = *b"CVAEIDN2";
const IDENTITY_ACTIVE_MAGIC: [u8; 8] = *b"CVAEIDA1";
const PERSONALITY_MAGIC: [u8; 8] = *b"CVAEPER1";
const ANCHOR_MAGIC: [u8; 8] = *b"CVAEANC1";
const SYNTHESIS_MAGIC: [u8; 8] = *b"CVAESYN1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum EgoRecord {
    LegacyIdentity {
        ego_version: u64,
        revision: u64,
        text: String,
    },
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

pub(crate) fn encode(record: &EgoRecord) -> Result<Vec<u8>, EgoError> {
    let mut out = Vec::new();
    match record {
        EgoRecord::LegacyIdentity {
            ego_version,
            revision,
            text,
        } => {
            validate_text("identity", text)?;
            out.extend_from_slice(&LEGACY_IDENTITY_MAGIC);
            write_u64(&mut out, *ego_version);
            write_u64(&mut out, *revision);
            write_string(&mut out, text)?;
        }
        EgoRecord::Identity {
            ego_version,
            id,
            revision,
            deleted,
            activate,
            name,
            text,
        } => {
            if *deleted {
                if *activate || !name.is_empty() || !text.is_empty() {
                    return Err(invalid("deleted identity must be empty and inactive"));
                }
            } else {
                validate_text("identity name", name)?;
                validate_text("identity", text)?;
            }
            out.extend_from_slice(&IDENTITY_MAGIC);
            write_u64(&mut out, *ego_version);
            out.extend_from_slice(&id.0);
            write_u64(&mut out, *revision);
            out.push(u8::from(*deleted));
            out.push(u8::from(*activate));
            write_string(&mut out, name)?;
            write_string(&mut out, text)?;
        }
        EgoRecord::IdentityActive { ego_version, id } => {
            out.extend_from_slice(&IDENTITY_ACTIVE_MAGIC);
            write_u64(&mut out, *ego_version);
            out.extend_from_slice(&id.0);
        }
        EgoRecord::Personality {
            ego_version,
            revision,
            source_memory_version,
            text,
        }
        | EgoRecord::Synthesis {
            ego_version,
            revision,
            source_memory_version,
            text,
        } => {
            validate_text(
                if matches!(record, EgoRecord::Personality { .. }) {
                    "personality"
                } else {
                    "synthesis"
                },
                text,
            )?;
            out.extend_from_slice(if matches!(record, EgoRecord::Personality { .. }) {
                &PERSONALITY_MAGIC
            } else {
                &SYNTHESIS_MAGIC
            });
            write_u64(&mut out, *ego_version);
            write_u64(&mut out, *revision);
            write_u64(&mut out, *source_memory_version);
            write_string(&mut out, text)?;
        }
        EgoRecord::Anchor {
            ego_version,
            id,
            revision,
            deleted,
            priority,
            text,
        } => {
            if !*deleted {
                validate_anchor_text(text)?;
            } else if !text.is_empty() {
                return Err(invalid("deleted anchor must have an empty body"));
            }
            out.extend_from_slice(&ANCHOR_MAGIC);
            write_u64(&mut out, *ego_version);
            out.extend_from_slice(&id.0);
            write_u64(&mut out, *revision);
            out.push(u8::from(*deleted));
            out.push(priority_byte(*priority));
            write_string(&mut out, text)?;
        }
    }
    Ok(out)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Option<EgoRecord>, EgoError> {
    if bytes.len() < 8 {
        return Ok(None);
    }
    let mut cursor = 8;
    let record = if bytes[..8] == LEGACY_IDENTITY_MAGIC {
        let ego_version = read_u64(bytes, &mut cursor)?;
        let revision = read_u64(bytes, &mut cursor)?;
        let text = read_string(bytes, &mut cursor)?;
        validate_text("identity", &text)?;
        EgoRecord::LegacyIdentity {
            ego_version,
            revision,
            text,
        }
    } else if bytes[..8] == IDENTITY_MAGIC {
        let ego_version = read_u64(bytes, &mut cursor)?;
        let id = EgoIdentityId(read_array::<16>(bytes, &mut cursor)?);
        let revision = read_u64(bytes, &mut cursor)?;
        let deleted = flag(bytes, &mut cursor, "identity deletion flag")?;
        let activate = flag(bytes, &mut cursor, "identity activation flag")?;
        let name = read_string(bytes, &mut cursor)?;
        let text = read_string(bytes, &mut cursor)?;
        if deleted {
            if activate || !name.is_empty() || !text.is_empty() {
                return Err(invalid("deleted identity must be empty and inactive"));
            }
        } else {
            validate_text("identity name", &name)?;
            validate_text("identity", &text)?;
        }
        EgoRecord::Identity {
            ego_version,
            id,
            revision,
            deleted,
            activate,
            name,
            text,
        }
    } else if bytes[..8] == IDENTITY_ACTIVE_MAGIC {
        EgoRecord::IdentityActive {
            ego_version: read_u64(bytes, &mut cursor)?,
            id: EgoIdentityId(read_array::<16>(bytes, &mut cursor)?),
        }
    } else if bytes[..8] == PERSONALITY_MAGIC || bytes[..8] == SYNTHESIS_MAGIC {
        let ego_version = read_u64(bytes, &mut cursor)?;
        let revision = read_u64(bytes, &mut cursor)?;
        let source_memory_version = read_u64(bytes, &mut cursor)?;
        let text = read_string(bytes, &mut cursor)?;
        if bytes[..8] == PERSONALITY_MAGIC {
            validate_text("personality", &text)?;
            EgoRecord::Personality {
                ego_version,
                revision,
                source_memory_version,
                text,
            }
        } else {
            validate_text("synthesis", &text)?;
            EgoRecord::Synthesis {
                ego_version,
                revision,
                source_memory_version,
                text,
            }
        }
    } else if bytes[..8] == ANCHOR_MAGIC {
        let ego_version = read_u64(bytes, &mut cursor)?;
        let id = EgoAnchorId(read_array::<16>(bytes, &mut cursor)?);
        let revision = read_u64(bytes, &mut cursor)?;
        let deleted = flag(bytes, &mut cursor, "anchor deletion flag")?;
        let priority = priority_from_byte(read_byte(bytes, &mut cursor)?)?;
        let text = read_string(bytes, &mut cursor)?;
        if deleted {
            if !text.is_empty() {
                return Err(invalid("deleted anchor must have an empty body"));
            }
        } else {
            validate_anchor_text(&text)?;
        }
        EgoRecord::Anchor {
            ego_version,
            id,
            revision,
            deleted,
            priority,
            text,
        }
    } else {
        return Ok(None);
    };
    if cursor != bytes.len() {
        return Err(invalid("record has trailing bytes"));
    }
    Ok(Some(record))
}

fn flag(bytes: &[u8], cursor: &mut usize, name: &str) -> Result<bool, EgoError> {
    match read_byte(bytes, cursor)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(invalid(format!("{name} is invalid"))),
    }
}

pub(crate) fn record_ego_version(record: &EgoRecord) -> u64 {
    match record {
        EgoRecord::LegacyIdentity { ego_version, .. }
        | EgoRecord::Identity { ego_version, .. }
        | EgoRecord::IdentityActive { ego_version, .. }
        | EgoRecord::Personality { ego_version, .. }
        | EgoRecord::Anchor { ego_version, .. }
        | EgoRecord::Synthesis { ego_version, .. } => *ego_version,
    }
}
