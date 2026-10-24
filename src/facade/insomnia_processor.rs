use crate::{Cva, GeneralEndpoint, InsomniaExtractionError, InsomniaExtractor, InsomniaWork};

pub use crate::insomnia::native_processor::{InsomniaProcessError, InsomniaProcessResult};
use crate::insomnia::native_processor::{prepare_application, publish_user_application};
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
        let mut prepared = prepare_application(
            &self.archive,
            &mut self.container,
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
            &mut self.container,
            &mut self.memories,
            &mut self.insomnia,
            &mut self.freshness,
            &mut self.dream_freshness,
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
        let mut prepared = prepare_application(
            &self.archive,
            &mut self.container,
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
            Some(publish_user_application(phylactery, &prepared.user_drafts)?)
        };
        let owner_uuid = self.owner_uuid();
        let accepted_rel_turn = self.rel_turn_count();
        commit_application(
            &mut self.container,
            &mut self.memories,
            &mut self.insomnia,
            &mut self.freshness,
            &mut self.dream_freshness,
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
        crate::insomnia::native_processor::claimed_episode_input_parts(
            &self.archive,
            &mut self.container,
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
                &cva.archive,
                &mut cva.lexical_index,
                &mut cva.container,
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
            &mut self.container,
            &self.memories,
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
                container: &mut self.container,
                archive: &self.archive,
                memories: &mut self.memories,
                insomnia: &mut self.insomnia,
                freshness: &mut self.freshness,
                dream_freshness: &mut self.dream_freshness,
            },
            completion,
        )
    }
}

impl application::UserMemoryOwner for crate::Phylactery {
    fn owner_id(&self) -> Option<String> {
        crate::Phylactery::owner_id(self)
    }
    fn publish(
        &mut self,
        prepared: &PreparedUserMemory,
        temporal: Option<crate::MemoryTemporalInference>,
    ) -> Result<(crate::Memory, bool), crate::MemoryError> {
        self.publish_memory_with_source_ref_and_temporal_inference(
            None,
            0,
            prepared.memory.draft.clone(),
            prepared.source_ref.clone(),
            temporal,
        )
    }
    fn memory(&mut self, id: crate::MemoryId) -> Result<crate::Memory, crate::MemoryError> {
        crate::Phylactery::memory(self, id)
    }
    fn put_routing_metadata(
        &mut self,
        metadata: crate::MemoryRoutingMetadata,
    ) -> Result<(), crate::MemoryError> {
        self.memories
            .put_routing_metadata(&mut self.container, metadata)
            .map(|_| ())
    }
    fn sync(&mut self) -> Result<(), String> {
        crate::Phylactery::sync(self).map_err(|error| error.to_string())
    }
}
