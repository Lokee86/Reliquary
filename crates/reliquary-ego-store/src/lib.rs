pub mod ego_codec;
pub mod ego_compact_apply;
pub mod ego_compact_contract;
mod ego_error;
pub mod ego_model;
pub mod ego_store;
pub use ego_error::EgoError;
pub use ego_model::*;
pub use ego_store::EgoStore;

#[cfg(test)]
mod ego_compact_contract_tests;
#[cfg(test)]
mod ego_compact_creation_codec_tests;
#[cfg(test)]
mod ego_compact_delta_codec_tests;
#[cfg(test)]
mod ego_compact_replay_tests;
#[cfg(test)]
mod ego_current_fixture;
