//! Read-only accepted publication birth-cohort evidence.
use crate::MemoryId;
use crate::owner_lifecycle::{HistorySource, OwnerError};
use std::collections::BTreeSet;

pub(crate) fn publication_birth_cohort(
    source: &HistorySource<'_>,
    member: MemoryId,
) -> Result<BTreeSet<MemoryId>, OwnerError> {
    let mut cohort = BTreeSet::from([member]);
    let mut cursor = source
        .cursor()
        .map_err(|error| OwnerError::Container(error.0))?;
    while let Some(frame) = cursor
        .next_frame()
        .map_err(|error| OwnerError::Container(error.0))?
    {
        let payload = frame.payload.as_slice();
        if let Some(publication) = crate::memory_external_publication::decode(payload)?
            && publication.birth_turn.is_some()
            && publication.records.iter().any(|record| record.id == member)
        {
            cohort.extend(publication.records.iter().map(|record| record.id));
        }
        if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
            .map_err(|message| OwnerError::Freshness(message.into()))?
            && completion.freshness_birth_turn.is_some()
            && completion.freshness_birth_memory_ids.contains(&member)
        {
            cohort.extend(completion.freshness_birth_memory_ids);
        }
    }
    Ok(cohort)
}

pub(crate) fn same_publication_birth_cohort(
    source: &HistorySource<'_>,
    a: MemoryId,
    b: MemoryId,
) -> Result<bool, OwnerError> {
    let mut same = false;
    let mut cursor = source
        .cursor()
        .map_err(|error| OwnerError::Container(error.0))?;
    while let Some(frame) = cursor
        .next_frame()
        .map_err(|error| OwnerError::Container(error.0))?
    {
        let payload = frame.payload.as_slice();
        if let Some(publication) = crate::memory_external_publication::decode(payload)? {
            same |= publication.birth_turn.is_some()
                && publication.records.iter().any(|record| record.id == a)
                && publication.records.iter().any(|record| record.id == b);
        }
        if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
            .map_err(|message| OwnerError::Freshness(message.into()))?
        {
            same |= completion.freshness_birth_turn.is_some()
                && completion.freshness_birth_memory_ids.contains(&a)
                && completion.freshness_birth_memory_ids.contains(&b);
        }
    }
    Ok(same)
}
