//! Insomnia ownership boundary.

mod candidate_policy;
mod candidate_receipt_policy;
mod candidate_text;

pub use candidate_policy::validate_semantic_authority;
mod evidence_request;
pub use evidence_request::{
    EvidenceRequest, MAX_INSOMNIA_EVIDENCE_BYTES, MAX_INSOMNIA_EVIDENCE_REQUESTS,
    MAX_INSOMNIA_EVIDENCE_TURNS, parse_evidence_requests,
};

mod candidate;
mod candidate_shape;
mod candidate_source;
mod contract;
mod enrichment;
mod extraction;
mod input;
mod ledger;
mod metadata;
mod ownership;
mod synthesis;

pub use candidate::hex;
pub use contract::{
    INSOMNIA_EXTRACTOR_CONTRACT_VERSION, INSOMNIA_SYSTEM_PROMPT, MAX_INSOMNIA_CANDIDATES,
    insomnia_schema,
};
pub use enrichment::enrich;
pub use extraction::{
    InsomniaCandidate, InsomniaEvidenceResult, InsomniaEvidenceRound, InsomniaEvidenceTurn,
    InsomniaExtraction, InsomniaExtractionError, InsomniaExtractionStage, InsomniaExtractor,
    InsomniaRejection, InsomniaRoutingMetadata,
};
pub use synthesis::SYNTHESIS_SYSTEM_PROMPT;
pub fn ledger_schema(
    turns: &[ResolvedTurn],
    evidence_turns: &[InsomniaEvidenceTurn],
    allow_evidence_requests: bool,
) -> serde_json::Value {
    ledger::schema(turns, evidence_turns, allow_evidence_requests)
}
pub use ownership::{INSOMNIA_OWNERSHIP_SYSTEM_PROMPT, InsomniaOwnership};
pub use reliquary_archive::{Episode, EpisodeId, ResolvedTurn};
pub use reliquary_memory_core::{
    MAX_MEMORY_ENTITY_MENTIONS, MAX_MEMORY_ROUTING_TEXT_BYTES, MemoryEntityMention, MemoryTextField,
};
pub use reliquary_switchboard::{
    DecisionEndpoint, DecisionEndpointError, GeneralEndpoint, GeneralEndpointError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InsomniaSemanticStage {
    Ledger,
    LedgerTurnRepair,
    LedgerFieldRepair,
    Evidence,
    Metadata,
    Ownership,
    Wording,
    Enrichment,
    Persistence,
}

impl InsomniaSemanticStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ledger => "ledger",
            Self::LedgerTurnRepair => "ledger_turn_repair",
            Self::LedgerFieldRepair => "ledger_field_repair",
            Self::Evidence => "evidence",
            Self::Metadata => "metadata",
            Self::Ownership => "ownership",
            Self::Wording => "wording",
            Self::Enrichment => "enrichment",
            Self::Persistence => "persistence",
        }
    }
}

#[cfg(test)]
mod candidate_policy_tests;
#[cfg(test)]
mod candidate_receipt_policy_tests;
