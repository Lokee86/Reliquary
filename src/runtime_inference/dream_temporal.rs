use crate::dream_temporal_policy::analyze_memory_temporal;
use crate::memory_source_time_admission::reliquary_memory_source_timestamp_ns;
use crate::{Cva, DreamTemporalAnalysis, Memory, MemoryError, MemoryId};

impl Cva {
    pub fn dream_temporal_analysis(
        &mut self,
        memory_id: MemoryId,
    ) -> Result<DreamTemporalAnalysis, MemoryError> {
        let memory = self.memories.memory(&mut self.container, memory_id)?;
        let body_id = self.memories.current_body_id(memory_id)?;
        let source_timestamp_ns = reliquary_memory_source_timestamp_ns(&self.archive, &memory);
        Ok(analyze_memory_temporal(
            &memory,
            body_id,
            source_timestamp_ns,
        ))
    }
}
