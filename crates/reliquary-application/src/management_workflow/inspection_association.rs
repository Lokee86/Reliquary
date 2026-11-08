//! Checked user-origin Memory association workflow.

use super::*;
use crate::management_workflow::user_mutation_id;
use reliquary_owner_session::{MemoryAssociationCut, RelWriter};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryAssociationAction {
    Link,
    Unlink,
}

#[derive(Clone, Debug)]
pub struct MemoryAssociationRequest {
    pub request_id: String,
    pub source_id: MemoryId,
    pub target_id: MemoryId,
    pub source_revision: u64,
    pub target_revision: u64,
    pub expected_edge_id: Option<String>,
    pub expected_edge_revision: Option<u64>,
    pub action: MemoryAssociationAction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MemoryAssociationReceipt {
    pub edge: MemoryGraphEdge,
    pub cut: InspectionCut,
    /// True for an exact replay of the original committed effect.
    pub replayed: bool,
}

pub fn change_memory_association(
    writer: &mut RelWriter,
    principal: &str,
    expected: InspectionCut,
    request: MemoryAssociationRequest,
) -> Result<MemoryAssociationReceipt, ManagementError> {
    if request.source_id == request.target_id
        || request.source_revision == 0
        || request.target_revision == 0
        || request.expected_edge_revision == Some(0)
        || request.expected_edge_id.is_some() != request.expected_edge_revision.is_some()
        || (request.action == MemoryAssociationAction::Unlink && request.expected_edge_id.is_none())
    {
        return Err(ManagementError::InvalidInput("Memory association request"));
    }
    let source = request.source_id.0;
    let target = request.target_id.0;
    let current = inspection_cut(&writer.read_session().management_read());
    if current.owner_id != expected.owner_id || current.generation != expected.generation {
        return Err(ManagementError::Stale);
    }
    let id = graph::edge_id(current.owner_id, source, target, RelationKind::Topical);
    if request
        .expected_edge_id
        .as_ref()
        .is_some_and(|expected| expected != &id)
    {
        return Err(ManagementError::Stale);
    }
    let mutation_id = user_mutation_id(principal, "memory-association", &request.request_id)?;
    let request_key = Sha256::digest(mutation_id.as_bytes()).into();
    let bytes = serde_json::to_vec(&(
        expected,
        source,
        target,
        request.source_revision,
        request.target_revision,
        request.expected_edge_revision,
        request.action,
    ))
    .map_err(|error| ManagementError::Owner(error.to_string()))?;
    let fingerprint = Sha256::digest(bytes).into();
    let prior = writer
        .read_session()
        .memory_association_receipt(request_key, fingerprint)
        .map_err(map_error)?;
    let relation = match prior {
        Some(relation) => relation,
        None => {
            if current != expected {
                return Err(ManagementError::Stale);
            }
            writer
                .publish_memory_association(MemoryAssociationCut {
                    request_key,
                    fingerprint,
                    source: request.source_id,
                    target: request.target_id,
                    source_revision: request.source_revision,
                    target_revision: request.target_revision,
                    graph_version: expected.graph,
                    expected_edge_revision: request.expected_edge_revision,
                    active: request.action == MemoryAssociationAction::Link,
                })
                .map_err(map_error)?
        }
    };
    Ok(MemoryAssociationReceipt {
        edge: MemoryGraphEdge {
            id,
            edge: semantic::graph_edge(&relation, None),
        },
        cut: inspection_cut(&writer.read_session().management_read()),
        replayed: prior.is_some(),
    })
}

fn map_error(error: OwnerSessionError) -> ManagementError {
    match error {
        OwnerSessionError::ManagementMissing => ManagementError::NotFound,
        OwnerSessionError::ManagementStale => ManagementError::Stale,
        OwnerSessionError::ManagementConflict => ManagementError::Conflict,
        other => ManagementError::Owner(other.to_string()),
    }
}
