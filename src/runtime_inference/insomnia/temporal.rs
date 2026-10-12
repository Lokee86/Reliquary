use crate::insomnia::processor::PreparedApplication;
use crate::{
    GeneralEndpoint, InsomniaExtractionError, MemoryDraft, TemporalAssessment, TemporalInferencer,
};

pub(crate) fn assess_draft(draft: &MemoryDraft) -> TemporalAssessment {
    crate::chronos::assess(&semantic_text(draft), draft.source_time_ns)
}

pub(crate) fn infer_prepared(
    endpoint: &dyn GeneralEndpoint,
    prepared: &mut PreparedApplication,
) -> Result<usize, InsomniaExtractionError> {
    let inferencer = TemporalInferencer::new(endpoint);
    let mut inferred = 0;
    for memory in &mut prepared.project_drafts {
        inferred += infer_memory(&inferencer, memory)?;
    }
    for user in &mut prepared.user_drafts {
        inferred += infer_memory(&inferencer, &mut user.memory)?;
    }
    Ok(inferred)
}

pub(crate) fn infer_memory<E: GeneralEndpoint>(
    inferencer: &TemporalInferencer<E>,
    prepared: &mut crate::insomnia::processor::PreparedMemory,
) -> Result<usize, InsomniaExtractionError> {
    if !prepared.temporal.resolution.needs_inference() {
        return Ok(0);
    }
    prepared.temporal_inference = inferencer
        .infer(
            &semantic_text(&prepared.draft),
            prepared.draft.source_time_ns,
            &prepared.temporal,
        )
        .map_err(InsomniaExtractionError::from)?;
    Ok(usize::from(prepared.temporal_inference.is_some()))
}

fn semantic_text(draft: &MemoryDraft) -> String {
    format!("{}\n{}", draft.title, draft.content)
}
