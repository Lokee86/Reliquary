use crate::conversation_compaction_codec::{DecodedCompactionChunk, decode, encode_live};
use crate::{Container, ConversationCompaction, ConversationCompactionError, ObjectRef};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(crate) enum StoredCompactionChunk {
    Free(ObjectRef),
    Live {
        chunk: ObjectRef,
        record: ConversationCompaction,
        supersedes: Option<String>,
    },
}

impl StoredCompactionChunk {
    pub(crate) fn chunk(&self) -> ObjectRef {
        match self {
            Self::Free(chunk) | Self::Live { chunk, .. } => *chunk,
        }
    }

    pub(crate) fn is_free(&self) -> bool {
        matches!(self, Self::Free(_))
    }
}

#[derive(Default)]
pub(crate) struct ConversationCompactionOpenState {
    chunks: BTreeMap<ObjectRef, StoredCompactionChunk>,
    next_generation: u64,
}

impl ConversationCompactionOpenState {
    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
    ) -> Result<(), ConversationCompactionError> {
        let Some(decoded) = decode(payload)? else {
            return Ok(());
        };
        let stored = match decoded {
            DecodedCompactionChunk::Free => StoredCompactionChunk::Free(chunk),
            DecodedCompactionChunk::Live {
                record,
                supersedes_through_message_id,
            } => {
                self.next_generation = self
                    .next_generation
                    .max(record.generation.saturating_add(1));
                StoredCompactionChunk::Live {
                    chunk,
                    record,
                    supersedes: supersedes_through_message_id,
                }
            }
        };
        self.chunks.insert(chunk, stored);
        Ok(())
    }

    pub(crate) fn finish(
        self,
        container: &mut Container,
    ) -> Result<ConversationCompactionStore, ConversationCompactionError> {
        let mut store = ConversationCompactionStore {
            chunks: self.chunks,
            next_generation: self.next_generation.max(1),
        };
        store.recover(container)?;
        Ok(store)
    }

    /// Builds the same visible compaction projection without reclaiming or rewriting chunks.
    pub(crate) fn finish_without_reclaim(
        self,
    ) -> Result<ConversationCompactionStore, ConversationCompactionError> {
        let mut store = ConversationCompactionStore {
            chunks: self.chunks,
            next_generation: self.next_generation.max(1),
        };
        store.remove_stale_rows();
        Ok(store)
    }
}

#[derive(Default)]
pub(crate) struct ConversationCompactionStore {
    pub(crate) chunks: BTreeMap<ObjectRef, StoredCompactionChunk>,
    pub(crate) next_generation: u64,
}

impl ConversationCompactionStore {
    pub(crate) fn empty() -> Self {
        Self {
            chunks: BTreeMap::new(),
            next_generation: 1,
        }
    }

    pub(crate) fn all_records(&self) -> Vec<ConversationCompaction> {
        let mut records = self
            .chunks
            .values()
            .filter_map(|chunk| match chunk {
                StoredCompactionChunk::Live { record, .. } => Some(record.clone()),
                StoredCompactionChunk::Free(_) => None,
            })
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.generation);
        records
    }

    pub(crate) fn for_conversation(&self, conversation_id: &str) -> Vec<ConversationCompaction> {
        let mut records = self
            .chunks
            .values()
            .filter_map(|chunk| match chunk {
                StoredCompactionChunk::Live { record, .. }
                    if record.conversation_id == conversation_id =>
                {
                    Some(record.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.generation);
        records
    }

    pub(crate) fn put(
        &mut self,
        container: &mut Container,
        conversation_id: String,
        through_message_id: String,
        summary: String,
        supersedes: Option<&str>,
    ) -> Result<ConversationCompaction, ConversationCompactionError> {
        container.require_storage_format_writable()?;
        let generation = self.next_generation;
        self.next_generation = generation
            .checked_add(1)
            .ok_or(ConversationCompactionError::GenerationExhausted)?;
        let record = ConversationCompaction {
            conversation_id,
            through_message_id,
            summary,
            generation,
        };
        let encoded = encode_live(&record, supersedes)?;
        let chunk = self.allocate(container, encoded.len() as u64)?;
        container.write_chunk_prefix(chunk, &encoded)?;
        container.sync()?;
        self.chunks.insert(
            chunk,
            StoredCompactionChunk::Live {
                chunk,
                record: record.clone(),
                supersedes: supersedes.map(str::to_owned),
            },
        );
        if let Some(old_through) = supersedes
            && let Some(old_object) = self.find_record_object(&record.conversation_id, old_through)
            && old_object != chunk
        {
            self.free_and_coalesce(container, old_object)?;
            container.sync()?;
        }
        Ok(record)
    }

    fn recover(&mut self, container: &mut Container) -> Result<(), ConversationCompactionError> {
        container.require_storage_format_writable()?;
        let stale = self.stale_chunks();
        for object in stale {
            if self
                .chunks
                .get(&object)
                .is_some_and(|chunk| !chunk.is_free())
            {
                self.free_and_coalesce(container, object)?;
            }
        }
        if !self.chunks.is_empty() {
            container.sync()?;
        }
        Ok(())
    }

    fn remove_stale_rows(&mut self) {
        for object in self.stale_chunks() {
            if self
                .chunks
                .get(&object)
                .is_some_and(|chunk| !chunk.is_free())
            {
                self.chunks.remove(&object);
            }
        }
    }

    fn stale_chunks(&self) -> BTreeSet<ObjectRef> {
        let live = self
            .chunks
            .iter()
            .filter_map(|(object, chunk)| match chunk {
                StoredCompactionChunk::Live {
                    record, supersedes, ..
                } => Some((*object, record.clone(), supersedes.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut stale = BTreeSet::new();
        for (object, record, supersedes) in &live {
            if let Some(old) = supersedes
                && let Some(old_object) = self.find_record_object(&record.conversation_id, old)
                && old_object != *object
            {
                stale.insert(old_object);
            }
            for (other_object, other, _) in &live {
                if other_object != object
                    && other.conversation_id == record.conversation_id
                    && other.through_message_id == record.through_message_id
                    && other.generation < record.generation
                {
                    stale.insert(*other_object);
                }
            }
        }
        stale
    }

    pub(crate) fn find_record_object(
        &self,
        conversation_id: &str,
        through: &str,
    ) -> Option<ObjectRef> {
        self.chunks.iter().find_map(|(object, chunk)| match chunk {
            StoredCompactionChunk::Live { record, .. }
                if record.conversation_id == conversation_id
                    && record.through_message_id == through =>
            {
                Some(*object)
            }
            _ => None,
        })
    }
}
