use super::enrichment;
use super::evidence_request::{EvidenceRequest, parse_evidence_requests as parse_requests};
use super::ledger;
use super::metadata;
use super::ownership::{self, InsomniaOwnership};
use super::synthesis;
use crate::{
    DecisionEndpoint, DecisionEndpointError, Episode, GeneralEndpoint, GeneralEndpointError,
    InsomniaSemanticStage, MemoryEntityMention, ResolvedTurn,
};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsomniaRoutingMetadata {
    pub entity_mentions: Vec<MemoryEntityMention>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsomniaCandidate {
    pub key: String,
    pub authority_kind: String,
    pub category: String,
    pub memory_type: String,
    pub temporal_status: String,
    pub ownership: InsomniaOwnership,
    pub title: String,
    pub content: String,
    pub source_node_id: String,
    pub source_quote: String,
    pub authority_source_conversation_id: Option<String>,
    pub authority_source_node_id: Option<String>,
    pub authority_source_quote: Option<String>,
    pub grounding_source_conversation_id: Option<String>,
    pub grounding_source_node_id: Option<String>,
    pub grounding_source_quote: Option<String>,
    pub routing_metadata: Option<InsomniaRoutingMetadata>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsomniaRejection {
    pub candidate_key: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsomniaEvidenceTurn {
    pub conversation_id: String,
    pub node_id: String,
    pub role: String,
    pub timestamp_ns: i64,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsomniaEvidenceResult {
    pub kind: String,
    pub turns: Vec<InsomniaEvidenceTurn>,
    pub error: Option<String>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsomniaExtraction {
    pub model: String,
    pub candidates: Vec<InsomniaCandidate>,
    pub rejected: Vec<InsomniaRejection>,
    pub evidence_turns: Vec<InsomniaEvidenceTurn>,
}

pub enum InsomniaExtractionStage {
    Complete(InsomniaExtraction),
    Evidence(InsomniaEvidenceRound),
}

pub struct InsomniaEvidenceRound {
    episode_payload: Value,
    initial_ledger: Value,
    requests: Vec<EvidenceRequest>,
}

impl InsomniaEvidenceRound {
    /// The bounded immutable requests that Root must resolve and authorize.
    pub fn requests(&self) -> &[EvidenceRequest] {
        &self.requests
    }
}

#[derive(Debug)]
pub enum InsomniaExtractionError {
    Endpoint(GeneralEndpointError),
    InvalidOutput(String),
    Decision(DecisionEndpointError),
}

impl fmt::Display for InsomniaExtractionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Endpoint(error) => write!(f, "{error}"),
            Self::InvalidOutput(message) => write!(f, "invalid Insomnia extraction: {message}"),
            Self::Decision(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for InsomniaExtractionError {}

impl From<GeneralEndpointError> for InsomniaExtractionError {
    fn from(value: GeneralEndpointError) -> Self {
        Self::Endpoint(value)
    }
}

fn parse_evidence_requests(value: &Value) -> Result<Vec<EvidenceRequest>, InsomniaExtractionError> {
    parse_requests(value).map_err(InsomniaExtractionError::InvalidOutput)
}

pub struct InsomniaExtractor<E> {
    endpoint: E,
    metadata_endpoint: Option<Arc<dyn GeneralEndpoint>>,
    enrichment_endpoint: Option<Arc<dyn GeneralEndpoint>>,
    ownership_endpoint: Option<Arc<dyn GeneralEndpoint>>,
    temporal_endpoint: Option<Arc<dyn DecisionEndpoint>>,
}

impl<E: GeneralEndpoint> InsomniaExtractor<E> {
    pub fn new(endpoint: E) -> Self {
        Self {
            endpoint,
            metadata_endpoint: None,
            enrichment_endpoint: None,
            ownership_endpoint: None,
            temporal_endpoint: None,
        }
    }

    pub fn with_metadata_endpoint<M>(mut self, endpoint: M) -> Self
    where
        M: GeneralEndpoint + 'static,
    {
        self.metadata_endpoint = Some(Arc::new(endpoint));
        self
    }

    pub fn with_enrichment_endpoint<R>(mut self, endpoint: R) -> Self
    where
        R: GeneralEndpoint + 'static,
    {
        self.enrichment_endpoint = Some(Arc::new(endpoint));
        self
    }

    pub fn with_ownership_endpoint<O>(mut self, endpoint: O) -> Self
    where
        O: GeneralEndpoint + 'static,
    {
        self.ownership_endpoint = Some(Arc::new(endpoint));
        self
    }

    pub fn with_temporal_decision(mut self, endpoint: Arc<dyn DecisionEndpoint>) -> Self {
        self.temporal_endpoint = Some(endpoint);
        self
    }

    pub fn temporal_endpoint(&self) -> Option<&dyn DecisionEndpoint> {
        self.temporal_endpoint.as_deref()
    }

    pub fn model(&self) -> &str {
        self.endpoint.model()
    }
}

#[path = "extraction/ledger_run.rs"]
mod ledger_run;
#[path = "extraction/payload.rs"]
mod payload;
#[path = "extraction/response.rs"]
mod response;
#[cfg(test)]
#[path = "extraction/response_tests.rs"]
mod response_tests;
#[path = "extraction/run.rs"]
mod run;
#[path = "extraction/synthesis_run.rs"]
mod synthesis_run;

use payload::{encode_episode_value, evidence_result_json, evidence_turn_json, no_stage_progress};
use response::validate_evidence_response;
