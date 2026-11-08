//! Exact cut admission for user semantic corrections and terminal retirement.
use super::*;
use inspection::InspectionCut;
use reliquary_owner_session::model::MemoryBodyId;
use reliquary_owner_session::{MemoryRetirementRequest, UserMemoryCorrection};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorrectMemoryRequest {
    pub request_id: String,
    pub memory_id: MemoryId,
    pub expected_revision: u64,
    pub expected_body_id: MemoryBodyId,
    pub title: String,
    pub content: String,
    pub created_at_ns: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CorrectMemoryReceipt {
    pub original_memory_id: String,
    pub original_revision: u64,
    pub replacement: MemorySummary,
    pub cut: InspectionCut,
    pub created: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RetireMemoryReceipt {
    pub memory_id: String,
    pub revision: u64,
    pub memory_version: u64,
    pub cut: InspectionCut,
    pub retired: bool,
}

pub fn correct_memory_at_cut(
    writer: &mut RelWriter,
    principal: &str,
    expected: InspectionCut,
    request: CorrectMemoryRequest,
) -> Result<CorrectMemoryReceipt, ManagementError> {
    if request.expected_revision == 0
        || request.title.trim().is_empty()
        || request.content.trim().is_empty()
        || request.created_at_ns < 0
    {
        return Err(ManagementError::InvalidInput("Memory correction"));
    }
    let payload = serde_json::to_vec(&(
        request.memory_id.0,
        request.expected_revision,
        request.expected_body_id.0,
        &request.title,
        &request.content,
        request.created_at_ns,
    ))
    .map_err(|error| ManagementError::Owner(error.to_string()))?;
    if payload.len() > MAX_MANAGEMENT_PAGE_BYTES - 4096 {
        return Err(ManagementError::PageBytesExceeded);
    }
    let hash = Sha256::digest(payload).into();
    let key = user_mutation_id(principal, "memory-correction", &request.request_id)?;
    check_cut(writer, principal, expected, &key, hash)?;
    let result = writer
        .correct_user_memory(
            principal,
            key,
            hash,
            UserMemoryCorrection {
                id: request.memory_id,
                revision: request.expected_revision,
                body: request.expected_body_id,
                title: request.title,
                content: request.content,
                created_at_ns: request.created_at_ns,
            },
        )
        .map_err(user_memory_error)?;
    let replacement = summary(result.replacement)?;
    if !replacement.payload_available {
        return Err(ManagementError::Terminal);
    }
    Ok(CorrectMemoryReceipt {
        original_memory_id: hex(&request.memory_id.0),
        original_revision: result.archived_revision,
        replacement,
        cut: inspection::inspection_cut(&writer.read_session().management_read()),
        created: result.created,
    })
}

pub fn retire_memory_at_cut(
    writer: &mut RelWriter,
    principal: &str,
    expected: InspectionCut,
    request_id: &str,
    request: MemoryRetirementRequest,
) -> Result<RetireMemoryReceipt, ManagementError> {
    if request.memory_revision == 0
        || request.successor_revision == 0
        || request.memory_id == request.superseded_by
        || request.owner_id != expected.owner_id
    {
        return Err(ManagementError::InvalidInput("Memory retirement evidence"));
    }
    let payload = serde_json::to_vec(&(
        request.owner_id,
        request.memory_id.0,
        request.memory_revision,
        request.body_id.0,
        request.superseded_by.0,
        request.successor_revision,
        request.successor_body_id.0,
    ))
    .map_err(|error| ManagementError::Owner(error.to_string()))?;
    let hash = Sha256::digest(payload).into();
    let key = user_mutation_id(principal, "memory-retirement", request_id)?;
    check_cut(writer, principal, expected, &key, hash)?;
    let id = request.memory_id;
    let (tombstone, retired) = writer
        .retire_user_memory(principal, key, hash, request)
        .map_err(user_memory_error)?;
    Ok(RetireMemoryReceipt {
        memory_id: hex(&id.0),
        revision: tombstone.revision,
        memory_version: tombstone.memory_version,
        cut: inspection::inspection_cut(&writer.read_session().management_read()),
        retired,
    })
}

fn check_cut(
    writer: &RelWriter,
    principal: &str,
    expected: InspectionCut,
    key: &str,
    hash: [u8; 32],
) -> Result<(), ManagementError> {
    let current = inspection::inspection_cut(&writer.read_session().management_read());
    if current.owner_id != expected.owner_id || current.generation != expected.generation {
        return Err(ManagementError::Stale);
    }
    let replay = writer
        .has_user_memory_request(principal, key, hash)
        .map_err(user_memory_error)?;
    if !replay && current != expected {
        return Err(ManagementError::Stale);
    }
    Ok(())
}

fn user_memory_error(error: reliquary_owner_session::OwnerSessionError) -> ManagementError {
    use reliquary_owner_session::{OwnerSessionError, model::MemoryError};
    match error {
        OwnerSessionError::ManagementStale => ManagementError::Stale,
        OwnerSessionError::ManagementConflict => ManagementError::Conflict,
        OwnerSessionError::ManagementEvidence => {
            ManagementError::InvalidInput("Memory mutation evidence")
        }
        OwnerSessionError::Memory(MemoryError::MissingMemory) => ManagementError::NotFound,
        OwnerSessionError::Memory(MemoryError::TerminalTombstone) => ManagementError::Terminal,
        other => inspection::write_error(other),
    }
}
