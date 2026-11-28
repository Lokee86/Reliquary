//! Immutable owner read models and pure value helpers admitted to workflows.
//!
//! This is not a mutable store facade. Container handles, writer leases,
//! concrete store access, and replay/repair entry points remain owner-local.

pub use reliquary_archive::{
    Branch, ContentId, CopiedSourceChange, CopiedSourceChangeReceipt, CopiedSourceChangeRequest,
    CopiedSourceHead, EchoEvent, EchoEventKind, Episode, EpisodeBoundary, EpisodeId, EpisodeOrigin,
    FragmentPassage, HERMES_COPIED_SOURCE_FORMAT, ImportReceipt, ImportedArtifact, ImportedBranch,
    ImportedConversation, ImportedMessage, ImportedSourceAuthor, ProjectAttachmentInput,
    ProjectFileRef, ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef, ResolvedTurn,
    SourceAttachment, TurnInput, validate_project_file_reference,
};
/// Activity correlation used for copied Hermes model-round delimiters in Echo.
pub const HERMES_MODEL_ROUND_ECHO: &str = "hermes:model_round";

pub use reliquary_freshness_store::FRESHNESS_POLICY_VERSION;
pub use reliquary_graph::{
    CommunityId, CommunityRename, CommunityRenameReceipt, validate_community_name,
};
pub use reliquary_graph::{NodeRef, Relation, RelationChange, RelationKind, RelationOrigin};
pub use reliquary_insomnia_store::{EpisodeId as InsomniaEpisodeId, InsomniaWork};
pub use reliquary_memory_core::{
    MemoryBody, MemoryBodyId, MemoryDraft, MemoryError, MemoryHistory, MemoryId, MemoryRecord,
    MemoryRevision, MemorySourceRef, MemoryTemporalInference, PayloadState, TemporalIndicationKind,
    TemporalInference, TemporalInferenceResolution, TerminalTombstone, memory_body_id, memory_id,
};
pub use reliquary_perception_store::{
    Entity, EntityId, EntityTombstone, MAX_MENTION_CANDIDATES, MemoryEntityMentionKey,
    MemoryEntityResolution, MemoryTextField, Relationship, RelationshipEntityRef, RelationshipId,
    RelationshipMemoryRef, RelationshipParticipant, RelationshipTombstone,
};
pub use reliquary_vector_store::{CompatibilityProfile, CompatibilityProfileId, EmbeddingMode};

pub use reliquary_perception_store::{EntityResolutionReason, MemoryEntityResolutionStatus};

pub mod identity;
pub use reliquary_perception_store::identity_evidence;
