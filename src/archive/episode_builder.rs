use crate::episode_model::episode_id;
use crate::{
    Archive, ArchiveError, Container, Episode, EpisodeBoundary, EpisodeBuildResult, EpisodeConfig,
    EpisodeOrigin, Node,
};
use serde_json::json;

impl Archive {
    pub(crate) fn materialize_branch_episodes(
        &mut self,
        container: &mut Container,
        conversation_id: &str,
        branch_id: &str,
        config: EpisodeConfig,
        origin: EpisodeOrigin,
        close_tail: Option<(EpisodeBoundary, i64)>,
    ) -> Result<EpisodeBuildResult, ArchiveError> {
        let branch = self
            .branches
            .get(conversation_id, branch_id)
            .ok_or(ArchiveError::MissingBranch)?
            .clone();
        self.materialize_path_episodes(
            container,
            conversation_id,
            &branch.leaf_node_id,
            config,
            origin,
            close_tail,
        )
    }

    pub(crate) fn materialize_path_episodes(
        &mut self,
        container: &mut Container,
        conversation_id: &str,
        leaf_node_id: &str,
        config: EpisodeConfig,
        origin: EpisodeOrigin,
        close_tail: Option<(EpisodeBoundary, i64)>,
    ) -> Result<EpisodeBuildResult, ArchiveError> {
        if config.max_input_bytes == 0 {
            return Err(ArchiveError::InvalidEpisodeConfig);
        }
        let nodes = self.branch_nodes(conversation_id, leaf_node_id)?;
        if origin == EpisodeOrigin::Import
            || self
                .episodes
                .for_conversation(conversation_id)
                .iter()
                .any(|e| e.origin == EpisodeOrigin::Import)
        {
            return self.materialize_uncovered_path(
                container,
                conversation_id,
                &nodes,
                config,
                origin,
                close_tail,
            );
        }
        let start = self.uncovered_episode_start(conversation_id, &nodes)?;
        if start >= nodes.len() {
            return Ok(EpisodeBuildResult {
                created: Vec::new(),
                has_open_tail: false,
                open_from_node_id: None,
            });
        }
        let uncovered = &nodes[start..];
        let cycles = response_cycles(uncovered);
        let ranges = self.pack_cycles(container, uncovered, &cycles, config.max_input_bytes)?;
        let mut persisted = ranges.len();
        let has_open_tail = close_tail.is_none() && !ranges.is_empty();
        if has_open_tail {
            persisted -= 1;
        }
        let mut created = Vec::new();
        for (index, range) in ranges.iter().take(persisted).enumerate() {
            let first = &uncovered[range.0];
            let last = &uncovered[range.1];
            let (boundary, finalized_at_ns) = if index + 1 == persisted {
                close_tail.unwrap_or_else(|| {
                    let next_range = &ranges[index + 1];
                    (EpisodeBoundary::Size, uncovered[next_range.0].timestamp_ns)
                })
            } else {
                let next_range = &ranges[index + 1];
                (EpisodeBoundary::Size, uncovered[next_range.0].timestamp_ns)
            };
            let episode = Episode {
                id: episode_id(conversation_id, &first.id, &last.id),
                conversation_id: conversation_id.to_owned(),
                start_node_id: first.id.clone(),
                end_node_id: last.id.clone(),
                origin,
                boundary,
                source_through_ns: last.timestamp_ns,
                finalized_at_ns: finalized_at_ns.max(last.timestamp_ns),
            };
            if self.put_episode(container, episode.clone())? {
                created.push(episode);
            }
        }
        let open_from_node_id = has_open_tail.then(|| {
            let range = ranges.last().unwrap();
            uncovered[range.0].id.clone()
        });
        Ok(EpisodeBuildResult {
            created,
            has_open_tail,
            open_from_node_id,
        })
    }

    fn uncovered_episode_start(
        &self,
        conversation_id: &str,
        nodes: &[Node],
    ) -> Result<usize, ArchiveError> {
        let episodes = self.episodes.for_conversation(conversation_id);
        if episodes.is_empty() {
            return Ok(0);
        }
        let mut expected = 0usize;
        for episode in episodes {
            let start = nodes
                .iter()
                .position(|node| node.id == episode.start_node_id)
                .ok_or(ArchiveError::EpisodeBranchConflict)?;
            let end = nodes
                .iter()
                .position(|node| node.id == episode.end_node_id)
                .ok_or(ArchiveError::EpisodeBranchConflict)?;
            if start != expected || end < start {
                return Err(ArchiveError::EpisodeBranchConflict);
            }
            expected = end + 1;
        }
        Ok(expected)
    }

    pub(crate) fn pack_cycles(
        &self,
        container: &mut Container,
        nodes: &[Node],
        cycles: &[(usize, usize)],
        max_input_bytes: usize,
    ) -> Result<Vec<(usize, usize)>, ArchiveError> {
        if cycles.is_empty() {
            return Ok(Vec::new());
        }
        let mut packed = Vec::new();
        let mut current = cycles[0];
        for cycle in &cycles[1..] {
            let prospective = (current.0, cycle.1);
            let size =
                self.episode_payload_size(container, &nodes[prospective.0..=prospective.1])?;
            if size > max_input_bytes {
                packed.push(current);
                current = *cycle;
            } else {
                current.1 = cycle.1;
            }
        }
        packed.push(current);
        Ok(packed)
    }

    fn episode_payload_size(
        &self,
        container: &mut Container,
        nodes: &[Node],
    ) -> Result<usize, ArchiveError> {
        let mut turns = Vec::with_capacity(nodes.len());
        for node in nodes {
            turns.push(json!({
                "id": node.id,
                "conversation_id": node.conversation_id,
                "role": node.role,
                "timestamp_ns": node.timestamp_ns,
                "content": self.content(container, node.content_id)?,
            }));
        }
        serde_json::to_vec(&json!({"turns": turns}))
            .map(|bytes| bytes.len())
            .map_err(|_| ArchiveError::InvalidEpisodePayload)
    }
}

pub(crate) fn response_cycles(nodes: &[Node]) -> Vec<(usize, usize)> {
    if nodes.is_empty() {
        return Vec::new();
    }
    let mut cycles = Vec::new();
    let mut start = 0usize;
    let mut has_user = false;
    for (index, node) in nodes.iter().enumerate() {
        if node.role != "user" {
            continue;
        }
        if has_user {
            cycles.push((start, index - 1));
            start = index;
        }
        has_user = true;
    }
    cycles.push((start, nodes.len() - 1));
    cycles
}
