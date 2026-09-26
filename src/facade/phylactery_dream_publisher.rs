use crate::dream_owner_publisher::publish_dream_pair as publish_dream_pair_from_parts;
use crate::memory_source_time::memory_source_timestamp_ns;
use crate::{
    DreamPairClassification, DreamPairVerification, DreamPublicationError, DreamPublicationOutcome,
    DreamVerificationPolicy, Memory, Phylactery,
};

impl Phylactery {
    pub fn publish_dream_pair(
        &mut self,
        classification: &DreamPairClassification,
        verification: Option<&DreamPairVerification>,
        policy: DreamVerificationPolicy,
        expected_graph_version: u64,
    ) -> Result<DreamPublicationOutcome, DreamPublicationError> {
        let source_time = |memory: &Memory| memory_source_timestamp_ns(memory);
        publish_dream_pair_from_parts(
            &mut self.container,
            &self.memories,
            &mut self.graph,
            &mut self.duplicate_index,
            classification,
            verification,
            policy,
            expected_graph_version,
            &source_time,
        )
    }
}
