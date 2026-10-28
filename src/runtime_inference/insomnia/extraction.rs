pub use reliquary_insomnia::{
    InsomniaCandidate, InsomniaEvidenceResult, InsomniaEvidenceRound, InsomniaEvidenceTurn,
    InsomniaExtraction, InsomniaExtractionError, InsomniaExtractionStage, InsomniaExtractor,
    InsomniaRejection, InsomniaRoutingMetadata,
};

use crate::{Cva, Episode, GeneralEndpoint, InsomniaSemanticStage, ResolvedTurn};

impl From<crate::TemporalInferenceError> for InsomniaExtractionError {
    fn from(value: crate::TemporalInferenceError) -> Self {
        match value {
            crate::TemporalInferenceError::InvalidOutput(message) => {
                Self::InvalidOutput(format!("Chronos temporal inference: {message}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn temporal_inference_error_conversion_preserves_public_message() {
        let error: super::InsomniaExtractionError =
            crate::TemporalInferenceError::InvalidOutput("bad selection".into()).into();
        assert_eq!(
            error.to_string(),
            "invalid Insomnia extraction: Chronos temporal inference: bad selection"
        );
    }
}

pub(crate) fn extract_with_evidence<E: GeneralEndpoint>(
    cva: &mut Cva,
    extractor: &InsomniaExtractor<E>,
    episode: &Episode,
    turns: &[ResolvedTurn],
) -> Result<InsomniaExtraction, InsomniaExtractionError> {
    match extractor.start(episode, turns)? {
        InsomniaExtractionStage::Complete(extraction) => Ok(extraction),
        InsomniaExtractionStage::Evidence(round) => {
            let (results, evidence_turns) =
                super::evidence::resolve_evidence(cva, episode, round.requests())?;
            extractor.finish_evidence(episode, turns, round, results, evidence_turns)
        }
    }
}

pub(crate) fn extract_with_evidence_progress<E: GeneralEndpoint>(
    cva: &mut Cva,
    extractor: &InsomniaExtractor<E>,
    episode: &Episode,
    turns: &[ResolvedTurn],
    stage_reporter: &dyn Fn(InsomniaSemanticStage, usize),
) -> Result<InsomniaExtraction, InsomniaExtractionError> {
    match extractor.start_with_stage_reporter(episode, turns, stage_reporter)? {
        InsomniaExtractionStage::Complete(extraction) => Ok(extraction),
        InsomniaExtractionStage::Evidence(round) => {
            stage_reporter(InsomniaSemanticStage::Evidence, round.requests().len());
            let (results, evidence_turns) =
                super::evidence::resolve_evidence(cva, episode, round.requests())?;
            extractor.finish_evidence_with_stage_reporter(
                episode,
                turns,
                round,
                results,
                evidence_turns,
                stage_reporter,
            )
        }
    }
}
