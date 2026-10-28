use super::*;
use crate::{MemoryError, memory_store::MemoryStore};
use std::collections::HashMap;

impl LexicalIndex {
    pub(crate) fn ensure_memories_current(
        &mut self,
        memories: &MemoryStore,
        container: &mut Container,
    ) -> Result<(), MemoryError> {
        let version = memories.memory_version();
        if self.engine.memory_version() == version {
            return Ok(());
        }
        if self.engine.memory_version() > version {
            self.engine.reset_memory_index();
        }
        let mut changed = HashMap::new();
        for record in memories
            .records()
            .iter()
            .filter(|record| record.memory_version > self.engine.memory_version())
        {
            changed.insert(
                record.id,
                record.archived || !memories.contains_memory(record.id),
            );
        }
        for id in self.engine.memory_ids() {
            if !memories.contains_memory(id) {
                changed.insert(id, true);
            }
        }
        let mut changed = changed.into_iter().collect::<Vec<_>>();
        changed.sort_by_key(|(id, _)| id.0);
        for (id, archived) in changed {
            if let Some(active) = self.engine.has_memory(id).then_some(!archived) {
                self.engine.set_memory_active(id, active);
                continue;
            }
            if archived {
                continue;
            }
            let memory = memories.memory(container, id)?;
            self.engine.add_memory(id, &memory.title, &memory.content);
        }
        self.engine.set_memory_version(version);
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn memory_indexed_slots(&self) -> usize {
        self.engine.memory_indexed_slots()
    }
    #[cfg(test)]
    pub(crate) fn indexed_memory_version(&self) -> u64 {
        self.engine.memory_version()
    }
    pub(crate) fn search_memories(&self, terms: &[String], limit: usize) -> Vec<MemoryLexicalHit> {
        self.engine.search_memories(terms, limit)
    }
    pub(crate) fn complete_memory_matches(&self, terms: &[String]) -> Vec<MemoryLexicalHit> {
        self.engine.complete_memory_matches(terms)
    }
}
