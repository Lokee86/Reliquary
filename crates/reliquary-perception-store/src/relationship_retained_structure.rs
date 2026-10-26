//! Canonical Relationship descriptor hash schema, never a persisted carrier.
//! The historical discriminator is immutable hash input, not a decoder dependency.
use crate::RelationshipError;
use crate::relationship_model::RelationshipRecord;

pub(crate) fn encode_record_structure(
    record: &RelationshipRecord,
) -> Result<Vec<u8>, RelationshipError> {
    let mut out = b"CVARELR1".to_vec();
    out.extend_from_slice(&record.id.0);
    out.extend_from_slice(&record.revision.to_le_bytes());
    out.extend_from_slice(&record.created_at_ns.to_le_bytes());
    out.extend_from_slice(&record.updated_at_ns.to_le_bytes());
    write_string(&mut out, &record.kind)?;
    write_count(&mut out, record.participants.len())?;
    for value in &record.participants {
        write_string(&mut out, &value.entity.owner_id)?;
        out.extend_from_slice(&value.entity.entity_id.0);
        match &value.role {
            Some(role) => {
                out.push(1);
                write_string(&mut out, role)?;
            }
            None => out.push(0),
        }
    }
    write_count(&mut out, record.evidence.len())?;
    for value in &record.evidence {
        write_string(&mut out, &value.owner_id)?;
        out.extend_from_slice(&value.memory_id);
    }
    write_string(&mut out, "")?;
    write_string(&mut out, &record.mutation_id)?;
    Ok(out)
}
fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), RelationshipError> {
    let len = u32::try_from(value.len()).map_err(|_| RelationshipError::FieldTooLarge)?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}
fn write_count(out: &mut Vec<u8>, count: usize) -> Result<(), RelationshipError> {
    let count = u16::try_from(count).map_err(|_| RelationshipError::FieldTooLarge)?;
    out.extend_from_slice(&count.to_le_bytes());
    Ok(())
}

#[cfg(test)]
#[path = "relationship_retained_structure_tests.rs"]
mod tests;
