//! One-call native Jev contracts for Observation contribution and verification.

mod contribution;
mod native;
mod verification;

pub use contribution::{
    CONTRIBUTION_POLICY_VERSION, ContributionDecision, ContributionHitDecision, ContributionState,
    ScopeChoice, TemporalChoice, contribute,
};
pub use native::{DecisionDiagnostics, HitVerdict, NativeDecisionError};
pub use reliquary_switchboard::OBSERVATION_DECISION_MODEL as NATIVE_DECISION_MODEL;
pub use verification::{
    EvidencePassage, VERIFICATION_POLICY_VERSION, VerificationDecision, VerificationState,
    VerificationVerdict, verify,
};
