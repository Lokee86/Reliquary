use super::*;
use crate::memory_store::MemoryStore;

impl LexicalIndex {
    pub(crate) fn ensure_memories_current(
        &mut self,
        memories: &MemoryStore,
        container: &mut Container,
    ) -> Result<(), MemoryError> {
        let version = memories.memory_version();
        if self.memory_version == version {
            return Ok(());
        }
        if self.memory_version > version {
            self.reset_memory_index();
        }

        let mut changed = HashMap::new();
        for record in memories
            .records()
            .iter()
            .filter(|record| record.memory_version > self.memory_version)
        {
            changed.insert(
                record.id,
                record.archived || !memories.contains_memory(record.id),
            );
        }

        for id in self.memory_slots.keys().copied() {
            if !memories.contains_memory(id) {
                changed.insert(id, true);
            }
        }
        let mut changed = changed.into_iter().collect::<Vec<_>>();
        changed.sort_by_key(|(id, _)| id.0);
        for (id, archived) in changed {
            if let Some(slot) = self.memory_slots.get(&id).copied() {
                self.memory_active[slot] = !archived;
                continue;
            }
            if archived {
                continue;
            }
            let memory = memories.memory(container, id)?;
            self.index_memory(id, &memory.title, &memory.content);
        }
        self.memory_version = version;
        Ok(())
    }

    fn index_memory(&mut self, id: MemoryId, title: &str, content: &str) {
        let slot = self.memories.len();
        for (term, count) in term_counts(&format!("{title}\n\n{content}")) {
            self.memory_postings
                .entry(term)
                .or_default()
                .push(MemoryPosting {
                    memory_slot: slot,
                    frequency: bounded_frequency(count),
                });
        }
        self.memory_slots.insert(id, slot);
        self.memories.push(id);
        self.memory_active.push(true);
    }

    fn reset_memory_index(&mut self) {
        self.memory_version = 0;
        self.memories.clear();
        self.memory_active.clear();
        self.memory_slots.clear();
        self.memory_postings.clear();
    }

    #[cfg(test)]
    pub(crate) fn memory_indexed_slots(&self) -> usize {
        self.memories.len()
    }

    #[cfg(test)]
    pub(crate) fn indexed_memory_version(&self) -> u64 {
        self.memory_version
    }

    pub(crate) fn search_memories(&self, terms: &[String], limit: usize) -> Vec<MemoryLexicalHit> {
        if limit == 0 {
            return Vec::new();
        }
        let mut hits = self.complete_memory_matches(terms);
        hits.truncate(limit);
        hits
    }

    pub(crate) fn complete_memory_matches(&self, terms: &[String]) -> Vec<MemoryLexicalHit> {
        if terms.is_empty() {
            return Vec::new();
        }
        let mut scores: HashMap<usize, ScoreParts> = HashMap::new();
        for term in terms {
            let Some(postings) = self.memory_postings.get(term) else {
                continue;
            };
            for posting in postings {
                if !self.memory_active[posting.memory_slot] {
                    continue;
                }
                add_score(&mut scores, posting.memory_slot, posting.frequency);
            }
        }
        let mut hits = scores
            .into_iter()
            .map(|(slot, parts)| MemoryLexicalHit {
                memory_id: self.memories[slot],
                score: score(parts, terms.len()),
            })
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.memory_id.0.cmp(&right.memory_id.0))
        });
        hits
    }
}
