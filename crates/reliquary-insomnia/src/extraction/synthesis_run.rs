use super::*;

impl<E: GeneralEndpoint> InsomniaExtractor<E> {
    pub(super) fn synthesize(
        &self,
        episode: &crate::input::InsomniaSourceInput,
        turns: &[ResolvedTurn],
        episode_payload: &Value,
        entries: Vec<ledger::LedgerEntry>,
        evidence_turns: Vec<InsomniaEvidenceTurn>,
        stage_reporter: &dyn Fn(InsomniaSemanticStage, usize),
    ) -> Result<InsomniaExtraction, InsomniaExtractionError> {
        let mut groups = synthesis::build_groups(&entries)?;
        if groups.is_empty() {
            return Ok(InsomniaExtraction {
                model: self.endpoint.model().to_owned(),
                candidates: Vec::new(),
                rejected: Vec::new(),
                evidence_turns,
            });
        }
        if let Some(metadata_endpoint) = &self.metadata_endpoint {
            stage_reporter(InsomniaSemanticStage::Metadata, groups.len());
            metadata::classify(
                metadata_endpoint.as_ref(),
                episode_payload,
                turns,
                &evidence_turns,
                &mut groups,
            )?;
        }
        if let Some(ownership_endpoint) = &self.ownership_endpoint {
            stage_reporter(InsomniaSemanticStage::Ownership, groups.len());
            ownership::classify(
                ownership_endpoint.as_ref(),
                episode_payload,
                turns,
                &evidence_turns,
                &mut groups,
            )?;
        }
        let payload = synthesis::payload(episode_payload, &groups);
        let payload = serde_json::to_string(&payload)
            .map_err(|error| InsomniaExtractionError::InvalidOutput(error.to_string()))?;
        stage_reporter(InsomniaSemanticStage::Wording, groups.len());
        let wording = self.endpoint.complete_json(
            synthesis::SYNTHESIS_SYSTEM_PROMPT,
            &payload,
            "insomnia_memory_wording",
            &synthesis::schema(&groups),
        )?;
        let (mut candidates, rejected) =
            synthesis::materialize_input(episode, turns, &evidence_turns, &groups, &wording)?;
        if let Some(enrichment_endpoint) = &self.enrichment_endpoint {
            stage_reporter(InsomniaSemanticStage::Enrichment, candidates.len());
            enrichment::enrich(enrichment_endpoint.as_ref(), &mut candidates)?;
        }
        Ok(InsomniaExtraction {
            model: self.endpoint.model().to_owned(),
            candidates,
            rejected,
            evidence_turns,
        })
    }
}
