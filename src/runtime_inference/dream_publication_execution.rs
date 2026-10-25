use crate::dream_duplicate_index::DuplicateIndex;
use crate::dream_owner_duplicate::{DuplicatePairProgress, DuplicatePlan};
use crate::dream_owner_publisher::{
    DreamPublicationFinish, DreamPublicationPlan, finish_dream_pair, prepare_dream_pair,
};
use crate::owner_lifecycle::DreamGraphPublication;
use crate::{
    DreamPairClassification, DreamPairVerification, DreamPublicationError, DreamPublicationOutcome,
    DreamVerificationPolicy, Memory,
};

pub(crate) enum DreamSourceTime<'a> {
    Reliquary(&'a crate::Archive),
    Phylactery,
}

impl DreamSourceTime<'_> {
    fn for_memory(&self, memory: &Memory) -> Option<i64> {
        match self {
            Self::Reliquary(archive) => {
                crate::memory_source_time_admission::reliquary_memory_source_timestamp_ns(
                    archive, memory,
                )
            }
            Self::Phylactery => crate::memory_source_time::memory_source_timestamp_ns(memory),
        }
    }
}

/// Upper composition sequences semantic preparation and the one owner writer.
pub(crate) fn publish_dream_pair(
    mut owner: DreamGraphPublication<'_>,
    duplicate_index: &mut DuplicateIndex,
    classification: &DreamPairClassification,
    verification: Option<&DreamPairVerification>,
    policy: DreamVerificationPolicy,
    expected_graph_version: u64,
    source_time: DreamSourceTime<'_>,
) -> Result<DreamPublicationOutcome, DreamPublicationError> {
    match prepare_dream_pair(
        &owner.memories(),
        owner.graph(),
        duplicate_index,
        classification,
        verification,
        policy,
        expected_graph_version,
    )? {
        DreamPublicationPlan::Complete(outcome) => Ok(outcome),
        DreamPublicationPlan::Duplicate(mut sequence) => loop {
            match sequence.advance(&owner.memories(), owner.graph(), duplicate_index)? {
                DuplicatePairProgress::SourceTime(memory) => {
                    let timestamp = source_time.for_memory(&memory);
                    sequence.resume(memory.id, timestamp)?;
                }
                DuplicatePairProgress::Ready(DuplicatePlan::NoChange) => {
                    return Ok(DreamPublicationOutcome::NoChange);
                }
                DuplicatePairProgress::Ready(DuplicatePlan::Changes {
                    changes,
                    publication,
                }) => {
                    let published = owner.publish(&changes, expected_graph_version)?;
                    match finish_dream_pair(
                        owner.graph(),
                        duplicate_index,
                        Some(publication),
                        published,
                    )? {
                        DreamPublicationFinish::Complete(outcome) => return Ok(outcome),
                        DreamPublicationFinish::Rebuild {
                            mut sequence,
                            outcome,
                        } => loop {
                            let memories = owner.memories();
                            match sequence.advance(&memories, duplicate_index)? {
                                Some(memory) => {
                                    sequence.resume(memory.id, source_time.for_memory(&memory))?;
                                }
                                None => return Ok(outcome),
                            }
                        },
                    }
                }
            }
        },
        DreamPublicationPlan::Changes { changes, duplicate } => {
            let published = owner.publish(&changes, expected_graph_version)?;
            match finish_dream_pair(owner.graph(), duplicate_index, duplicate, published)? {
                DreamPublicationFinish::Complete(outcome) => Ok(outcome),
                DreamPublicationFinish::Rebuild {
                    mut sequence,
                    outcome,
                } => loop {
                    let memories = owner.memories();
                    match sequence.advance(&memories, duplicate_index)? {
                        Some(memory) => {
                            sequence.resume(memory.id, source_time.for_memory(&memory))?;
                        }
                        None => return Ok(outcome),
                    }
                },
            }
        }
    }
}
