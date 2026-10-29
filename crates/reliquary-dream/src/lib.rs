//! Pure Dream semantic policy over owner-supplied values.

mod candidate_config;
mod candidate_ranking;
mod canonical_policy;
mod classifier;
mod classifier_error;
mod classifier_model;
mod classifier_schema;
pub mod community_naming;
mod duplicate_policy;
mod evaluator;
mod evaluator_frontier;
mod lifecycle;
mod pair_context;
mod publication_policy;
mod verifier;
mod verifier_error;
mod verifier_model;
mod verifier_schema;

pub use candidate_config::*;
pub use candidate_ranking::*;
pub use canonical_policy::{
    DreamCanonicalMemory, DreamCanonicalSupersederFact, canonical_superseders,
    corroborated_representative, has_active_duplicate_peer, historical_supersession_terminal,
    select_duplicate_representative, unique_superseder,
};
pub use classifier::DreamClassifier;
pub use classifier_error::*;
pub use classifier_model::*;
pub use classifier_schema::*;
pub use community_naming::{
    DREAM_COMMUNITY_NAMING_RETRY_LIMIT, DREAM_COMMUNITY_NAMING_SYSTEM_PROMPT,
    DreamCommunityNameEvaluationError, DreamCommunityNameEvaluator,
    build_payload as build_community_naming_payload, dream_community_naming_schema,
    parse_name as parse_community_name, representative_ids as community_representative_ids,
};
pub use duplicate_policy::{
    DuplicateIndex, DuplicateTemporalKey, duplicate_component, representative_order_key,
};
pub use evaluator::{DreamEvaluationError, DreamPairEvaluator};
pub use lifecycle::{
    DreamLifecycleIntent, DreamLifecycleMemory, explicit_authority_promotes, plan_lifecycle,
};
pub use pair_context::{DreamCandidateContexts, DreamContext, DreamRelationContext};
pub use publication_policy::*;
pub use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
pub use verifier::DreamVerifier;
pub use verifier_error::*;
pub use verifier_model::*;
pub use verifier_schema::*;

use reliquary_memory_core::Memory;
use std::collections::HashSet;

pub fn lexical_score(source: &Memory, candidate: &Memory) -> f64 {
    let source_tokens = tokens(&format!("{} {}", source.title, source.content));
    let candidate_tokens = tokens(&format!("{} {}", candidate.title, candidate.content));
    let shared = source_tokens.intersection(&candidate_tokens).count() as f64;
    let union = source_tokens.union(&candidate_tokens).count() as f64;
    let mut score = if union > 0.0 { shared / union } else { 0.0 };
    if source.category == candidate.category {
        score += 0.08;
    }
    if source.memory_type == candidate.memory_type {
        score += 0.08;
    }
    if source.parent_id.is_some() && source.parent_id == candidate.parent_id {
        score += 0.12;
    }
    score
}

fn tokens(text: &str) -> HashSet<String> {
    text.split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| token.len() > 1)
        .map(str::to_lowercase)
        .collect()
}
