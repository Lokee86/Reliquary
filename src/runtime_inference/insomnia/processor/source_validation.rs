use crate::InsomniaCandidate;
use crate::archive::ArchiveReadView;

pub(super) fn candidate_source_time_ns(
    archive: &ArchiveReadView<'_>,
    candidate: &InsomniaCandidate,
    turns: &[crate::ResolvedTurn],
) -> Result<i64, super::InsomniaProcessError> {
    if let (Some(conversation_id), Some(node_id)) = (
        candidate.authority_source_conversation_id.as_deref(),
        candidate.authority_source_node_id.as_deref(),
    ) {
        return archive
            .node(conversation_id, node_id)
            .map(|node| node.timestamp_ns)
            .ok_or_else(|| {
                super::InsomniaProcessError::InvalidCandidate(
                    "authority source timestamp is unavailable".into(),
                )
            });
    }
    turns
        .iter()
        .find(|turn| turn.node_id == candidate.source_node_id)
        .map(|turn| turn.timestamp_ns)
        .ok_or_else(|| {
            super::InsomniaProcessError::InvalidCandidate("source timestamp is unavailable".into())
        })
}

pub(super) fn validate_candidate_sources(
    archive: &ArchiveReadView<'_>,
    candidate: &InsomniaCandidate,
    episode_conversation_id: &str,
    episode_turns: &[crate::ResolvedTurn],
    evidence_turns: &[crate::InsomniaEvidenceTurn],
) -> Result<(), String> {
    let authority = episode_turns
        .iter()
        .find(|turn| turn.node_id == candidate.source_node_id)
        .ok_or_else(|| "authority source disappeared from episode".to_owned())?;
    validate_support_source(
        archive,
        candidate.authority_source_conversation_id.as_deref(),
        candidate.authority_source_node_id.as_deref(),
        candidate.authority_source_quote.as_deref(),
        true,
        "authority",
        authority.timestamp_ns,
        episode_conversation_id,
        episode_turns,
        evidence_turns,
    )?;
    validate_support_source(
        archive,
        candidate.grounding_source_conversation_id.as_deref(),
        candidate.grounding_source_node_id.as_deref(),
        candidate.grounding_source_quote.as_deref(),
        false,
        "grounding",
        authority.timestamp_ns,
        episode_conversation_id,
        episode_turns,
        evidence_turns,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_support_source(
    archive: &ArchiveReadView<'_>,
    conversation_id: Option<&str>,
    node_id: Option<&str>,
    quote: Option<&str>,
    assistant_only: bool,
    label: &str,
    authority_timestamp_ns: i64,
    episode_conversation_id: &str,
    episode_turns: &[crate::ResolvedTurn],
    evidence_turns: &[crate::InsomniaEvidenceTurn],
) -> Result<(), String> {
    let Some(conversation_id) = conversation_id else {
        return Ok(());
    };
    let node_id = node_id.ok_or_else(|| format!("{label} source node is missing"))?;
    let quote = quote.ok_or_else(|| format!("{label} source quote is missing"))?;
    let in_episode = conversation_id == episode_conversation_id
        && episode_turns.iter().any(|turn| turn.node_id == node_id);
    let in_evidence = evidence_turns
        .iter()
        .any(|turn| turn.conversation_id == conversation_id && turn.node_id == node_id);
    if !in_episode && !in_evidence {
        return Err(format!(
            "external {label} source was not supplied by bounded archive evidence"
        ));
    }
    let source = archive
        .require_node(
            conversation_id,
            node_id,
            crate::ArchiveError::MissingEpisodeNode,
        )
        .map_err(|_| format!("{label} source node does not exist"))?
        .clone();
    if assistant_only && source.role != "assistant" {
        return Err("authority source must be an assistant-authored turn".into());
    }
    if source.timestamp_ns > authority_timestamp_ns {
        return Err(format!("{label} source must not postdate user authority"));
    }
    let content = archive
        .content(source.content_id)
        .map_err(|_| format!("{label} source body is unavailable"))?;
    if quote.trim().is_empty() || !content.contains(quote.trim()) {
        return Err(format!("{label} source quote is not verbatim"));
    }
    Ok(())
}
