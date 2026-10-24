use crate::dream_duplicate_index::DuplicateIndex;
use crate::dream_owner_publisher::{DreamPublicationPlan, finish_dream_pair, prepare_dream_pair};
use crate::owner_lifecycle::DreamGraphPublication;
use crate::{
    DreamPairClassification, DreamPairVerification, DreamPublicationError, DreamPublicationOutcome,
    DreamVerificationPolicy, Memory,
};

/// Upper composition sequences semantic preparation and the one owner writer.
pub(crate) fn publish_dream_pair<R>(
    mut owner: DreamGraphPublication<'_>,
    duplicate_index: &mut DuplicateIndex,
    classification: &DreamPairClassification,
    verification: Option<&DreamPairVerification>,
    policy: DreamVerificationPolicy,
    expected_graph_version: u64,
    source_time: &R,
) -> Result<DreamPublicationOutcome, DreamPublicationError>
where
    R: Fn(&Memory) -> Option<i64>,
{
    match prepare_dream_pair(
        &owner.memories(),
        owner.graph(),
        duplicate_index,
        classification,
        verification,
        policy,
        expected_graph_version,
        source_time,
    )? {
        DreamPublicationPlan::Complete(outcome) => Ok(outcome),
        DreamPublicationPlan::Changes { changes, duplicate } => {
            let published = owner.publish(&changes, expected_graph_version)?;
            finish_dream_pair(
                &owner.memories(),
                owner.graph(),
                duplicate_index,
                duplicate,
                published,
                source_time,
            )
        }
    }
}
