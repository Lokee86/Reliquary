//! Narrow, source-bound Entity/Relationship and Memory lifecycle commands.

use super::*;

mod entity;
mod memory;
pub(super) use memory::validate_retirement;
mod mention;
mod relationship;
mod requests;
mod support;
mod writer;

pub use requests::{
    CreateRelationshipRequest, EntityRetirementRequest, MemoryRetirementRequest,
    RelationshipParticipantEvidence, RelationshipRetirementRequest, WithdrawEntityMentionRequest,
};

#[cfg(test)]
#[path = "perception_lifecycle/tests.rs"]
mod tests;
