//! Typed composition and mutation authority for the current REL and PHY owners.
//!
//! The aggregate keeps individual stores private. Replayed graph endpoints,
//! accepted Episodes are derived from durable owners; Dream first-cycle
//! populations still require externally supplied admission evidence.

pub use reliquary_graph::GraphAdmission;
pub use reliquary_insomnia_store::AcceptedEpisodeOrder;
pub use reliquary_memory_core::MemoryId;
pub mod model;
pub mod records;
pub use records::{
    DreamBeginAdmission, DreamMemoryCut, DreamNoLinkCut, VerifiedOwnerCopyPlan,
    VerifiedOwnerCopyReceipt,
};
mod session;
/// Bound for durable Insomnia inference checkpoints, owned by the session store.
pub use reliquary_insomnia_store::MAX_HELD_PAYLOAD_BYTES as MAX_INSOMNIA_CHECKPOINT_BYTES;
pub use reliquary_insomnia_store::{CompletionCheck, InsomniaLeaseToken};
pub use session::{
    OwnerSessionError, PhySession, PhyWriter, RelInsomniaClaimInput, RelSession, RelWriter,
    RoutedUserMemoryProof,
};
