//! Exact Archive and Echo projections anchored to accepted copied receipts.
use super::echo::{MAX_ECHO_EVENTS, copied_echo};
use super::*;
use reliquary_owner_session::model::ResolvedTurn;

/// One exact Archive source and its bounded, redacted Echo projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HermesCopiedSourceTurn {
    pub source: HermesCopiedSource,
    pub archive: ResolvedTurn,
    pub echo: Vec<HermesCopiedEcho>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HermesCopiedSource {
    pub profile_id: String,
    pub session_id: String,
    pub message_uid: String,
    pub source_sha256: [u8; 32],
}

/// Read an exact inclusive source range from this principal's accepted copied receipt.
pub fn read_hermes_copy_source(
    session: &RelSession,
    scope: &HermesCopyScope,
    source_sha256: [u8; 32],
    start_message_uid: &str,
    end_message_uid: &str,
    principal: &str,
) -> Result<Option<Vec<HermesCopiedSourceTurn>>, reliquary_owner_session::OwnerSessionError> {
    let namespace = scope.namespace(principal);
    let Some(copy) = scope.accepted_conversation(session, &source_sha256, &namespace) else {
        return Ok(None);
    };
    if session
        .conversation_tombstone(&copy.conversation_id)
        .is_some()
    {
        return Ok(None);
    }
    let start = copy
        .messages
        .iter()
        .position(|row| row.source_id == start_message_uid);
    let end = copy
        .messages
        .iter()
        .position(|row| row.source_id == end_message_uid);
    let (Some(start), Some(end)) = (start, end) else {
        return Ok(None);
    };
    let Some(messages) = copy
        .messages
        .get(start..=end)
        .filter(|slice| !slice.is_empty())
    else {
        return Ok(None);
    };
    let mut turns = Vec::with_capacity(messages.len());
    for message in messages {
        let Some(node_id) = message.node_id.as_deref() else {
            return Ok(None);
        };
        let source = HermesCopiedSource {
            profile_id: scope.profile_id.clone(),
            session_id: scope.session_id.clone(),
            message_uid: message.source_id.clone(),
            source_sha256,
        };
        let Some(turn) = read_copied_turn(session, &copy.conversation_id, node_id, source)? else {
            return Ok(None);
        };
        turns.push(turn);
    }
    Ok(Some(turns))
}

/// Resolve an exact native Archive ancestry range against its generation cut.
pub fn read_hermes_archive_range(
    session: &RelSession,
    conversation_id: &str,
    start_node_id: &str,
    end_node_id: &str,
    source_archive_version: u64,
    principal: &str,
) -> Result<Option<Vec<HermesCopiedSourceTurn>>, reliquary_owner_session::OwnerSessionError> {
    if source_archive_version > session.archive_version()
        || session.conversation_tombstone(conversation_id).is_some()
    {
        return Ok(None);
    }
    let principal_suffix = format!("/principal-{:x}", Sha256::digest(principal.as_bytes()));
    let mut node_id = end_node_id;
    let mut turns = Vec::new();
    let mut bytes = 0;
    let mut event_count = 0;
    for _ in 0..MAX_MESSAGES {
        if session
            .archive_node_version(conversation_id, node_id)
            .is_none_or(|version| version > source_archive_version)
        {
            return Ok(None);
        }
        let Some((receipt, conversation, message)) =
            session.imported_archive_source(conversation_id, node_id)
        else {
            return Ok(None);
        };
        let Some(profile_id) = receipt
            .source_namespace
            .strip_prefix("hermes/profile/")
            .and_then(|namespace| namespace.strip_suffix(&principal_suffix))
        else {
            return Ok(None);
        };
        let Ok(scope) = HermesCopyScope::new(profile_id.to_owned(), conversation.source_id.clone())
        else {
            return Ok(None);
        };
        if receipt.source_format != FORMAT || scope.conversation_id() != conversation_id {
            return Ok(None);
        }
        let source = HermesCopiedSource {
            profile_id: scope.profile_id,
            session_id: scope.session_id,
            message_uid: message.source_id.clone(),
            source_sha256: receipt.source_digest,
        };
        let Some(turn) = read_copied_turn(session, conversation_id, node_id, source)? else {
            return Ok(None);
        };
        bytes += turn.archive.content.len();
        bytes += turn
            .echo
            .iter()
            .map(|event| match &event.kind {
                HermesCopiedEchoKind::ToolCall {
                    redacted_arguments, ..
                } => redacted_arguments.len(),
                HermesCopiedEchoKind::ToolResult {
                    redacted_result, ..
                } => redacted_result.len(),
                HermesCopiedEchoKind::ReasoningSummary { redacted_summary } => {
                    redacted_summary.len()
                }
                _ => 0,
            })
            .sum::<usize>();
        event_count += turn.echo.len();
        if bytes > MAX_COPY_BYTES || event_count > MAX_ECHO_EVENTS {
            return Ok(None);
        }
        turns.push(turn);
        if node_id == start_node_id {
            turns.reverse();
            return Ok(Some(turns));
        }
        let Some(parent) = session
            .archive_node(conversation_id, node_id)
            .and_then(|node| node.parent_id.as_deref())
        else {
            return Ok(None);
        };
        node_id = parent;
    }
    Ok(None)
}

fn read_copied_turn(
    session: &RelSession,
    conversation_id: &str,
    node_id: &str,
    source: HermesCopiedSource,
) -> Result<Option<HermesCopiedSourceTurn>, reliquary_owner_session::OwnerSessionError> {
    if session.archive_source_removed(conversation_id, node_id) {
        return Ok(None);
    }
    let Some(archive) = session.resolved_archive_turn(conversation_id, node_id)? else {
        return Ok(None);
    };
    let echo = session
        .echo_for_turn(conversation_id, node_id)
        .into_iter()
        .filter_map(copied_echo)
        .collect();
    Ok(Some(HermesCopiedSourceTurn {
        source,
        archive,
        echo,
    }))
}
