//! Typed, read-only composition of the current REL and PHY owners.
//!
//! The aggregate keeps individual stores private. Replayed graph endpoints,
//! accepted episodes, and Dream first-cycle populations require explicit
//! admissions supplied by their owning domains; no admissions are synthesized.

pub use reliquary_graph::GraphAdmission;
pub use reliquary_insomnia_store::AcceptedEpisodeOrder;
pub use reliquary_memory_core::MemoryId;
pub mod model;
pub mod records;
pub use records::{DreamBeginAdmission, VerifiedOwnerCopyPlan, VerifiedOwnerCopyReceipt};
mod session;
/// Bound for durable Insomnia inference checkpoints, owned by the session store.
pub use reliquary_insomnia_store::MAX_HELD_PAYLOAD_BYTES as MAX_INSOMNIA_CHECKPOINT_BYTES;
pub use reliquary_insomnia_store::{CompletionCheck, InsomniaLeaseToken};
pub use session::{
    OwnerSessionError, PhySession, PhyWriter, RelInsomniaClaimInput, RelSession, RelWriter,
    RoutedUserMemoryProof,
};
