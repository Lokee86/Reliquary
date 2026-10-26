//! Durable Perception owners; product authorization and inference stay above this crate.
pub mod entity_resolution_codec;
pub mod entity_resolution_model;
pub mod entity_resolution_store;
pub use entity_resolution_model::*;
pub use resolution_error::ResolutionError;
mod resolution_error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct EntityId(pub [u8; 32]);
