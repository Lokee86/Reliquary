//! Preflight proof that terminal Archive text is not retained by another owner.
use super::*;
use reliquary_archive::{ContentId, EpisodeId};

pub(super) fn verify_dependencies(
    session: &RelSession,
    selected: &BTreeSet<String>,
) -> Result<(), String> {
    let mut selected_content = BTreeSet::<ContentId>::new();
    let mut protected_content = BTreeSet::<ContentId>::new();
    let mut selected_nodes = BTreeSet::<String>::new();

    for source in session.archive.accepted_turn_sources() {
        let node = session
            .archive
            .node(&source.conversation_id, &source.node_id)
            .ok_or("Archive source node missing")?;
        if selected.contains(&source.conversation_id) {
            selected_content.insert(node.content_id);
            selected_nodes.insert(node.id.clone());
        } else {
            protected_content.insert(node.content_id);
        }
        // Archive attachment/file bytes are retained independently of text
        // pruning, even when attached to a selected terminal conversation.
        protected_content.extend(
            session
                .archive
                .files_for_source(&source.conversation_id, &source.node_id)
                .into_iter()
                .map(|attachment| attachment.content_id),
        );
    }
    if selected_content
        .iter()
        .any(|id| protected_content.contains(id))
    {
        return Err("selected Archive text is shared by retained turns or file bytes".into());
    }

    let episodes = session
        .archive
        .episodes()
        .into_iter()
        .filter(|episode| selected.contains(&episode.conversation_id))
        .map(|episode| episode.id)
        .collect::<BTreeSet<EpisodeId>>();
    for id in session.common.memories.memory_ids() {
        let history = session
            .common
            .memories
            .history(id)
            .map_err(|e| e.to_string())?;
        for revision in &history.revisions {
            let record = &revision.record;
            let direct = [
                &record.content_source_conversation_id,
                &record.grounding_source_conversation_id,
            ]
            .into_iter()
            .flatten()
            .any(|id| selected.contains(id))
                || record
                    .source_episode_id
                    .is_some_and(|id| episodes.contains(&EpisodeId(id)))
                || record
                    .source_node_id
                    .as_ref()
                    .is_some_and(|id| selected_nodes.contains(id));
            let indirect = record.source_ref.as_ref().is_some_and(|source| {
                [
                    &source.content_source_conversation_id,
                    &source.grounding_source_conversation_id,
                ]
                .into_iter()
                .flatten()
                .any(|id| selected.contains(id))
                    || episodes.contains(&EpisodeId(source.source_episode_id))
                    || selected_nodes.contains(&source.source_node_id)
            });
            if direct || indirect {
                return Err("selected Archive content is retained by Memory provenance".into());
            }
        }
    }

    // A queued or unqueued Insomnia Episode still needs the source transcript.
    // Only terminal or completed processing is independent of those bytes.
    for episode in &episodes {
        let work = session
            .insomnia
            .work(reliquary_insomnia_store::EpisodeId(episode.0));
        if !work.is_some_and(|work| {
            matches!(
                work.state,
                reliquary_insomnia_store::InsomniaWorkState::Complete
                    | reliquary_insomnia_store::InsomniaWorkState::Terminal
            )
        }) {
            return Err("selected Archive Episode has unfinished Insomnia work".into());
        }
    }

    for receipt in session.archive.import_receipts() {
        if receipt
            .conversations
            .iter()
            .any(|conversation| selected.contains(&conversation.conversation_id))
        {
            return Err("selected Archive content has an unresolved import mapping".into());
        }
    }

    let selected_fragments = session
        .archive
        .fragment_definitions()
        .into_iter()
        .filter(|definition| selected.contains(&definition.fragment.conversation_id))
        .map(|definition| reliquary_vector_store::ArchiveFragmentId(definition.fragment.id.0))
        .collect::<BTreeSet<_>>();
    for info in session.archive_vectors.infos() {
        if session
            .archive_vectors
            .get(info.id)
            .map_err(|e| e.to_string())?
            .fragment_ids
            .iter()
            .any(|id| selected_fragments.contains(id))
        {
            return Err("selected Archive content is retained by Archive vectors".into());
        }
    }
    Ok(())
}
