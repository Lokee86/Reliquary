use crate::ConversationCompactionError;
use crate::conversation_compaction_codec::free_prefix;
use crate::conversation_compaction_store::{ConversationCompactionStore, StoredCompactionChunk};
use reliquary_container::{Container, ObjectRef};

const MIN_ALLOC_PAYLOAD: u64 = 256;
const ALLOC_GRANULARITY: u64 = 256;
const CHUNK_HEADER_LEN: u64 = 8;

impl ConversationCompactionStore {
    pub(crate) fn allocate(
        &mut self,
        container: &mut Container,
        required: u64,
    ) -> Result<ObjectRef, ConversationCompactionError> {
        let desired = round_up(required.max(MIN_ALLOC_PAYLOAD), ALLOC_GRANULARITY)?;
        let mut best = None;
        for stored in self.chunks.values() {
            let StoredCompactionChunk::Free(object) = stored else {
                continue;
            };
            let capacity = container.object_capacity(*object);
            if capacity >= desired && best.is_none_or(|(_, best_capacity)| capacity < best_capacity)
            {
                best = Some((*object, capacity));
            }
        }

        let Some((free, capacity)) = best else {
            let payload_len = usize::try_from(desired)
                .map_err(|_| ConversationCompactionError::RecordTooLarge)?;
            return Ok(container.append(&vec![0_u8; payload_len])?);
        };

        self.chunks.remove(&free);
        let remainder = capacity
            .saturating_sub(desired)
            .saturating_sub(CHUNK_HEADER_LEN);
        if remainder >= MIN_ALLOC_PAYLOAD {
            let (allocated, leftover) = container.split_chunk(free, desired, &free_prefix())?;
            self.chunks
                .insert(leftover, StoredCompactionChunk::Free(leftover));
            Ok(allocated)
        } else {
            Ok(free)
        }
    }

    pub(crate) fn free_and_coalesce(
        &mut self,
        container: &mut Container,
        object: ObjectRef,
    ) -> Result<(), ConversationCompactionError> {
        let Some(old) = self.chunks.remove(&object) else {
            return Ok(());
        };
        let object = old.chunk();
        container.write_chunk_prefix(object, &free_prefix())?;
        self.chunks
            .insert(object, StoredCompactionChunk::Free(object));
        self.coalesce_at(container, object)
    }

    fn coalesce_at(
        &mut self,
        container: &mut Container,
        mut object: ObjectRef,
    ) -> Result<(), ConversationCompactionError> {
        loop {
            let current = self
                .chunks
                .get(&object)
                .map(StoredCompactionChunk::chunk)
                .unwrap();
            if let Some(left) = self.left_free(container, current) {
                self.chunks.remove(&left);
                self.chunks.remove(&current);
                let merged = container.merge_adjacent_chunks(left, current)?;
                container.write_chunk_prefix(merged, &free_prefix())?;
                object = merged;
                self.chunks
                    .insert(merged, StoredCompactionChunk::Free(merged));
                continue;
            }
            if let Some(right) = self.right_free(container, current) {
                self.chunks.remove(&current);
                self.chunks.remove(&right);
                let merged = container.merge_adjacent_chunks(current, right)?;
                container.write_chunk_prefix(merged, &free_prefix())?;
                object = merged;
                self.chunks
                    .insert(merged, StoredCompactionChunk::Free(merged));
                continue;
            }
            if container.truncate_tail_chunk(current)? {
                self.chunks.remove(&object);
            }
            return Ok(());
        }
    }

    fn left_free(&self, container: &Container, current: ObjectRef) -> Option<ObjectRef> {
        self.chunks.values().find_map(|stored| match stored {
            StoredCompactionChunk::Free(candidate)
                if container.objects_adjacent(*candidate, current) =>
            {
                Some(*candidate)
            }
            _ => None,
        })
    }

    fn right_free(&self, container: &Container, current: ObjectRef) -> Option<ObjectRef> {
        self.chunks.values().find_map(|stored| match stored {
            StoredCompactionChunk::Free(candidate)
                if container.objects_adjacent(current, *candidate) =>
            {
                Some(*candidate)
            }
            _ => None,
        })
    }
}

fn round_up(value: u64, granularity: u64) -> Result<u64, ConversationCompactionError> {
    value
        .checked_add(granularity - 1)
        .map(|value| value / granularity * granularity)
        .ok_or(ConversationCompactionError::RecordTooLarge)
}
