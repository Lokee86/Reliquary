use crate::episode_model::episode_id;
use crate::{
    Archive, ArchiveError, Container, Episode, EpisodeBoundary, EpisodeBuildResult, EpisodeConfig,
    EpisodeOrigin, Node,
};
use std::collections::HashSet;

impl Archive {
    /// Imported history and its native continuation share per-Node coverage across
    /// branches. Conversations without import history retain the sequential policy.
    pub(crate) fn materialize_uncovered_path(
        &mut self,
        container: &mut Container,
        conversation_id: &str,
        nodes: &[Node],
        config: EpisodeConfig,
        origin: EpisodeOrigin,
        close_tail: Option<(EpisodeBoundary, i64)>,
    ) -> Result<EpisodeBuildResult, ArchiveError> {
        let mut covered = HashSet::new();
        for episode in self.episodes.for_conversation(conversation_id) {
            let path = self.branch_nodes(conversation_id, &episode.end_node_id)?;
            let start = path
                .iter()
                .position(|n| n.id == episode.start_node_id)
                .ok_or(ArchiveError::InvalidEpisodeRange)?;
            covered.extend(path[start..].iter().map(|n| n.id.clone()));
        }
        let mut created = Vec::new();
        let mut open_from_node_id = None;
        let mut index = 0;
        while index < nodes.len() {
            if covered.contains(&nodes[index].id) {
                index += 1;
                continue;
            }
            let begin = index;
            while index < nodes.len() && !covered.contains(&nodes[index].id) {
                index += 1;
            }
            let segment = &nodes[begin..index];
            let cycles = crate::episode_builder::response_cycles(segment);
            let ranges = self.pack_cycles(container, segment, &cycles, config.max_input_bytes)?;
            for (range_index, &(first, last)) in ranges.iter().enumerate() {
                let is_tail = index == nodes.len() && range_index + 1 == ranges.len();
                if is_tail && close_tail.is_none() {
                    open_from_node_id = Some(segment[first].id.clone());
                    continue;
                }
                let (boundary, finalized_at_ns) = if is_tail {
                    close_tail.unwrap()
                } else {
                    (EpisodeBoundary::Size, segment[last].timestamp_ns)
                };
                let episode = Episode {
                    id: episode_id(conversation_id, &segment[first].id, &segment[last].id),
                    conversation_id: conversation_id.to_owned(),
                    start_node_id: segment[first].id.clone(),
                    end_node_id: segment[last].id.clone(),
                    origin,
                    boundary,
                    source_through_ns: segment[last].timestamp_ns,
                    finalized_at_ns: finalized_at_ns.max(segment[last].timestamp_ns),
                };
                if self.put_episode(container, episode.clone())? {
                    created.push(episode);
                }
            }
        }
        Ok(EpisodeBuildResult {
            created,
            has_open_tail: open_from_node_id.is_some(),
            open_from_node_id,
        })
    }
}
