//! Ego compact frames. Creation revision is implicit; terminals retain exact priority.
#[path = "ego_compact_delta_codec.rs"]
mod delta;
use super::EgoRecord;
use super::scalar::{
    invalid, priority_byte, priority_from_byte, read_array, read_byte, read_string, read_u64,
    write_string, write_u64,
};
use crate::ego_compact_contract::{EgoCreation, EgoTerminal};
use crate::{EgoAnchorId, EgoError, EgoIdentityId};
pub use delta::{decode_delta, encode_delta};

const CREATION_MAGIC: &[u8; 8] = b"CVAECR01";
const TERMINAL_MAGIC: &[u8; 8] = b"CVAETM01";

/// Fresh-history rewrite emits only first revisions and independent activation.
pub fn encode_initial(record: &EgoRecord) -> Result<Vec<u8>, EgoError> {
    if let EgoRecord::IdentityActive { ego_version, id } = record {
        crate::ego_compact_contract::validate(record)?;
        Ok(super::encode_activation(*ego_version, *id))
    } else {
        encode_creation(&EgoCreation::new(record.clone())?)
    }
}

/// Unknown versions in a recognized family are errors, never opaque payloads.
pub fn is_compact_record(bytes: &[u8]) -> Result<bool, EgoError> {
    if bytes.starts_with(b"CVAECR") {
        decode_creation(bytes)?;
    } else if bytes.starts_with(b"CVAEDL") {
        decode_delta(bytes)?;
    } else if bytes.starts_with(b"CVAETM") {
        decode_terminal(bytes)?;
    } else {
        return Ok(false);
    }
    Ok(true)
}

pub fn encode_creation(creation: &EgoCreation) -> Result<Vec<u8>, EgoError> {
    let creation = EgoCreation::new(creation.record.clone())?;
    let mut out = CREATION_MAGIC.to_vec();
    write_document(&mut out, &creation.record, false)?;
    Ok(out)
}

pub fn decode_creation(bytes: &[u8]) -> Result<EgoCreation, EgoError> {
    require_magic(bytes, CREATION_MAGIC)?;
    let mut cursor = 8;
    let record = read_document(bytes, &mut cursor, false)?;
    finish(bytes, cursor)?;
    EgoCreation::new(record)
}

pub fn encode_terminal(terminal: &EgoTerminal) -> Result<Vec<u8>, EgoError> {
    EgoTerminal::new(terminal.record.clone())?;
    let mut out = TERMINAL_MAGIC.to_vec();
    write_document(&mut out, &terminal.record, true)?;
    Ok(out)
}

pub fn decode_terminal(bytes: &[u8]) -> Result<EgoTerminal, EgoError> {
    require_magic(bytes, TERMINAL_MAGIC)?;
    let mut cursor = 8;
    let record = read_document(bytes, &mut cursor, true)?;
    finish(bytes, cursor)?;
    EgoTerminal::new(record)
}

fn write_document(out: &mut Vec<u8>, record: &EgoRecord, terminal: bool) -> Result<(), EgoError> {
    let kind = match record {
        EgoRecord::Identity { .. } => 1,
        EgoRecord::Anchor { .. } => 2,
        EgoRecord::Personality { .. } => 3,
        EgoRecord::Synthesis { .. } => 4,
        _ => return Err(invalid("unsupported compact Ego document")),
    };
    out.push(kind);
    write_u64(out, super::record_ego_version(record));
    match record {
        EgoRecord::Identity {
            id,
            revision,
            activate,
            name,
            text,
            ..
        } => {
            out.extend_from_slice(&id.0);
            if terminal {
                write_u64(out, *revision);
            } else {
                out.push(u8::from(*activate));
                write_string(out, name)?;
                write_string(out, text)?;
            }
        }
        EgoRecord::Anchor {
            id,
            revision,
            priority,
            text,
            ..
        } => {
            out.extend_from_slice(&id.0);
            if terminal {
                write_u64(out, *revision);
            }
            out.push(priority_byte(*priority));
            if !terminal {
                write_string(out, text)?;
            }
        }
        EgoRecord::Personality {
            source_memory_version,
            text,
            ..
        }
        | EgoRecord::Synthesis {
            source_memory_version,
            text,
            ..
        } => {
            if terminal {
                return Err(invalid("singleton Ego document has no terminal"));
            }
            write_u64(out, *source_memory_version);
            write_string(out, text)?;
        }
        _ => return Err(invalid("unsupported compact Ego document")),
    }
    Ok(())
}

fn read_document(bytes: &[u8], cursor: &mut usize, terminal: bool) -> Result<EgoRecord, EgoError> {
    let kind = read_byte(bytes, cursor)?;
    let ego_version = read_u64(bytes, cursor)?;
    Ok(match kind {
        1 => {
            let id = EgoIdentityId(read_array(bytes, cursor)?);
            let revision = if terminal {
                read_u64(bytes, cursor)?
            } else {
                1
            };
            let activate = if terminal {
                false
            } else {
                match read_byte(bytes, cursor)? {
                    0 => false,
                    1 => true,
                    _ => return Err(invalid("invalid compact Identity activation")),
                }
            };
            let (name, text) = if terminal {
                (String::new(), String::new())
            } else {
                (read_string(bytes, cursor)?, read_string(bytes, cursor)?)
            };
            EgoRecord::Identity {
                ego_version,
                id,
                revision,
                deleted: terminal,
                activate,
                name,
                text,
            }
        }
        2 => {
            let id = EgoAnchorId(read_array(bytes, cursor)?);
            let revision = if terminal {
                read_u64(bytes, cursor)?
            } else {
                1
            };
            let priority = priority_from_byte(read_byte(bytes, cursor)?)?;
            let text = if terminal {
                String::new()
            } else {
                read_string(bytes, cursor)?
            };
            EgoRecord::Anchor {
                ego_version,
                id,
                revision,
                deleted: terminal,
                priority,
                text,
            }
        }
        3 | 4 if !terminal => {
            let source_memory_version = read_u64(bytes, cursor)?;
            let text = read_string(bytes, cursor)?;
            if kind == 3 {
                EgoRecord::Personality {
                    ego_version,
                    revision: 1,
                    source_memory_version,
                    text,
                }
            } else {
                EgoRecord::Synthesis {
                    ego_version,
                    revision: 1,
                    source_memory_version,
                    text,
                }
            }
        }
        _ => return Err(invalid("unknown compact Ego document kind")),
    })
}

fn require_magic(bytes: &[u8], magic: &[u8; 8]) -> Result<(), EgoError> {
    if !bytes.starts_with(magic) {
        return Err(invalid("unsupported compact Ego frame"));
    }
    Ok(())
}

fn finish(bytes: &[u8], cursor: usize) -> Result<(), EgoError> {
    if cursor != bytes.len() {
        return Err(invalid("compact Ego frame has trailing bytes"));
    }
    Ok(())
}
