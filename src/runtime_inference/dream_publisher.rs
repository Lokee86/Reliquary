use crate::dream_owner_publisher::publish_dream_pair as publish_dream_pair_from_parts;
use crate::memory_source_time::reliquary_memory_source_timestamp_ns;
use crate::{
    Cva, DreamPairClassification, DreamPairVerification, DreamPublicationError,
    DreamPublicationOutcome, DreamVerificationPolicy, Memory,
};

impl Cva {
    pub fn publish_dream_pair(
        &mut self,
        classification: &DreamPairClassification,
        verification: Option<&DreamPairVerification>,
        policy: DreamVerificationPolicy,
        expected_graph_version: u64,
    ) -> Result<DreamPublicationOutcome, DreamPublicationError> {
        let archive = &self.archive;
        let source_time = |memory: &Memory| reliquary_memory_source_timestamp_ns(archive, memory);
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
