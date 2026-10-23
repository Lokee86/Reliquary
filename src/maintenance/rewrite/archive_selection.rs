//! Read-only Archive provenance and explicit selection for rewriting.
use crate::CvaReconcileError;

pub(crate) fn rewrite_archive_conversations(
    archive: &crate::Archive,
    container: &crate::Container,
    memories: &[crate::Memory],
    selection: &super::model::ArchiveSelection,
) -> Result<std::collections::BTreeSet<String>, CvaReconcileError> {
    use std::collections::BTreeSet;

    let mut conversations = BTreeSet::new();
    for memory in memories {
        for conversation in [
            memory.content_source_conversation_id.as_deref(),
            memory.grounding_source_conversation_id.as_deref(),
            memory
                .source_ref
                .as_ref()
                .and_then(|source| source.content_source_conversation_id.as_deref()),
            memory
                .source_ref
                .as_ref()
                .and_then(|source| source.grounding_source_conversation_id.as_deref()),
        ]
        .into_iter()
        .flatten()
        {
            conversations.insert(conversation.to_owned());
        }
        if let Some(episode_id) = memory.source_episode_id {
            let episode = archive.historical_episode(episode_id).ok_or(
                CvaReconcileError::UnsupportedSemanticOwner(
                    "retained Memory source Episode is unavailable",
                ),
            )?;
            conversations.insert(episode.conversation_id.clone());
        }
        if let Some(source_ref) = &memory.source_ref {
            if let Some(episode) = archive.historical_episode(source_ref.source_episode_id) {
                conversations.insert(episode.conversation_id.clone());
            }
        }
    }
    match selection {
        super::model::ArchiveSelection::ProvenanceClosureOnly => {}
        super::model::ArchiveSelection::AllRetained => {
            conversations.extend(
                archive
                    .historical_episodes()
                    .into_iter()
                    .map(|episode| episode.conversation_id),
            );
            conversations.extend(
                archive
                    .historical_branches()
                    .into_iter()
                    .map(|branch| branch.conversation_id),
            );
            conversations.extend(
                archive
                    .conversation_summaries()
                    .into_iter()
                    .map(|summary| summary.conversation_id),
            );
        }
        super::model::ArchiveSelection::CurrentFromActivity(cut) => {
            for summary in archive.conversation_summaries() {
                let mut has_recent = false;
                for leaf in &summary.leaf_node_ids {
                    let turns = archive
                        .historical_conversation_turns(container, &summary.conversation_id, leaf)
                        .map_err(CvaReconcileError::from)?;
                    if turns.iter().any(|turn| {
                        archive
                            .activity_position_for_turn(&summary.conversation_id, &turn.node_id)
                            .is_some_and(|position| position >= *cut)
                    }) {
                        has_recent = true;
                        break;
                    }
                }
                if has_recent {
                    conversations.insert(summary.conversation_id);
                }
            }
        }
        super::model::ArchiveSelection::Conversations(ids) => {
            conversations.extend(ids.iter().cloned());
        }
    }

    Ok(conversations)
}
