use reliquary_owner_session::model::{
    CompatibilityProfile, CompatibilityProfileId, Entity, EntityId, MemoryBodyId,
};
use reliquary_owner_session::observation::{
    ObservationCitation, ObservationComparison, ObservationComparisonOutcome, ObservationDraft,
    ObservationId, ObservationInspection,
};
use reliquary_owner_session::{
    DreamCandidatePool, ManagementRead, MemoryId, OwnerSessionError, PerceptionMemoryEvidence,
    PerceptionMemoryMetadata, PerceptionMemorySource, PerceptionParticipantEvidence,
    PerceptionSourceCut, PhyWriter, RelSession,
};

/// A borrowed owner view supplied by the authorized Perception worker.
#[derive(Clone, Copy)]
pub enum ObservationRead<'a> {
    Rel(&'a RelSession),
    Phy(&'a PhyWriter),
}

macro_rules! shared_reads {
    ($(fn $name:ident($($arg:ident: $ty:ty),*) -> $output:ty;)*) => {
        $(pub(crate) fn $name(self, $($arg: $ty),*) -> $output {
            match self {
                Self::Rel(rel) => rel.$name($($arg),*),
                Self::Phy(phy) => phy.$name($($arg),*),
            }
        })*
    };
}

impl<'a> ObservationRead<'a> {
    shared_reads! {
        fn perception_source_cut() -> PerceptionSourceCut;
        fn perception_source_metadata(cut: &PerceptionSourceCut, ids: &[MemoryId]) -> Result<Vec<PerceptionMemoryMetadata>, OwnerSessionError>;
        fn perception_memory_evidence(cut: &PerceptionSourceCut, ids: &[MemoryId]) -> Result<Vec<PerceptionMemoryEvidence>, OwnerSessionError>;
        fn perception_participant_evidence(cut: &PerceptionSourceCut, source: PerceptionMemorySource) -> Result<Vec<PerceptionParticipantEvidence>, OwnerSessionError>;
        fn perception_graph_neighbors(id: MemoryId) -> Vec<MemoryId>;
        fn perception_lexical_candidates(query: &str, limit: usize) -> Vec<MemoryId>;
        fn entity(id: EntityId) -> Option<&'a Entity>;
        fn active_compatibility_profile(id: CompatibilityProfileId) -> Option<&'a CompatibilityProfile>;
        fn dream_candidate_pool(id: MemoryId, profile: CompatibilityProfileId, row: &[f32]) -> Result<DreamCandidatePool, OwnerSessionError>;
        fn memory_vector_row(profile: CompatibilityProfileId, body: MemoryBodyId) -> Result<Option<Vec<f32>>, OwnerSessionError>;
        fn observation(id: ObservationId) -> Option<ObservationInspection<'a>>;
        fn observations_after(after: Option<ObservationId>, limit: usize) -> Vec<ObservationInspection<'a>>;
        fn observation_comparison(comparison: &ObservationComparison) -> Option<ObservationComparisonOutcome>;
        fn observation_citation_text(cite: &ObservationCitation) -> Result<String, OwnerSessionError>;
        fn observation_revision_source_eligible(id: ObservationId, revision: u64) -> bool;
        fn matching_observation(draft: &ObservationDraft) -> Option<ObservationId>;
        fn has_equivalent_manual_relationship(draft: &ObservationDraft) -> bool;
    }

    pub(crate) fn management_read(self) -> Result<ManagementRead<'a>, OwnerSessionError> {
        match self {
            Self::Rel(rel) => Ok(rel.management_read()),
            Self::Phy(phy) => phy.management_read(),
        }
    }

    pub(crate) fn source_removal(self) -> u64 {
        match self {
            Self::Rel(rel) => rel.archive_source_removal_version(),
            Self::Phy(_) => 0,
        }
    }
}
