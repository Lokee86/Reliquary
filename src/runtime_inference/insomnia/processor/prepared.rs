use crate::{
    InsomniaRejection, Memory, MemoryDraft, MemoryRef, MemoryRoutingMetadata, MemorySourceRef,
    TemporalAssessment,
};

pub(crate) struct PreparedApplication {
    pub(crate) project_drafts: Vec<PreparedMemory>,
    pub(crate) user_drafts: Vec<PreparedUserMemory>,
    pub(crate) rejected: Vec<InsomniaRejection>,
    pub(crate) model: String,
}

pub(crate) struct PreparedMemory {
    pub(crate) draft: MemoryDraft,
    pub(crate) temporal: TemporalAssessment,
    pub(crate) temporal_inference: Option<crate::TemporalInference>,
    pub(crate) routing_metadata: Option<MemoryRoutingMetadata>,
}

pub(crate) struct PreparedUserMemory {
    pub(crate) memory: PreparedMemory,
    pub(crate) source_ref: MemorySourceRef,
}

pub(crate) struct UserPublication {
    pub(crate) created: Vec<Memory>,
    pub(crate) existing: Vec<Memory>,
    pub(crate) refs: Vec<MemoryRef>,
}

impl PreparedMemory {
    pub(crate) fn bound_temporal_inference(&self) -> Option<crate::MemoryTemporalInference> {
        self.temporal_inference
            .clone()
            .map(|inference| crate::MemoryTemporalInference {
                body_id: crate::memory_model::memory_body_id(
                    &self.draft.title,
                    &self.draft.content,
                ),
                source_time_ns: self.draft.source_time_ns,
                inference: crate::memory_temporal_conversion::to_memory_inference(inference),
            })
    }
}
