//! Authenticated user rename over an exact accepted owner and Community cut.

use super::{ManagementError, inspection, user_mutation_id};
use inspection::{CommunityMetadata, CommunityNameOrigin, InspectionCut};
use reliquary_owner_session::model::{CommunityId, CommunityRename, validate_community_name};
use reliquary_owner_session::{OwnerSessionError, RelWriter};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommunityRenameRequest {
    pub request_id: String,
    pub community_id: [u8; 32],
    pub expected_generation: u64,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CommunityRenameResult {
    pub community: CommunityMetadata,
    pub cut: InspectionCut,
    /// The original accepted effect, including on exact replay.
    pub changed: bool,
    pub replayed: bool,
}

pub fn rename_community_at_cut(
    writer: &mut RelWriter,
    principal: &str,
    expected: InspectionCut,
    request: CommunityRenameRequest,
) -> Result<CommunityRenameResult, ManagementError> {
    validate_community_name(&request.name)
        .map_err(|_| ManagementError::InvalidInput("Community name"))?;
    let key = user_mutation_id(principal, "community_rename", &request.request_id)?;
    let key = Sha256::digest(key.as_bytes()).into();
    let read = writer.read_session().management_read();
    let current = inspection::inspection_cut(&read);
    if current.owner_id != expected.owner_id
        || current.generation != expected.generation
        || expected.community != Some(request.expected_generation)
    {
        return Err(ManagementError::Stale);
    }
    let replayed = read.community_rename_receipt(&key).is_some();
    if !replayed && current != expected {
        return Err(ManagementError::Stale);
    }
    let cut_bytes =
        serde_json::to_vec(&expected).map_err(|error| ManagementError::Owner(error.to_string()))?;
    let accepted = writer
        .rename_management_community(CommunityRename {
            request_key: key,
            cut_digest: Sha256::digest(cut_bytes).into(),
            id: CommunityId(request.community_id),
            expected_generation: request.expected_generation,
            graph_version: expected.graph,
            name: request.name,
        })
        .map_err(|error| match error {
            OwnerSessionError::ManagementStale => ManagementError::Stale,
            OwnerSessionError::ManagementConflict => ManagementError::Conflict,
            error => ManagementError::Owner(error.to_string()),
        })?
        .ok_or(ManagementError::NotFound)?;
    let read = writer.read_session().management_read();
    let cut = inspection::inspection_cut(&read);
    let changed = accepted.changed();
    let id = accepted.request.id.0;
    let current =
        cut.community == Some(accepted.generation) && cut.graph == accepted.request.graph_version;
    Ok(CommunityRenameResult {
        community: CommunityMetadata {
            id,
            generation: accepted.generation,
            graph_version: accepted.request.graph_version,
            member_count: accepted.member_count,
            name: Some(accepted.request.name),
            name_origin: CommunityNameOrigin::User,
            current,
        },
        cut,
        changed,
        replayed,
    })
}
