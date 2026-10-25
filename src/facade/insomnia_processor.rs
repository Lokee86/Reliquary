pub use crate::insomnia::native_processor::{InsomniaProcessError, InsomniaProcessResult};
#[cfg(test)]
pub(super) use crate::public_insomnia_operations::extract_with_evidence;
use crate::{Cva, GeneralEndpoint, InsomniaExtractor, InsomniaWork};
impl Cva {
    pub fn process_claimed_insomnia_episode<E: GeneralEndpoint>(
        &mut self,
        claim: &InsomniaWork,
        extractor: &InsomniaExtractor<E>,
        scope: &str,
        started_at_ns: i64,
        completed_at_ns: i64,
    ) -> Result<InsomniaProcessResult, InsomniaProcessError> {
        self.native_process_claimed_insomnia_episode(
            claim,
            extractor,
            scope,
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
        self.native_process_claimed_insomnia_episode_routed(
            phylactery,
            claim,
            extractor,
            scope,
            started_at_ns,
            completed_at_ns,
        )
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
