use super::EgoStore;
use crate::EgoError;

impl EgoStore {
    pub fn validate_memory_version(&self, current: u64) -> Result<(), EgoError> {
        if let Some(personality) = &self.personality {
            validate_source_memory_version(personality.source_memory_version, current)?;
        }
        if let Some(synthesis) = &self.synthesis {
            validate_source_memory_version(synthesis.source_memory_version, current)?;
        }
        Ok(())
    }
}

pub fn validate_source_memory_version(source: u64, current: u64) -> Result<(), EgoError> {
    if source > current {
        return Err(EgoError::InvalidSourceMemoryVersion { source, current });
    }
    Ok(())
}

pub(super) fn next_revision(revision: u64) -> Result<u64, EgoError> {
    revision
        .checked_add(1)
        .ok_or_else(|| EgoError::InvalidRecord("Ego revision exhausted".into()))
}

pub(super) fn check_revision(expected: u64, actual: u64) -> Result<(), EgoError> {
    if expected != actual {
        return Err(EgoError::RevisionConflict { expected, actual });
    }
    Ok(())
}

pub(super) fn validate_record_revision(
    previous: Option<u64>,
    revision: u64,
) -> Result<(), EgoError> {
    let expected = match previous {
        Some(value) => next_revision(value)?,
        None => 1,
    };
    if revision != expected {
        return Err(EgoError::InvalidRecord(format!(
            "record revision expected {expected}, got {revision}"
        )));
    }
    Ok(())
}
