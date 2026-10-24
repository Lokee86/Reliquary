use crate::{Memory, MemoryDraft, MemoryError, MemoryId, MemorySourceRef, Phylactery};

impl Phylactery {
    #[cfg(test)]
    pub(crate) fn publish_memory_with_source_ref(
        &mut self,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
        source_ref: MemorySourceRef,
    ) -> Result<(Memory, bool), MemoryError> {
        self.native_publish_memory_with_source_ref(id, expected_revision, draft, source_ref)
    }

    #[cfg(test)]
    pub(crate) fn publish_memory_with_source_ref_and_temporal_inference(
        &mut self,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
        source_ref: MemorySourceRef,
        temporal_inference: Option<crate::MemoryTemporalInference>,
    ) -> Result<(Memory, bool), MemoryError> {
        self.native_publish_memory_with_source_ref_and_temporal_inference(
            id,
            expected_revision,
            draft,
            source_ref,
            temporal_inference,
        )
    }

    pub fn backfill_memory_source_ref(
        &mut self,
        id: MemoryId,
        source_ref: MemorySourceRef,
        updated_at_ns: i64,
    ) -> Result<(Memory, bool), MemoryError> {
        self.native_backfill_memory_source_ref(id, source_ref, updated_at_ns)
    }
}
