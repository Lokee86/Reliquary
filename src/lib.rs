//! Supported 2.1 product API for Reliquary owners and trusted hosts.
//!
//! The public boundary deliberately exposes opaque owner handles and immutable
//! snapshots. Mutable containers and semantic stores remain internal.

mod error;
mod host;
mod maintenance;
mod management;
mod owner;

pub use error::{ProductError, ProductErrorKind};
pub use host::{
    ConversationQueue, Gateway, HostAdmissionAdapter, HostAdmissionControl, HostCredentialBinding,
    InsomniaWorkerConfig, InsomniaWorkerHandle, Instance, MemoryRetrievalTurn, OwnerGrant,
    PreparedMemoryDelivery, RetrievalOutcome, TrustedHostAdmission,
};
pub use maintenance::*;
pub use management::WorkspaceManagement;
pub use owner::{OwnerId, OwnerInfo, OwnerKind, OwnerScope, Phylactery, Rel};

pub use reliquary_application::native_import_workflow::{
    NativeImportBatch, NativeImportEpisode, StagedNativeImport,
};
pub use reliquary_application::{
    AgenticMemory, MemoryRetrievalConfig, MemoryRetrievalResult, OwnerMemoryEvidence,
    OwnerMemorySelection, RetrievalAction, RetrievalAnchorReport, RetrievalConfig,
    RetrievalDirection, RetrievalEvaluation, RetrievalRelationEvidence, RetrievalWorkflowError,
    SelectedMemory,
};
pub use reliquary_application::{
    ConversationInspection, MAX_MANAGEMENT_PAGE_BYTES, MAX_MANAGEMENT_PAGE_ITEMS, ManagedBranch,
    ManagementPage, ManagementVersions, MemoryProvenance, MemoryStatistics, MemorySummary,
    RelManagementHealth, SemanticOwnerStatistics, TranscriptTurn,
};
pub use reliquary_memory_core::{MemoryId, PayloadState};
pub use reliquary_owner_session::model::{
    Branch, CompatibilityProfileId, EntityTombstone, EpisodeId, ImportReceipt,
    MemoryEntityResolution, Relationship, RelationshipTombstone, TerminalTombstone, TurnInput,
};
pub use reliquary_owner_session::records::ImportFreshnessSettlement;
pub use reliquary_owner_session::{
    CreateRelationshipRequest, EntityRetirementRequest, MemoryRetirementRequest,
    RelationshipRetirementRequest, WithdrawEntityMentionRequest,
};
pub use reliquary_runtime_gateway::{
    ArchiveReceipt, CapabilityRouter, ConversationSubmissionId, DurableGeneration,
    DurableGenerationCheckpoint, DurableGenerationPhase, DurableHostOperation,
    DurableHostOperationPhase, EnqueueOutcome, FinalizeCapture, GenerationReplay,
    GenerationStreamDelivery, GenerationStreamEvent, GenerationStreamPayload,
    GenerationStreamSnapshot, GenerationStreamSubscription, HandoffReceipt, HostCancelOutcome,
    HostCapabilityContract, HostCapabilityState, HostConnection, HostDispatchOutcome,
    HostExecutionMode, HostOperation, HostOperationStatus, HostQueueGuarantees,
    HostReconcileStatus, InsomniaWorkerState, MemoryDeliveryOperation, MemoryDeliveryReceipt,
    MemoryDeliveryStatus, MemoryRetrievalRoute, QueueState, QueuedSubmission, RuntimeCapability,
    SubmissionId, SubmissionStatus, TrustedHostAdapter,
};
pub use reliquary_switchboard::{
    DecisionEndpoint, DecisionEndpointError, GeneralEndpoint, GeneralEndpointError,
    ModelSwitchboardConfig,
};
