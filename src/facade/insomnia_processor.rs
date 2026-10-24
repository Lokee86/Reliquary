use crate::owner_lifecycle::personal_memory_publication::publish_user_application;
use crate::{Cva, GeneralEndpoint, InsomniaExtractionError, InsomniaExtractor, InsomniaWork};

pub use crate::insomnia::native_processor::{InsomniaProcessError, InsomniaProcessResult};
use crate::insomnia::native_processor::{
    PreparedUserMemory, application, prepare_application_read,
};
pub(crate) use crate::owner_lifecycle::insomnia_application::commit_application;

impl Cva {
    pub fn process_claimed_insomnia_episode<E: GeneralEndpoint>(
        &mut self,
        claim: &InsomniaWork,
        extractor: &InsomniaExtractor<E>,
        scope: &str,
        started_at_ns: i64,
        completed_at_ns: i64,
    ) -> Result<InsomniaProcessResult, InsomniaProcessError> {
        let (episode, turns) = self.claimed_episode_input(claim, scope)?;
        let extraction = extract_with_evidence(self, extractor, &episode, &turns)?;
        let mut prepared = prepare_application_read(
            &crate::archive::ArchiveReadView::new(&self.archive, &self.container),
            self.container.owner_id(),
            &episode,
            &turns,
            extraction,
            scope,
            completed_at_ns,
        )?;
        crate::insomnia::temporal::infer_prepared(extractor.temporal_endpoint(), &mut prepared)?;
        let owner_uuid = self.owner_uuid();
        let accepted_rel_turn = self.rel_turn_count();
        commit_application(
            &mut self.owner.container,
            &mut self.owner.memories,
            &mut self.owner.insomnia,
            &mut self.owner.freshness,
            &mut self.owner.dream_freshness,
            owner_uuid,
            accepted_rel_turn,
            claim,
            prepared,
            None,
            started_at_ns,
            completed_at_ns,
        )
    }

    pub fn process_claimed_insomnia_episode_routed<E: GeneralEndpoint>(
        &mut self,
        phylactery: &mut crate::Phylactery,
        claim: &InsomniaWork,
        extractor: &InsomniaExtractor<E>,
        scope: &str,
        started_at_ns: i64,
        completed_at_ns: i64,
    ) -> Result<InsomniaProcessResult, InsomniaProcessError> {
        let (episode, turns) = self.claimed_episode_input(claim, scope)?;
        let extraction = extract_with_evidence(self, extractor, &episode, &turns)?;
        let mut prepared = prepare_application_read(
            &crate::archive::ArchiveReadView::new(&self.archive, &self.container),
            self.container.owner_id(),
            &episode,
            &turns,
            extraction,
            scope,
            completed_at_ns,
        )?;
        crate::insomnia::temporal::infer_prepared(extractor.temporal_endpoint(), &mut prepared)?;
        let user_publication = if prepared.user_drafts.is_empty() {
            None
        } else {
            Some(publish_user_application(&mut crate::owner_lifecycle::personal_memory_publication::PersonalMemoryPublication::new(&mut phylactery.owner.container,&mut phylactery.owner.memories), &prepared.user_drafts)?)
        };
        let owner_uuid = self.owner_uuid();
        let accepted_rel_turn = self.rel_turn_count();
        commit_application(
            &mut self.owner.container,
            &mut self.owner.memories,
            &mut self.owner.insomnia,
            &mut self.owner.freshness,
            &mut self.owner.dream_freshness,
            owner_uuid,
            accepted_rel_turn,
            claim,
            prepared,
            user_publication,
            started_at_ns,
            completed_at_ns,
        )
    }

    pub(crate) fn claimed_episode_input(
        &mut self,
        claim: &InsomniaWork,
        scope: &str,
    ) -> Result<(crate::Episode, Vec<crate::ResolvedTurn>), InsomniaProcessError> {
        crate::insomnia::native_processor::claimed_episode_input_read(
            &crate::archive::ArchiveReadView::new(&self.archive, &self.container),
            claim,
            scope,
        )
    }
}

pub(super) fn extract_with_evidence<E: GeneralEndpoint>(
    cva: &mut Cva,
    extractor: &InsomniaExtractor<E>,
    episode: &crate::Episode,
    turns: &[crate::ResolvedTurn],
) -> Result<crate::InsomniaExtraction, InsomniaExtractionError> {
    use super::extraction::InsomniaExtractionStage;

    match extractor.start(episode, turns)? {
        InsomniaExtractionStage::Complete(extraction) => Ok(extraction),
        InsomniaExtractionStage::Evidence(round) => {
            let (results, evidence_turns) = super::evidence::resolve_evidence_parts(
                &crate::archive::ArchiveReadView::new(&cva.owner.archive, &cva.owner.container),
                &mut cva.lexical_index,
                episode,
                &round.requests,
            )?;
            extractor.finish_evidence(episode, turns, round, results, evidence_turns)
        }
    }
}

#[cfg(test)]
impl Cva {
    pub(crate) fn hydrate_grouped_import_completion_bodies(
        &mut self,
        completion: &mut crate::insomnia::completion::InsomniaCompletion,
    ) -> Result<(), crate::CvaError> {
        crate::owner_lifecycle::grouped_import_replay::hydrate_grouped_import_completion_bodies(
            &mut self.owner.container,
            &self.owner.memories,
            completion,
        )
        .map_err(Into::into)
    }
    pub(crate) fn replay_grouped_import_completion(
        &mut self,
        completion: crate::insomnia::completion::InsomniaCompletion,
    ) -> Result<bool, crate::CvaError> {
        crate::owner_lifecycle::grouped_import_replay::replay_grouped_import_completion(
            crate::owner_lifecycle::grouped_import_replay::GroupedImportDestination {
                container: &mut self.owner.container,
                archive: &self.owner.archive,
                memories: &mut self.owner.memories,
                insomnia: &mut self.owner.insomnia,
                freshness: &mut self.owner.freshness,
                dream_freshness: &mut self.owner.dream_freshness,
            },
            completion,
        )
    }
}
