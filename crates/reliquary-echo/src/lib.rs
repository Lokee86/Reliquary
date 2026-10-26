//! Durable, source-keyed Echo records stored in REL containers.
//!
//! Echo owns historical reasoning and tool evidence. It does not grant access,
//! render context, or interpret provider continuation state.

mod echo_codec;
mod echo_model;
mod echo_store;

pub use echo_model::{EchoError, EchoEvent, EchoEventKind};
pub use echo_store::EchoStore;

#[cfg(test)]
mod echo_store_tests;
