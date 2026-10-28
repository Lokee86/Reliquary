#[path = "insomnia/backpressure.rs"]
pub(crate) mod backpressure;
#[path = "insomnia/completion.rs"]
pub(crate) mod completion;
#[path = "insomnia/cva.rs"]
mod cva;
#[path = "insomnia/enrichment.rs"]
mod enrichment;
#[path = "insomnia/error.rs"]
mod error;
#[path = "insomnia/evidence.rs"]
mod evidence;
#[path = "insomnia/extraction.rs"]
mod extraction;
#[path = "insomnia/model.rs"]
mod model;
#[path = "insomnia/processor.rs"]
mod processor;
#[path = "insomnia/product_adapter.rs"]
pub(crate) mod product_adapter;
#[path = "insomnia/progress.rs"]
mod progress;
#[path = "insomnia/rebuild.rs"]
pub(crate) mod rebuild;
#[path = "insomnia/runtime_step.rs"]
pub(crate) mod runtime_step;
#[path = "insomnia/temporal.rs"]
pub(crate) mod temporal;
#[path = "insomnia/worker.rs"]
mod worker;

pub use backpressure::DEFAULT_INSOMNIA_BACKPRESSURE_DELAY_NS;
pub use error::InsomniaError;
pub use model::{
    EpisodeSchedulingResult, InsomniaAttempt, InsomniaLeaseToken, InsomniaPriority, InsomniaStats,
    InsomniaWork, InsomniaWorkState,
};
pub use processor::{InsomniaProcessError, InsomniaProcessResult};
pub(crate) use processor::{
    PreparedApplication, PreparedMemory, PreparedUserMemory, UserPublication,
};
pub use progress::{
    DEFAULT_INSOMNIA_PROGRESS_INTERVAL_SECS, InsomniaProgressEvent, InsomniaProgressReporter,
};
pub use reliquary_insomnia::{
    INSOMNIA_EXTRACTOR_CONTRACT_VERSION, INSOMNIA_OWNERSHIP_SYSTEM_PROMPT, INSOMNIA_SYSTEM_PROMPT,
    InsomniaCandidate, InsomniaEvidenceResult, InsomniaEvidenceRound, InsomniaEvidenceTurn,
    InsomniaExtraction, InsomniaExtractionError, InsomniaExtractionStage, InsomniaExtractor,
    InsomniaOwnership, InsomniaRejection, InsomniaRoutingMetadata, InsomniaSemanticStage,
    MAX_INSOMNIA_CANDIDATES, insomnia_schema,
};
pub use reliquary_insomnia::{
    MAX_INSOMNIA_EVIDENCE_BYTES, MAX_INSOMNIA_EVIDENCE_REQUESTS, MAX_INSOMNIA_EVIDENCE_TURNS,
};
pub(crate) use runtime_step::RuntimeInsomniaClaim;
pub use worker::{
    DEFAULT_INSOMNIA_LEASE_NS, DEFAULT_INSOMNIA_MAX_ATTEMPTS, DEFAULT_INSOMNIA_POLL_NS,
    DEFAULT_INSOMNIA_RETRY_DELAY_NS, DEFAULT_INSOMNIA_SCOPE, DEFAULT_INSOMNIA_WORKERS,
    InsomniaDrainResult, InsomniaWorkerConfig, InsomniaWorkerError, MAX_INSOMNIA_WORKERS,
};

#[cfg(feature = "entity-calibration")]
pub fn enrich_entity_calibration(
    endpoint: &dyn crate::GeneralEndpoint,
    candidates: &mut [InsomniaCandidate],
) -> Result<(), InsomniaExtractionError> {
    enrichment::enrich(endpoint, candidates)
}

#[cfg(test)]
#[path = "insomnia/completion_tests.rs"]
pub(crate) mod completion_tests;
#[cfg(test)]
#[path = "insomnia/enrichment_tests.rs"]
mod enrichment_tests;
#[cfg(test)]
#[path = "insomnia/evidence_flow_tests.rs"]
mod evidence_flow_tests;
#[cfg(test)]
#[path = "insomnia/evidence_tests.rs"]
mod evidence_tests;
#[cfg(test)]
#[path = "insomnia/extraction_tests.rs"]
mod extraction_tests;
#[cfg(test)]
#[path = "insomnia/gold_tests.rs"]
mod gold_tests;
#[cfg(test)]
#[path = "insomnia/grounding_tests.rs"]
mod grounding_tests;
#[cfg(test)]
#[path = "insomnia/ownership_tests.rs"]
mod ownership_tests;
#[cfg(test)]
#[path = "insomnia/queue_recovery_tests.rs"]
mod queue_recovery_tests;
#[cfg(test)]
#[path = "insomnia/queue_tests.rs"]
mod queue_tests;
#[cfg(test)]
#[path = "insomnia/scheduler_tests.rs"]
mod scheduler_tests;
#[cfg(test)]
#[path = "insomnia/temporal_tests.rs"]
mod temporal_tests;
#[cfg(test)]
#[path = "insomnia/test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "insomnia/worker_backpressure_tests.rs"]
mod worker_backpressure_tests;
#[cfg(test)]
#[path = "insomnia/worker_tests.rs"]
mod worker_tests;
#[cfg(test)]
#[path = "insomnia/worker_vector_tests.rs"]
mod worker_vector_tests;

#[cfg(test)]
#[path = "insomnia/completion_compact_tests.rs"]
mod completion_compact_tests;
