//! Accepted LIVE extraction projections. No worker claims or writable stores escape.
use super::*;
use reliquary_archive::{EpisodeId, EpisodeOrigin};
use reliquary_insomnia_store::{AcceptedCompletionState, InsomniaWorkState};

pub struct LiveExtractionOutput<'a> {
    pub episode: EpisodeId,
    pub model: &'a str,
    pub local: Vec<MemoryId>,
    pub routed: Vec<MemoryId>,
}

impl RelSession {
    pub fn visit_live_outputs(
        &self,
        destination: Option<[u8; 16]>,
        mut visit: impl FnMut(LiveExtractionOutput<'_>),
    ) {
        let destination = destination.map(|id| {
            id.iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        });
        for work in self.insomnia.work_items() {
            if let Some(output) = self.live_output(work.episode_id, destination.as_deref()) {
                visit(output);
            }
        }
    }

    fn live_output(
        &self,
        id: WorkEpisodeId,
        destination: Option<&str>,
    ) -> Option<LiveExtractionOutput<'_>> {
        let work = self.insomnia.work(id)?;
        if work.state != InsomniaWorkState::Complete
            || self.archive.episode(EpisodeId(id.0))?.origin != EpisodeOrigin::Live
        {
            return None;
        }
        let AcceptedCompletionState {
            extractor_model,
            memory_ids,
            external_memory_refs,
            ..
        } = self.insomnia.receipt(id, work.attempt_count)?;
        let routed: Vec<_> = external_memory_refs
            .iter()
            .filter(|reference| destination == Some(reference.owner_id.as_str()))
            .map(|reference| MemoryId(reference.memory_id.0))
            .collect();
        if memory_ids.is_empty() && routed.is_empty() {
            return None;
        }
        Some(LiveExtractionOutput {
            episode: EpisodeId(id.0),
            model: extractor_model,
            local: memory_ids.iter().map(|id| MemoryId(id.0)).collect(),
            routed,
        })
    }

    pub fn visit_live_checkpoints(
        &self,
        destination: Option<[u8; 16]>,
        mut visit: impl FnMut(LiveExtractionOutput<'_>, &[u8]),
    ) -> Result<(), OwnerSessionError> {
        // Mounted writers use their already-held Container through RelWriter.
        if self.writer_lease_proof.is_some() {
            return Err(OwnerSessionError::ManagementConflict);
        }
        let container = Container::open_read_only_as(&self.container_path, FileKind::Reliquary)?;
        if container.owner_id() != self.owner_id
            || container.physical_file_identity()? != self.container_file_identity
        {
            return Err(OwnerSessionError::ManagementConflict);
        }
        self.visit_live_checkpoints_in(&container, destination, &mut visit)
    }

    fn visit_live_checkpoints_in(
        &self,
        container: &Container,
        destination: Option<[u8; 16]>,
        visit: &mut impl FnMut(LiveExtractionOutput<'_>, &[u8]),
    ) -> Result<(), OwnerSessionError> {
        let destination = destination.map(|id| {
            id.iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        });
        self.insomnia
            .visit_completed_checkpoints(container, |id, payload| {
                if let Some(output) = self.live_output(id, destination.as_deref()) {
                    visit(output, payload);
                }
            })?;
        Ok(())
    }
}

impl RelWriter {
    pub fn validate_live_processing(&self) -> Result<(), OwnerSessionError> {
        self.ensure_write_ready()
    }

    pub fn retry_held_live_episode(
        &mut self,
        episode: EpisodeId,
        now_ns: i64,
    ) -> Result<(), OwnerSessionError> {
        self.ensure_write_ready()?;
        if !self
            .session
            .archive
            .episode(episode)
            .is_some_and(|row| row.origin == EpisodeOrigin::Live)
        {
            return Err(OwnerSessionError::Insomnia(
                InsomniaError::UnacceptedEpisode,
            ));
        }
        let result =
            self.session
                .insomnia
                .retry_held(&mut self.container, WorkEpisodeId(episode.0), now_ns);
        self.finish_insomnia_write(result).map(|_| ())
    }

    pub fn visit_live_checkpoints(
        &self,
        destination: Option<[u8; 16]>,
        mut visit: impl FnMut(LiveExtractionOutput<'_>, &[u8]),
    ) -> Result<(), OwnerSessionError> {
        self.session
            .visit_live_checkpoints_in(&self.container, destination, &mut visit)
    }
}
