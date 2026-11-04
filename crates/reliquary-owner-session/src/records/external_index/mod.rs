//! Disposable external-source index snapshots, owner-bound by their REL
//! container. Canonical Archive nodes authenticate indexed message identity;
//! index contents remain hints and must be rehydrated and authorized at use.
pub mod compact_index;
pub mod model;
pub mod store;

pub use model::{
    ExternalChunkRef, ExternalMessageDescriptor, ExternalMessageRef, ExternalSourceError,
};
pub use store::{ExternalIndexStore, IndexedChunk, IndexedMessage, SourceState};

#[cfg(test)]
mod tests;
