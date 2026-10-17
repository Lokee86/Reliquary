use crate::episode_codec::encode_episode;
use crate::episode_model::episode_id;
use crate::{Archive, ArchiveError, Container, Episode, EpisodeId, Node};
use std::collections::HashSet;

impl Archive {
    pub(crate) fn put_episode(
        &mut self,
        container: &mut Container,
        mut episode: Episode,
    ) -> Result<bool, ArchiveError> {
        self.require_current_conversation_id(&episode.conversation_id)?;
        episode.id = episode_id(
            &episode.conversation_id,
            &episode.start_node_id,
            &episode.end_node_id,
        );
        self.validate_episode(&episode)?;
        if let Some(existing) = self.episodes.get(episode.id) {
            return if existing == &episode {
                Ok(false)
            } else {
                Err(ArchiveError::ConflictingEpisode)
            };
        }
        let record = container.append(&encode_episode(&episode)?)?;
        self.publish_record(container, record)?;
        self.episodes.insert(episode)
    }

    pub fn episodes(&self) -> Vec<Episode> {
        self.episodes
            .iter()
            .filter(|episode| !self.is_conversation_tombstoned(&episode.conversation_id))
            .cloned()
            .collect()
    }

    pub fn episodes_for_conversation(&self, conversation_id: &str) -> Vec<Episode> {
        if self.is_conversation_tombstoned(conversation_id) {
            return Vec::new();
        }
        self.historical_episodes_for_conversation(conversation_id)
    }

    pub fn historical_episodes(&self) -> Vec<Episode> {
        self.episodes.iter().cloned().collect()
    }

    pub fn historical_episodes_for_conversation(&self, conversation_id: &str) -> Vec<Episode> {
        self.episodes
            .for_conversation(conversation_id)
            .into_iter()
            .cloned()
            .collect()
    }

    pub fn episode(&self, id: EpisodeId) -> Option<&Episode> {
        self.episodes
            .get(id)
            .filter(|episode| !self.is_conversation_tombstoned(&episode.conversation_id))
    }

    pub fn historical_episode(&self, id: EpisodeId) -> Option<&Episode> {
        self.episodes.get(id)
    }

    pub(crate) fn episode_turns(
        &self,
        container: &mut Container,
        id: EpisodeId,
    ) -> Result<Vec<crate::ResolvedTurn>, ArchiveError> {
        let episode = self
            .episodes
            .get(id)
            .ok_or(ArchiveError::InvalidEpisodeRange)?;
        self.require_current_conversation_id(&episode.conversation_id)?;
        let nodes = self.branch_nodes(&episode.conversation_id, &episode.end_node_id)?;
        let start = nodes
            .iter()
            .position(|node| node.id == episode.start_node_id)
            .ok_or(ArchiveError::InvalidEpisodeRange)?;
        let mut turns = Vec::with_capacity(nodes.len() - start);
        for node in &nodes[start..] {
            turns.push(crate::ResolvedTurn {
                node_id: node.id.clone(),
                role: node.role.clone(),
                principal_id: node.principal_id.clone(),
                timestamp_ns: node.timestamp_ns,
                content: self.content(container, node.content_id)?,
            });
        }
        Ok(turns)
    }

    pub(crate) fn episode_contains_node(
        &self,
        episode_id: EpisodeId,
        node_id: &str,
    ) -> Result<bool, ArchiveError> {
        let episode = self
            .episodes
            .get(episode_id)
            .ok_or(ArchiveError::InvalidEpisodeRange)?;
        let nodes = self.branch_nodes(&episode.conversation_id, &episode.end_node_id)?;
        let start = nodes
            .iter()
            .position(|node| node.id == episode.start_node_id)
            .ok_or(ArchiveError::InvalidEpisodeRange)?;
        Ok(nodes[start..].iter().any(|node| node.id == node_id))
    }

    pub(crate) fn episode_contains_all_nodes(
        &self,
        episode_id: EpisodeId,
        node_ids: &HashSet<&str>,
    ) -> Result<bool, ArchiveError> {
        if node_ids.is_empty() {
            return Ok(true);
        }
        let episode = self
            .episodes
            .get(episode_id)
            .ok_or(ArchiveError::InvalidEpisodeRange)?;
        let mut remaining = node_ids.clone();
        let mut seen = HashSet::new();
        let mut current = Some(episode.end_node_id.as_str());

        while let Some(id) = current {
            if !seen.insert(id) {
                return Err(ArchiveError::NodeCycle);
            }
            remaining.remove(id);
            if id == episode.start_node_id.as_str() {
                return Ok(remaining.is_empty());
            }
            let node =
                self.require_node(&episode.conversation_id, id, ArchiveError::MissingNode)?;
            current = node.parent_id.as_deref();
        }
        Err(ArchiveError::InvalidEpisodeRange)
    }

    pub(crate) fn validate_episodes(&self) -> Result<(), ArchiveError> {
        for episode in self.episodes.iter() {
            self.validate_episode(episode)?;
        }
        Ok(())
    }

    fn validate_episode(&self, episode: &Episode) -> Result<(), ArchiveError> {
        if episode.conversation_id.is_empty()
            || episode.start_node_id.is_empty()
            || episode.end_node_id.is_empty()
            || episode.finalized_at_ns < episode.source_through_ns
        {
            return Err(ArchiveError::InvalidEpisodeRange);
        }
        let start = self.require_node(
            &episode.conversation_id,
            &episode.start_node_id,
            ArchiveError::MissingEpisodeNode,
        )?;
        let end = self.require_node(
            &episode.conversation_id,
            &episode.end_node_id,
            ArchiveError::MissingEpisodeNode,
        )?;
        if end.timestamp_ns != episode.source_through_ns {
            return Err(ArchiveError::InvalidEpisodeRange);
        }
        if !self.node_descends_from(end, start)? {
            return Err(ArchiveError::InvalidEpisodeRange);
        }
        Ok(())
    }

    fn node_descends_from(&self, end: &Node, start: &Node) -> Result<bool, ArchiveError> {
        let mut seen = HashSet::new();
        let mut current = Some(end.id.as_str());
        while let Some(id) = current {
            if !seen.insert(id) {
                return Err(ArchiveError::NodeCycle);
            }
            if id == start.id.as_str() {
                return Ok(true);
            }
            let node = self.require_node(&end.conversation_id, id, ArchiveError::MissingNode)?;
            current = node.parent_id.as_deref();
        }
        Ok(false)
    }
}
