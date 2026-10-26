use crate::{ArchiveError, Episode, EpisodeId};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct EpisodeIndex {
    records: Vec<Episode>,
    by_id: HashMap<EpisodeId, usize>,
    by_conversation: HashMap<String, Vec<usize>>,
}

impl EpisodeIndex {
    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }

    pub(crate) fn get(&self, id: EpisodeId) -> Option<&Episode> {
        self.by_id.get(&id).map(|index| &self.records[*index])
    }

    pub(crate) fn insert(&mut self, episode: Episode) -> Result<bool, ArchiveError> {
        if let Some(existing) = self.get(episode.id) {
            return if existing == &episode {
                Ok(false)
            } else {
                Err(ArchiveError::ConflictingEpisode)
            };
        }
        let index = self.records.len();
        self.by_id.insert(episode.id, index);
        self.by_conversation
            .entry(episode.conversation_id.clone())
            .or_default()
            .push(index);
        self.records.push(episode);
        Ok(true)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Episode> {
        self.records.iter()
    }

    pub(crate) fn for_conversation(&self, conversation_id: &str) -> Vec<&Episode> {
        self.by_conversation
            .get(conversation_id)
            .into_iter()
            .flatten()
            .map(|index| &self.records[*index])
            .collect()
    }
}
