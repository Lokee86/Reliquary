use crate::chronos::{analyze, analyze_with_inference};
use crate::memory_source_time::reliquary_memory_source_timestamp_ns;
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

pub(crate) fn analyze_memory_temporal(
    memory: &Memory,
    body_id: crate::MemoryBodyId,
    source_timestamp_ns: Option<i64>,
) -> DreamTemporalAnalysis {
    let text = format!("{}\n{}", memory.title, memory.content);
    let deterministic = analyze(&text, source_timestamp_ns);
    let Some(inference) = memory.temporal_inference.as_ref() else {
        return deterministic;
    };
    if inference.body_id != body_id || inference.source_time_ns != source_timestamp_ns {
        return deterministic;
    }
    analyze_with_inference(&text, source_timestamp_ns, &inference.inference)
        .unwrap_or(deterministic)
}
