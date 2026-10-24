#[path = "processor/application.rs"]
pub(crate) mod application;
#[path = "processor/prepared.rs"]
mod prepared;
#[path = "processor/result.rs"]
mod result;
#[path = "processor/source_validation.rs"]
mod source_validation;

pub(crate) use application::prepare_application_read;
pub(crate) use prepared::{
    PreparedApplication, PreparedMemory, PreparedUserMemory, UserPublication,
};
pub use result::{InsomniaProcessError, InsomniaProcessResult};

impl super::evidence::EvidenceSearch for crate::lexical_index::LexicalIndex {
    fn ensure_current(
        &mut self,
        archive: &crate::archive::ArchiveReadView<'_>,
    ) -> Result<(), crate::ArchiveError> {
        crate::lexical_index::LexicalIndex::ensure_current(self, archive)
    }

    fn search_fragments(&self, query: &str, limit: usize) -> Vec<crate::Fragment> {
        self.search(&crate::lexical_contract::lexical_terms(query), limit)
            .into_iter()
            .map(|hit| hit.fragment)
            .collect()
    }
}

fn validate_claim(claim: &crate::InsomniaWork, scope: &str) -> Result<(), InsomniaProcessError> {
    if claim.state != crate::InsomniaWorkState::Processing
        || claim.lease_token.is_none()
        || scope.trim().is_empty()
    {
        Err(InsomniaProcessError::InvalidClaim)
    } else {
        Ok(())
    }
}

pub(crate) fn claimed_episode_input_read(
    archive: &crate::archive::ArchiveReadView<'_>,
    claim: &crate::InsomniaWork,
    scope: &str,
) -> Result<(crate::Episode, Vec<crate::ResolvedTurn>), InsomniaProcessError> {
    validate_claim(claim, scope)?;
    let episode = archive
        .episode(claim.episode_id)
        .cloned()
        .ok_or(InsomniaProcessError::MissingEpisode)?;
    let turns = archive.episode_turns(claim.episode_id)?;
    Ok((episode, turns))
}
