use crate::{EntityCandidateSet, EntityResolutionOutcome, Memory, MemoryEntityMentionKey};

pub(crate) enum EntityResolutionPreparation {
    Complete(EntityResolutionOutcome),
    Ready(EntityResolutionPrepared),
}

pub(crate) struct EntityResolutionPrepared {
    pub(crate) owner_id: String,
    pub(crate) key: MemoryEntityMentionKey,
    pub(crate) memory: Memory,
    pub(crate) candidates: EntityCandidateSet,
    pub(crate) evidence: Vec<Vec<Memory>>,
    pub(crate) admission_context: Vec<Memory>,
    pub(crate) candidate_fingerprint: [u8; 32],
    pub(crate) context_fingerprint: [u8; 32],
    pub(crate) expected_resolution_revision: u32,
    pub(crate) memory_version: u64,
    pub(crate) entity_version: u64,
    pub(crate) graph_version: u64,
}

impl EntityResolutionPrepared {
    pub(crate) fn evaluation_input(
        &self,
    ) -> reliquary_entity_resolution::EntityResolutionPrepared<'_> {
        reliquary_entity_resolution::EntityResolutionPrepared {
            memory: &self.memory,
            candidates: &self.candidates,
            evidence: &self.evidence,
            admission_context: &self.admission_context,
        }
    }
}
