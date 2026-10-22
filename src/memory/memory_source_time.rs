use crate::Memory;

pub(crate) fn memory_source_timestamp_ns(memory: &Memory) -> Option<i64> {
    memory.source_time_ns
}
