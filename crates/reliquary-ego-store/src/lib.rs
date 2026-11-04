//! Durable owner-local Ego state over REL/PHY container frames.
mod model;
mod store;
pub use model::*;
pub use store::{EgoError, EgoOwnerKind, EgoStore};

#[cfg(test)]
mod tests;
