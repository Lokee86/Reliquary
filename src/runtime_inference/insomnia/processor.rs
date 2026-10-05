use crate::{
    Archive, Container, Cva, GeneralEndpoint, InsomniaExtractionError, InsomniaExtractor,
    InsomniaRejection, InsomniaWork, InsomniaWorkState, Memory, MemoryError,
};
use std::fmt;

#[path = "processor/application.rs"]
mod application;
#[path = "processor/prepared.rs"]
mod prepared;
#[path = "processor/source_validation.rs"]
mod source_validation;

pub(crate) use application::{commit_application, prepare_application, publish_user_application};
pub(crate) use prepared::{PreparedApplication, PreparedMemory};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsomniaProcessResult {
    pub created: Vec<Memory>,
    pub existing: Vec<Memory>,
    pub user_created: Vec<Memory>,
    pub user_existing: Vec<Memory>,
    pub rejected: Vec<InsomniaRejection>,
}

#[derive(Debug)]
pub enum InsomniaProcessError {
    MissingEpisode,
    InvalidClaim,
    InvalidCandidate(String),
    UserRoutingRequired,
    Phylactery(String),
    Extraction(InsomniaExtractionError),
    Memory(MemoryError),
    Queue(crate::InsomniaError),
    Archive(crate::ArchiveError),
}

impl fmt::Display for InsomniaProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEpisode => write!(f, "Insomnia episode is missing"),
            Self::InvalidClaim => write!(f, "Insomnia work is not an active claim"),
            Self::InvalidCandidate(message) => write!(f, "invalid Insomnia candidate: {message}"),
            Self::UserRoutingRequired => write!(
                f,
                "Insomnia produced user-owned Memory but no Phylactery routing target was supplied"
            ),
            Self::Phylactery(message) => write!(f, "Phylactery routing failed: {message}"),
            Self::Extraction(error) => write!(f, "{error}"),
            Self::Memory(error) => write!(f, "{error}"),
            Self::Queue(error) => write!(f, "{error}"),
            Self::Archive(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for InsomniaProcessError {}

impl From<InsomniaExtractionError> for InsomniaProcessError {
    fn from(value: InsomniaExtractionError) -> Self {
        Self::Extraction(value)
    }
}

impl From<MemoryError> for InsomniaProcessError {
    fn from(value: MemoryError) -> Self {
        Self::Memory(value)
    }
}

impl From<crate::InsomniaError> for InsomniaProcessError {
    fn from(value: crate::InsomniaError) -> Self {
        Self::Queue(value)
    }
}

impl From<crate::ArchiveError> for InsomniaProcessError {
    fn from(value: crate::ArchiveError) -> Self {
        Self::Archive(value)
    }
}

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
        let extraction = extractor.extract_with_evidence(self, &episode, &turns)?;
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
        let extraction = extractor.extract_with_evidence(self, &episode, &turns)?;
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
        claimed_episode_input_parts(&self.archive, &mut self.container, claim, scope)
    }
}

pub(crate) fn claimed_episode_input_parts(
    archive: &Archive,
    container: &mut Container,
    claim: &InsomniaWork,
    scope: &str,
) -> Result<(crate::Episode, Vec<crate::ResolvedTurn>), InsomniaProcessError> {
    validate_claim(claim, scope)?;
    let episode = archive
        .episode(claim.episode_id)
        .cloned()
        .ok_or(InsomniaProcessError::MissingEpisode)?;
    let turns = archive.episode_turns(container, claim.episode_id)?;
    Ok((episode, turns))
}

fn validate_claim(claim: &InsomniaWork, scope: &str) -> Result<(), InsomniaProcessError> {
    if claim.state != InsomniaWorkState::Processing
        || claim.lease_token.is_none()
        || scope.trim().is_empty()
    {
        Err(InsomniaProcessError::InvalidClaim)
    } else {
        Ok(())
    }
}
