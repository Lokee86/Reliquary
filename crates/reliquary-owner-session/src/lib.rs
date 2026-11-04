//! Typed, read-only composition of the current REL and PHY owners.
//!
//! The aggregate keeps individual stores private. Replayed graph endpoints,
//! accepted episodes, and Dream first-cycle populations require explicit
//! admissions supplied by their owning domains; no admissions are synthesized.

pub use reliquary_graph::GraphAdmission;
pub use reliquary_insomnia_store::AcceptedEpisodeOrder;
pub use reliquary_memory_core::MemoryId;
pub mod records;
mod session;
pub use reliquary_insomnia_store::{CompletionCheck, InsomniaLeaseToken};
pub use session::{
    OwnerSessionError, PhySession, PhyWriter, RelSession, RelWriter, RoutedUserMemoryProof,
};
