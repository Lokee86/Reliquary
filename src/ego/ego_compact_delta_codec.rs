//! Two-field Ego deltas. Successor revision is exactly predecessor + 1.
use super::super::scalar::{
    invalid, priority_byte, priority_from_byte, read_array, read_byte, read_string, read_u64,
    write_string, write_u64,
};
use crate::compact_delta::FieldChange;
use crate::ego_compact_contract::{EgoChanges, EgoDelta};
use crate::{EgoAnchorId, EgoError, EgoIdentityId};

const MAGIC: &[u8; 8] = b"CVAEDL01";

pub(crate) fn encode_delta(delta: &EgoDelta) -> Result<Vec<u8>, EgoError> {
    validate(delta)?;
    let mut out = MAGIC.to_vec();
    out.push(match &delta.changes {
        EgoChanges::Identity { .. } => 1,
        EgoChanges::Anchor { .. } => 2,
        EgoChanges::Personality { .. } => 3,
        EgoChanges::Synthesis { .. } => 4,
    });
    write_u64(&mut out, delta.ego_version);
    write_u64(&mut out, delta.previous_revision);
    match &delta.changes {
        EgoChanges::Identity {
            id,
            activate,
            name,
            text,
        } => {
            out.extend_from_slice(&id.0);
            out.push(u8::from(*activate));
            out.push(mask(name, text));
            write_text(&mut out, name)?;
            write_text(&mut out, text)?;
        }
        EgoChanges::Anchor { id, priority, text } => {
            out.extend_from_slice(&id.0);
            out.push(mask(priority, text));
            if let FieldChange::Set(value) = priority {
                out.push(priority_byte(*value));
            }
            write_text(&mut out, text)?;
        }
        EgoChanges::Personality {
            source_memory_version,
            text,
        }
        | EgoChanges::Synthesis {
            source_memory_version,
            text,
        } => {
            out.push(mask(source_memory_version, text));
            if let FieldChange::Set(value) = source_memory_version {
                write_u64(&mut out, *value);
            }
            write_text(&mut out, text)?;
        }
    }
    Ok(out)
}

pub(crate) fn decode_delta(bytes: &[u8]) -> Result<EgoDelta, EgoError> {
    super::require_magic(bytes, MAGIC)?;
    let mut cursor = 8;
    let kind = read_byte(bytes, &mut cursor)?;
    let ego_version = read_u64(bytes, &mut cursor)?;
    let previous_revision = read_u64(bytes, &mut cursor)?;
    let revision = previous_revision
        .checked_add(1)
        .ok_or_else(|| invalid("Ego revision exhausted"))?;
    let changes = match kind {
        1 => {
            let id = EgoIdentityId(read_array(bytes, &mut cursor)?);
            let activate = match read_byte(bytes, &mut cursor)? {
                0 => false,
                1 => true,
                _ => return Err(invalid("invalid compact Identity activation")),
            };
            let bits = read_mask(bytes, &mut cursor)?;
            EgoChanges::Identity {
                id,
                activate,
                name: read_text(bytes, &mut cursor, bits & 1 != 0)?,
                text: read_text(bytes, &mut cursor, bits & 2 != 0)?,
            }
        }
        2 => {
            let id = EgoAnchorId(read_array(bytes, &mut cursor)?);
            let bits = read_mask(bytes, &mut cursor)?;
            let priority = if bits & 1 != 0 {
                FieldChange::Set(priority_from_byte(read_byte(bytes, &mut cursor)?)?)
            } else {
                FieldChange::Unchanged
            };
            EgoChanges::Anchor {
                id,
                priority,
                text: read_text(bytes, &mut cursor, bits & 2 != 0)?,
            }
        }
        3 | 4 => {
            let bits = read_mask(bytes, &mut cursor)?;
            let source_memory_version = if bits & 1 != 0 {
                FieldChange::Set(read_u64(bytes, &mut cursor)?)
            } else {
                FieldChange::Unchanged
            };
            let text = read_text(bytes, &mut cursor, bits & 2 != 0)?;
            if kind == 3 {
                EgoChanges::Personality {
                    source_memory_version,
                    text,
                }
            } else {
                EgoChanges::Synthesis {
                    source_memory_version,
                    text,
                }
            }
        }
        _ => return Err(invalid("unknown compact Ego delta kind")),
    };
    super::finish(bytes, cursor)?;
    let delta = EgoDelta {
        ego_version,
        previous_revision,
        revision,
        changes,
    };
    validate(&delta)?;
    Ok(delta)
}

fn validate(delta: &EgoDelta) -> Result<(), EgoError> {
    if delta.ego_version == 0
        || delta.previous_revision == 0
        || delta.previous_revision.checked_add(1) != Some(delta.revision)
    {
        return Err(invalid("invalid compact Ego delta revision"));
    }
    match &delta.changes {
        EgoChanges::Identity { name, text, .. } => {
            validate_text(name)?;
            validate_text(text)?;
        }
        EgoChanges::Anchor { text, .. } => {
            if let FieldChange::Set(value) = text {
                crate::ego_codec::validate_anchor_text(value)?;
            }
        }
        EgoChanges::Personality { text, .. } | EgoChanges::Synthesis { text, .. } => {
            validate_text(text)?
        }
    }
    Ok(())
}

fn validate_text(value: &FieldChange<String>) -> Result<(), EgoError> {
    if let FieldChange::Set(value) = value {
        crate::ego_codec::validate_text("compact Ego", value)?;
    }
    Ok(())
}

fn mask<A, B>(a: &FieldChange<A>, b: &FieldChange<B>) -> u8 {
    u8::from(matches!(a, FieldChange::Set(_))) | (u8::from(matches!(b, FieldChange::Set(_))) << 1)
}

fn read_mask(bytes: &[u8], cursor: &mut usize) -> Result<u8, EgoError> {
    let value = read_byte(bytes, cursor)?;
    if value & !3 != 0 {
        return Err(invalid("unknown compact Ego delta fields"));
    }
    Ok(value)
}

fn write_text(out: &mut Vec<u8>, value: &FieldChange<String>) -> Result<(), EgoError> {
    if let FieldChange::Set(value) = value {
        write_string(out, value)?;
    }
    Ok(())
}

fn read_text(
    bytes: &[u8],
    cursor: &mut usize,
    changed: bool,
) -> Result<FieldChange<String>, EgoError> {
    if changed {
        Ok(FieldChange::Set(read_string(bytes, cursor)?))
    } else {
        Ok(FieldChange::Unchanged)
    }
}
