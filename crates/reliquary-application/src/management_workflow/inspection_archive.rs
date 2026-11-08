use super::*;
use crate::management_workflow::{ConversationInspection, ManagedBranch, hex};
pub use reliquary_owner_session::RelImportedSource as ImportedSourceReference;
use reliquary_owner_session::RelSession;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceAvailability {
    Available,
    Absent,
    Unavailable,
    Terminal,
    Direct,
    Restricted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceTurn {
    pub conversation_id: String,
    pub node_id: String,
    pub parent_node_id: Option<String>,
    pub role: String,
    pub principal_id: Option<String>,
    pub timestamp_ns: i64,
    pub content_id: String,
    pub archive_revision: Option<u64>,
    pub content: Option<String>,
    pub availability: SourceAvailability,
    pub imported: Option<ImportedSourceReference>,
}

pub(super) fn validate_keys(query: &InspectionQuery) -> Result<(), ManagementError> {
    let keys: &[&str] = match query {
        InspectionQuery::Branches { conversation } => &[conversation],
        InspectionQuery::Transcript { conversation, leaf } => &[conversation, leaf],
        InspectionQuery::SourceTurn { conversation, node } => &[conversation, node],
        _ => &[],
    };
    if keys
        .iter()
        .any(|key| key.trim().is_empty() || key.len() > 4096)
    {
        return Err(ManagementError::InvalidInput("Archive identity"));
    }
    Ok(())
}

pub(super) fn inspect(
    owner: &RelSession,
    query: &InspectionQuery,
    page: &mut PageBuilder,
) -> Result<(), ManagementError> {
    match query {
        InspectionQuery::Conversations => page.extend(
            owner.management_conversations().skip(page.offset),
            |branch| {
                let conversation = &branch.conversation_id;
                let (turn_count, _) = owner
                    .management_transcript_nodes(conversation, &branch.leaf_node_id, 0, 0)
                    .map_err(archive_error)?;
                Ok(InspectionItem::Conversation(ConversationInspection {
                    conversation_id: conversation.clone(),
                    title: owner
                        .management_conversation_title(conversation)
                        .map(str::to_owned),
                    branch: managed_branch(&branch.id, &branch.leaf_node_id, branch.canonical),
                    turn_count,
                }))
            },
        ),
        InspectionQuery::Branches { conversation } => {
            let mut branches = owner
                .management_branches(conversation)
                .ok_or(ManagementError::Terminal)?
                .peekable();
            if branches.peek().is_none() {
                return Err(ManagementError::NotFound);
            }
            page.extend(branches.skip(page.offset), |branch| {
                Ok(InspectionItem::Branch(managed_branch(
                    &branch.id,
                    &branch.leaf_node_id,
                    branch.canonical,
                )))
            })
        }
        InspectionQuery::Transcript { conversation, leaf } => {
            if owner.management_conversation_terminal(conversation) {
                return Err(ManagementError::Terminal);
            }
            let (_, nodes) = owner
                .management_transcript_nodes(conversation, leaf, page.offset, page.limit + 1)
                .map_err(archive_error)?;
            page.extend(nodes.into_iter(), |node| {
                source_turn(owner, conversation, &node, false)?
                    .map(InspectionItem::Turn)
                    .ok_or(ManagementError::NotFound)
            })
        }
        InspectionQuery::SourceTurn { conversation, node } => {
            let turn =
                source_turn(owner, conversation, node, true)?.ok_or(ManagementError::NotFound)?;
            page.single(InspectionItem::Turn(turn))
        }
        _ => unreachable!("Archive inspection query"),
    }
}

fn managed_branch(id: &str, leaf: &str, canonical: bool) -> ManagedBranch {
    ManagedBranch {
        id: id.to_owned(),
        leaf_node_id: leaf.to_owned(),
        canonical,
    }
}

pub(super) fn source_turn(
    owner: &RelSession,
    conversation: &str,
    node: &str,
    include_import_reference: bool,
) -> Result<Option<SourceTurn>, ManagementError> {
    Ok(owner
        .management_source_turn(
            conversation,
            node,
            MAX_MANAGEMENT_PAGE_BYTES - 4096,
            include_import_reference,
        )
        .map_err(archive_error)?
        .map(|turn| SourceTurn {
            availability: if turn.terminal {
                SourceAvailability::Terminal
            } else if turn.content.is_some() {
                SourceAvailability::Available
            } else {
                SourceAvailability::Unavailable
            },
            conversation_id: conversation.to_owned(),
            node_id: turn.node_id,
            parent_node_id: turn.parent_node_id,
            role: turn.role,
            principal_id: turn.principal_id,
            timestamp_ns: turn.timestamp_ns,
            content_id: hex(&turn.content_id),
            archive_revision: turn.archive_revision,
            content: turn.content,
            imported: turn.imported,
        }))
}

fn archive_error(error: OwnerSessionError) -> ManagementError {
    match error {
        OwnerSessionError::ManagementMissing => ManagementError::NotFound,
        OwnerSessionError::ManagementLimit => ManagementError::PageBytesExceeded,
        other => ManagementError::Owner(other.to_string()),
    }
}
