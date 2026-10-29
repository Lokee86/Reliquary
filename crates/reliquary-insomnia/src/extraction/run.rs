use super::*;

impl<E: GeneralEndpoint> InsomniaExtractor<E> {
    /// Shared ledger, metadata, ownership, wording and enrichment over host-owned
    /// transient turns. External evidence cannot fall back to an Archive lookup.
    pub fn extract_external_input(
        &self,
        key: [u8; 32],
        namespace: &str,
        conversation: &str,
        turns: &[ResolvedTurn],
    ) -> Result<InsomniaExtraction, InsomniaExtractionError> {
        if turns.is_empty() {
            return Err(InsomniaExtractionError::InvalidOutput(
                "external input is empty".into(),
            ));
        }
        let input = crate::input::InsomniaSourceInput {
            key,
            conversation_id: conversation.to_owned(),
        };
        let payload = input.external_payload(namespace, turns);
        let ledger = self.complete_ledger(&payload, turns, &[], false, &no_stage_progress)?;
        if !parse_evidence_requests(&ledger)?.is_empty() {
            return Err(InsomniaExtractionError::InvalidOutput(
                "external evidence must be in the authorized window".into(),
            ));
        }
        let entries = ledger::parse(&ledger, turns, &[])?;
        self.synthesize(
            &input,
            turns,
            &payload,
            entries,
            Vec::new(),
            &no_stage_progress,
        )
    }

    pub fn extract(
        &self,
        episode: &Episode,
        turns: &[ResolvedTurn],
    ) -> Result<InsomniaExtraction, InsomniaExtractionError> {
        match self.start(episode, turns)? {
            InsomniaExtractionStage::Complete(extraction) => Ok(extraction),
            InsomniaExtractionStage::Evidence(_) => Err(InsomniaExtractionError::InvalidOutput(
                "archive evidence was requested but no evidence archive was supplied".into(),
            )),
        }
    }

    pub fn start(
        &self,
        episode: &Episode,
        turns: &[ResolvedTurn],
    ) -> Result<InsomniaExtractionStage, InsomniaExtractionError> {
        self.start_with_stage_reporter(episode, turns, &no_stage_progress)
    }

    pub fn start_with_stage_reporter(
        &self,
        episode: &Episode,
        turns: &[ResolvedTurn],
        stage_reporter: &dyn Fn(InsomniaSemanticStage, usize),
    ) -> Result<InsomniaExtractionStage, InsomniaExtractionError> {
        let episode_payload = encode_episode_value(episode, turns)?;
        let initial_ledger =
            self.complete_ledger(&episode_payload, turns, &[], true, stage_reporter)?;
        let requests = parse_evidence_requests(&initial_ledger)?;
        if requests.is_empty() {
            let entries = ledger::parse(&initial_ledger, turns, &[])?;
            return self
                .synthesize(
                    &crate::input::InsomniaSourceInput::native(episode),
                    turns,
                    &episode_payload,
                    entries,
                    Vec::new(),
                    stage_reporter,
                )
                .map(InsomniaExtractionStage::Complete);
        }
        Ok(InsomniaExtractionStage::Evidence(InsomniaEvidenceRound {
            episode_payload,
            initial_ledger,
            requests,
        }))
    }

    pub fn finish_evidence(
        &self,
        episode: &Episode,
        turns: &[ResolvedTurn],
        round: InsomniaEvidenceRound,
        results: Vec<InsomniaEvidenceResult>,
        evidence_turns: Vec<InsomniaEvidenceTurn>,
    ) -> Result<InsomniaExtraction, InsomniaExtractionError> {
        self.finish_evidence_with_stage_reporter(
            episode,
            turns,
            round,
            results,
            evidence_turns,
            &no_stage_progress,
        )
    }

    pub fn finish_evidence_with_stage_reporter(
        &self,
        episode: &Episode,
        turns: &[ResolvedTurn],
        round: InsomniaEvidenceRound,
        results: Vec<InsomniaEvidenceResult>,
        evidence_turns: Vec<InsomniaEvidenceTurn>,
        stage_reporter: &dyn Fn(InsomniaSemanticStage, usize),
    ) -> Result<InsomniaExtraction, InsomniaExtractionError> {
        if encode_episode_value(episode, turns)? != round.episode_payload {
            return Err(InsomniaExtractionError::InvalidOutput(
                "evidence round does not match the authoritative episode".into(),
            ));
        }
        validate_evidence_response(&round, &results, &evidence_turns)?;
        let evidence_results: Vec<_> = results.iter().map(evidence_result_json).collect();
        let evidence_payload_turns: Vec<_> =
            evidence_turns.iter().map(evidence_turn_json).collect();
        let payload = json!({
            "authoritative_episode": round.episode_payload,
            "initial_authority_disposition_ledger": round.initial_ledger,
            "read_only_archive_evidence": {
                "results": evidence_results,
                "turns": evidence_payload_turns
            },
            "instruction": "Return the final authority/disposition ledger now. evidence_requests MUST be empty; no second evidence round is allowed. Archive evidence may resolve a referent through grounding_source_node_id or supply earlier assistant-authored content explicitly adopted/retained through authority_source_node_id, but archive evidence cannot supply user authority."
        });
        let final_ledger =
            self.complete_ledger(&payload, turns, &evidence_turns, false, stage_reporter)?;
        if !parse_evidence_requests(&final_ledger)?.is_empty() {
            return Err(InsomniaExtractionError::InvalidOutput(
                "model requested more than one archive-evidence round".into(),
            ));
        }
        let entries = ledger::parse(&final_ledger, turns, &evidence_turns)?;
        self.synthesize(
            &crate::input::InsomniaSourceInput::native(episode),
            turns,
            &payload["authoritative_episode"],
            entries,
            evidence_turns,
            stage_reporter,
        )
    }
}
