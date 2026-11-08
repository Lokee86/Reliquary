use super::*;
use reliquary_archive::EpisodeId;
use reliquary_insomnia_store::{AcceptedCompletionState, InsomniaWorkState};

/// Owner-verified inputs for replaying post-extraction work. Neither the
/// journal frame encoding nor a writable store escapes the owner.
pub struct CompletedImportExtraction {
    pub claim: RelInsomniaClaimInput,
    pub receipt: AcceptedCompletionState,
}

impl RelWriter {
    pub fn completed_import_extraction(
        &self,
        import_id: &str,
        episode_id: EpisodeId,
    ) -> Result<CompletedImportExtraction, OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        self.validate_import_episode_identity(import_id, episode_id)?;
        let id = WorkEpisodeId(episode_id.0);
        let work = self
            .session
            .insomnia
            .work(id)
            .filter(|work| work.state == InsomniaWorkState::Complete)
            .ok_or_else(|| OwnerSessionError::Record("import extraction is unfinished".into()))?;
        let receipt = self
            .session
            .insomnia
            .receipt(id, work.attempt_count)
            .ok_or_else(|| {
                OwnerSessionError::Record("import extraction receipt is missing".into())
            })?
            .clone();
        let checkpoint = InsomniaStore::work_history(&self.container, id)?
            .into_iter()
            .rev()
            .find(|work| {
                work.attempt_count == receipt.attempt
                    && work.updated_at_ns == receipt.started_at_ns
                    && work.state == InsomniaWorkState::Processing
                    && work.held_payload.is_some()
            })
            .ok_or_else(|| {
                OwnerSessionError::Record("accepted extraction has no exact checkpoint".into())
            })?;
        let episode = self.session.archive.episode(episode_id).cloned().ok_or(
            OwnerSessionError::Insomnia(InsomniaError::UnacceptedEpisode),
        )?;
        let turns = self.session.archive_evidence_episode(episode_id)?.ok_or(
            OwnerSessionError::Insomnia(InsomniaError::UnacceptedEpisode),
        )?;
        Ok(CompletedImportExtraction {
            claim: RelInsomniaClaimInput {
                owner_id: self.owner_id(),
                work: checkpoint,
                episode,
                turns,
                started_at_ns: receipt.started_at_ns,
            },
            receipt,
        })
    }
}
