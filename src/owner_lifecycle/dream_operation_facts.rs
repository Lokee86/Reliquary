//! Borrowed Dream policy inputs and bounded native journal publication.
use super::{PhylacteryOwner, ReliquaryOwner};
use crate::dream_pair_history::DreamPairStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryReadView;
use crate::memory_vector_store::MemoryVectorReadView;
use crate::{MemoryError, MemoryId};

pub(crate) struct DreamOwnerFacts<'a> {
    pub(crate) memories: MemoryReadView<'a>,
    pub(crate) graph: &'a GraphStore,
    pub(crate) pairs: &'a DreamPairStore,
    pub(crate) vectors: MemoryVectorReadView<'a>,
}
macro_rules! dream_facts {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn dream_operation_facts(&self) -> DreamOwnerFacts<'_> {
                DreamOwnerFacts {
                    memories: MemoryReadView::new(&self.memories, &self.container),
                    graph: &self.graph,
                    pairs: &self.dream_pairs,
                    vectors: MemoryVectorReadView::new(
                        &self.memory_vectors,
                        &self.packed_vectors,
                        &self.container,
                    ),
                }
            }
            pub(crate) fn mark_dream_pair_native(
                &mut self,
                left: MemoryId,
                right: MemoryId,
            ) -> Result<bool, MemoryError> {
                if !self.memories.contains_memory(left) || !self.memories.contains_memory(right) {
                    return Err(MemoryError::MissingMemory);
                }
                self.dream_pairs.put(&mut self.container, left, right)
            }
            pub(crate) fn dream_pair_records_native(&self) -> Vec<(MemoryId, MemoryId)> {
                self.dream_pairs.records()
            }
            pub(crate) fn dream_processing_state_native(
                &self,
                id: MemoryId,
            ) -> Option<crate::processing_epoch_model::ProcessingEpochState> {
                self.processing_epochs
                    .state(id, crate::processing_epoch_model::DREAM_PROCESSING_LANE)
            }
            pub(crate) fn mark_dream_processed_native(
                &mut self,
                id: MemoryId,
                state: crate::processing_epoch_model::ProcessingEpochState,
            ) -> Result<bool, MemoryError> {
                if !self.memories.contains_memory(id) {
                    return Err(MemoryError::MissingMemory);
                }
                self.processing_epochs.put(
                    &mut self.container,
                    id,
                    crate::processing_epoch_model::DREAM_PROCESSING_LANE,
                    state,
                )
            }
        }
    };
}
dream_facts!(ReliquaryOwner);
dream_facts!(PhylacteryOwner);
