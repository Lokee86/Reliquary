//! Observation discovery, synthesis, receptors, and native decision contracts.

mod claim;
mod contribution;
mod general;
mod native;
mod proposal;
mod receptors;
mod synthesis;
mod verification;

pub use claim::{ClaimParticipant, ObservationClaim, ObservationProfile, Status, TimeScope};
pub use contribution::{
    CONTRIBUTION_POLICY_VERSION, ContributionDecision, ContributionHitDecision, ContributionState,
    ScopeChoice, TemporalChoice, contribute,
};
pub use general::{StageError, StageLimits};
pub use native::{DecisionDiagnostics, HitVerdict, NativeDecisionError};
pub use proposal::{ObservationProposal, PerceptionContract};
pub use receptors::{
    RECEPTOR_POLICY_VERSION, ReceptorBinding, ReceptorCategory, ReceptorDescription, ReceptorSet,
    generate_receptors, receptor_fingerprint,
};
pub use reliquary_switchboard::OBSERVATION_DECISION_MODEL as NATIVE_DECISION_MODEL;
pub use synthesis::{
    ExistingObservation, ExistingObservationRef, RevisionContext, RevisionEvidence,
    SYNTHESIS_POLICY_VERSION, StageBinding, SynthesisDecision, SynthesisDraft, SynthesisRequest,
    synthesize,
};
pub use verification::{
    EvidencePassage, VERIFICATION_POLICY_VERSION, VerificationDecision, VerificationState,
    VerificationVerdict, verify,
};
