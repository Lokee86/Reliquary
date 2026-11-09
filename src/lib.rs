//! Supported 2.1 product API for Reliquary owners and trusted hosts.
//!
//! The public boundary deliberately exposes opaque owner handles and immutable
//! snapshots. Mutable containers and semantic stores remain internal.

mod configuration;
mod configured_runtime;
mod error;
mod host;
mod maintenance;
mod management;
mod native_import;
mod owner;
mod processing;
mod retrieval;

pub use configuration::*;

pub use configured_runtime::{
    CompatibilityProbeReference, CompatibilityProfile, ConfiguredRuntime, EmbeddingMode,
    RuntimeEndpointFailure, VectorNormalization,
};
pub use error::{ProductError, ProductErrorKind};
pub use host::{
    ConversationQueue, Gateway, HostAdmissionAdapter, HostAdmissionControl, HostCredentialBinding,
    Instance, OwnerGrant, TrustedHostAdmission,
};
pub use host::{context::*, external_source::*, project_source::*};
pub use maintenance::*;
pub use management::*;
pub use native_import::*;
pub use owner::{OwnerId, OwnerInfo, OwnerKind, OwnerScope, Phylactery, Rel};
pub use processing::{
    FreshnessLifecycleState, FreshnessTransition, ImportStage, InsomniaProgress,
    LiveProcessingSelection, LiveStageProgress, MAX_PROCESS_EPISODES, OwnerProcessingJob,
    ProcessOptions, ProcessingReport, ProcessingState, StageProgress, StageState, VectorCoverage,
};
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub use reliquary_switchboard::test_support;
pub use retrieval::*;

pub use reliquary_application::{
    ConversationInspection, MAX_MANAGEMENT_PAGE_BYTES, MAX_MANAGEMENT_PAGE_ITEMS, ManagedBranch,
    ManagementPage, ManagementVersions, MemoryProvenance, MemoryStatistics, MemorySummary,
    RelManagementHealth, SemanticOwnerStatistics, TranscriptTurn,
};
pub use reliquary_archive::{EpisodeBoundary, EpisodeId, HERMES_COPIED_SOURCE_FORMAT};
pub use reliquary_memory_core::{MemoryId, PayloadState};
pub use reliquary_runtime_gateway::{
    ArchiveReceipt, CapabilityRouter, ConversationSubmissionId, DurableGeneration,
    DurableGenerationCheckpoint, DurableGenerationPhase, DurableHostOperation,
    DurableHostOperationPhase, EnqueueOutcome, FinalizeCapture, GenerationReplay,
    GenerationStreamDelivery, GenerationStreamEvent, GenerationStreamPayload,
    GenerationStreamSnapshot, GenerationStreamSubscription, HandoffReceipt, HostCancelOutcome,
    HostCapabilityContract, HostCapabilityState, HostConnection, HostDispatchOutcome,
    HostExecutionMode, HostOperation, HostOperationStatus, HostQueueGuarantees,
    HostReconcileStatus, MemoryDeliveryOperation, MemoryDeliveryReceipt, MemoryDeliveryStatus,
    QueueError, QueueState, QueuedSubmission, RuntimeCapability, SubmissionId, SubmissionStatus,
    TrustedHostAdapter,
};
