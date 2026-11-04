//! Pure owner-qualified Memory selection. This module records no use receipt
//! and does not alter freshness; a host must confirm consumption separately.

use crate::EntityOwnerSession;
use reliquary_owner_session::model::{MemoryBody, MemoryBodyId, MemoryId, PayloadState};
use reliquary_retrieval::{RetrievalConfig, combine_hybrid_scores, validate_hybrid_request};
use std::collections::HashSet;
use std::fmt;

/// Caller-supplied ranking evidence. Scores are untrusted hints: they influence
/// order only and never grant access to a Memory or establish delivery.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OwnerMemoryEvidence {
    pub owner_id: [u8; 16],
    pub memory_id: MemoryId,
    pub lexical_score: Option<f64>,
    pub semantic_score: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectedMemory {
    pub memory_id: MemoryId,
    pub revision: u64,
    pub body_id: MemoryBodyId,
    pub body: MemoryBody,
    pub lexical_score: Option<f64>,
    pub semantic_score: Option<f64>,
    pub combined_score: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OwnerMemorySelection {
    pub owner_id: [u8; 16],
    pub selected: Vec<SelectedMemory>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetrievalWorkflowError {
    OwnerMismatch,
    EmptyQuery,
    InvalidConfig,
    CandidateLimit,
    DuplicateMemory,
    InvalidScore,
    MemoryUnavailable(MemoryId),
    MemoryArchived(MemoryId),
    MemoryBodyUnavailable(MemoryId),
    OwnerRead(String),
}

impl fmt::Display for RetrievalWorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnerMismatch => {
                f.write_str("retrieval evidence does not match authorized owner")
            }
            Self::EmptyQuery => f.write_str("retrieval query is empty"),
            Self::InvalidConfig => f.write_str("retrieval configuration is invalid"),
            Self::CandidateLimit => f.write_str("retrieval candidate limit exceeded"),
            Self::DuplicateMemory => f.write_str("retrieval repeats a Memory identity"),
            Self::InvalidScore => f.write_str("retrieval evidence contains an invalid score"),
            Self::MemoryUnavailable(id) => write!(f, "Memory {id:?} is unavailable in owner"),
            Self::MemoryArchived(id) => write!(f, "Memory {id:?} is archived"),
            Self::MemoryBodyUnavailable(id) => write!(f, "Memory {id:?} body is unavailable"),
            Self::OwnerRead(reason) => write!(f, "owner Memory read failed: {reason}"),
        }
    }
}

impl std::error::Error for RetrievalWorkflowError {}

/// Select from already-ranked owner-local evidence after the caller has
/// authorized this owner through its Gateway session. The `authorized_owner_id`
/// argument is an owner recheck, not an authentication credential. Callers must
/// perform authorization both before retrieval and before host handoff.
/// Evidence scores are untrusted caller inputs; bodies are rehydrated from the
/// supplied immutable session, and each result is restricted to that owner.
///
/// This function returns only currently readable, non-archived Memory bodies.
/// It does not claim external delivery and never reinforces Freshness.
pub fn select_owner_memories(
    owner: &EntityOwnerSession<'_>,
    authorized_owner_id: [u8; 16],
    query: &str,
    evidence: &[OwnerMemoryEvidence],
    config: RetrievalConfig,
) -> Result<OwnerMemorySelection, RetrievalWorkflowError> {
    let owner_id = owner.owner_id();
    if owner_id != authorized_owner_id || evidence.iter().any(|hit| hit.owner_id != owner_id) {
        return Err(RetrievalWorkflowError::OwnerMismatch);
    }
    if query.trim().is_empty() {
        return Err(RetrievalWorkflowError::EmptyQuery);
    }
    let weights = validate_hybrid_request(query, config).map_err(|error| match error {
        reliquary_retrieval::HybridPolicyError::EmptyQuery => RetrievalWorkflowError::EmptyQuery,
        reliquary_retrieval::HybridPolicyError::InvalidConfig => {
            RetrievalWorkflowError::InvalidConfig
        }
    })?;
    if evidence.len() > config.candidate_limit {
        return Err(RetrievalWorkflowError::CandidateLimit);
    }
    let mut seen = HashSet::with_capacity(evidence.len());
    let mut candidates = Vec::new();
    for hit in evidence {
        if !seen.insert(hit.memory_id) {
            return Err(RetrievalWorkflowError::DuplicateMemory);
        }
        for score in [hit.lexical_score, hit.semantic_score]
            .into_iter()
            .flatten()
        {
            if !score.is_finite() || score <= 0.0 {
                return Err(RetrievalWorkflowError::InvalidScore);
            }
        }
        if hit.lexical_score.is_none() && hit.semantic_score.is_none() {
            continue;
        }
        let history = owner
            .memory_history(hit.memory_id)
            .map_err(|error| RetrievalWorkflowError::OwnerRead(error.to_string()))?
            .ok_or(RetrievalWorkflowError::MemoryUnavailable(hit.memory_id))?;
        if history.tombstone.is_some() {
            continue;
        }
        let Some(revision) = history.revisions.last() else {
            return Err(RetrievalWorkflowError::MemoryUnavailable(hit.memory_id));
        };
        if revision.record.archived {
            return Err(RetrievalWorkflowError::MemoryArchived(hit.memory_id));
        }
        let body = match &revision.payload {
            PayloadState::Available(body) => body.clone(),
            PayloadState::Absent => {
                return Err(RetrievalWorkflowError::MemoryBodyUnavailable(hit.memory_id));
            }
        };
        let lexical = hit.lexical_score.unwrap_or(0.0);
        let semantic = hit.semantic_score.unwrap_or(0.0);
        candidates.push(SelectedMemory {
            memory_id: hit.memory_id,
            revision: revision.record.revision,
            body_id: revision.record.body_id,
            body,
            lexical_score: hit.lexical_score,
            semantic_score: hit.semantic_score,
            combined_score: combine_hybrid_scores(lexical, semantic, weights),
        });
    }
    candidates.sort_by(|left, right| {
        right
            .combined_score
            .total_cmp(&left.combined_score)
            .then_with(|| left.memory_id.cmp(&right.memory_id))
    });
    candidates.truncate(config.result_limit);
    Ok(OwnerMemorySelection {
        owner_id,
        selected: candidates,
    })
}
