//! Durable owner-local Ego state over REL/PHY container frames.
mod model;
mod store;
mod summary;
pub use model::*;
pub use store::{EgoError, EgoOwnerKind, EgoStore};
pub use summary::*;

#[cfg(test)]
mod tests;
