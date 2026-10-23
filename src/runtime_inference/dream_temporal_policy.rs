use crate::chronos::{analyze, analyze_with_inference};
use crate::{DreamTemporalAnalysis, Memory};

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
