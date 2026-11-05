//! Durable owner receipts for reference-only external source manifests.
//!
//! These records preserve host message identity and content hashes only. They
//! never create Archive turns or persist hydrated transcript text.

mod codec;
mod model;
mod store;

#[cfg(test)]
use super::external_index::ExternalMessageDescriptor;
#[cfg(test)]
use super::external_index::model::EXTERNAL_SOURCE_CONTRACT;
#[cfg(test)]
use codec::{encode, make_receipt};
pub use model::{ExternalReferenceSnapshotManifest, ExternalReferenceSnapshotReceipt};
pub(crate) use store::ExternalReferenceStore;

#[cfg(test)]
#[path = "external_reference_tests.rs"]
mod tests;
