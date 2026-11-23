use super::*;
use crate::session::grouped_completion::StoredGroupedCompletion;

pub(in crate::session) fn validate_grouped_memory_cut(
    completions: &BTreeMap<(WorkEpisodeId, u32), StoredGroupedCompletion>,
    memory_version: u64,
) -> Result<(), HistoricalProofError> {
    for completion in completions.values() {
        if completion
            .memory_versions
            .is_some_and(|(first, last)| first <= memory_version && memory_version < last)
        {
            return Err(HistoricalProofError::InconsistentCut(
                "Memory cut splits an atomic grouped publication",
            ));
        }
    }
    Ok(())
}
