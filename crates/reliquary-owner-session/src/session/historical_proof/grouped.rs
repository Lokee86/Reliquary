use super::*;

pub(super) fn validate_grouped_memory_cut(
    completions: &BTreeMap<(WorkEpisodeId, u32), GroupedInsomniaCompletion>,
    memory_version: u64,
) -> Result<(), HistoricalProofError> {
    for completion in completions.values() {
        let mut versions = completion
            .memory_records
            .iter()
            .map(|bytes| {
                reliquary_memory_core::decode_grouped_creation(bytes)
                    .map(|record| record.memory_version)
                    .map_err(|error| HistoricalProofError::InvalidState(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        versions.sort_unstable();
        if versions
            .first()
            .is_some_and(|first| *first <= memory_version)
            && versions.last().is_some_and(|last| memory_version < *last)
        {
            return Err(HistoricalProofError::InconsistentCut(
                "Memory cut splits an atomic grouped publication",
            ));
        }
    }
    Ok(())
}
