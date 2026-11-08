use super::*;
use reliquary_application::{
    AgenticMemory, RetrievalAction, RetrievalEvaluation, RetrievalTurn, evaluate_retrieval,
};
use reliquary_owner_session::{FreshnessEventProof, FreshnessMemoryId, model::MemoryId};
use reliquary_switchboard::{
    DecisionConsumer, DecisionEndpoint, DecisionEndpointError, GeneralEndpoint,
    ModelSwitchboardConfig, resolve_decision_endpoint,
};
use serde::Deserialize;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// Opaque proposal bound to one authenticated Instance, REL, turn, and exact
/// body revisions. It grants no delivery or Freshness authority by itself.
pub struct PreparedMemoryDelivery {
    pub(super) instance_id: Uuid,
    pub(super) principal: String,
    pub(super) owner: OwnerId,
    pub(super) turn_id: String,
    pub(super) controller_id: Uuid,
    pub(super) use_id: String,
    pub(super) digest: [u8; 32],
    pub(super) payload: Vec<u8>,
    pub(super) evaluation: RetrievalEvaluation,
    pub(super) proof: FreshnessEventProof,
    pub(super) accepted_turn: u64,
    pub(super) body_bindings: Vec<(
        reliquary_owner_session::model::MemoryId,
        u64,
        reliquary_owner_session::model::MemoryBodyId,
    )>,
}

impl PreparedMemoryDelivery {
    /// Owner bound to this exact prepared payload.
    pub fn owner_id(&self) -> OwnerId {
        self.owner
    }

    /// Stable trusted-host operation key for reconciliation and recovery.
    /// The opaque result remains the only authority to deliver its contents.
    pub fn stable_id(&self) -> &str {
        &self.use_id
    }
}

pub struct RetrievalOutcome {
    pub prepared: Option<PreparedMemoryDelivery>,
    pub already_provided: BTreeSet<reliquary_owner_session::model::MemoryId>,
    pub anchors: Vec<reliquary_application::RetrievalAnchorReport>,
}

/// The composed MemoryRetrieval route. Resolution follows the existing
/// dedicated → shared decision → adapted general → unavailable hierarchy.
pub struct MemoryRetrievalRoute(Arc<dyn DecisionEndpoint>);
impl MemoryRetrievalRoute {
    pub fn resolve(
        config: &ModelSwitchboardConfig,
        dedicated: Option<Arc<dyn DecisionEndpoint>>,
        shared: Option<Arc<dyn DecisionEndpoint>>,
        general: Option<Arc<dyn GeneralEndpoint>>,
    ) -> Result<Option<Self>, DecisionEndpointError> {
        Ok(resolve_decision_endpoint(
            config,
            DecisionConsumer::MemoryRetrieval,
            dedicated,
            shared,
            general,
        )?
        .map(Self))
    }
}

pub(super) struct TurnState {
    pub(super) turn: reliquary_application::RetrievalTurn,
    pub(super) generation: u64,
    pub(super) pending_use_id: Option<String>,
}

/// Host-owned local state for one external conversational turn. It is opaque,
/// bound to the creating Instance, and drops without Gateway-side registration.
pub struct MemoryRetrievalTurn {
    pub(super) instance_id: Uuid,
    pub(super) principal: String,
    pub(super) owner: OwnerId,
    pub(super) turn_id: String,
    pub(super) controller_id: Uuid,
    pub(super) state: std::sync::Mutex<TurnState>,
}

impl MemoryRetrievalTurn {
    /// Owner bound to this authenticated retrieval turn.
    pub fn owner_id(&self) -> OwnerId {
        self.owner
    }

    /// Stable caller turn identifier used by the host delivery manifest.
    pub fn turn_id(&self) -> &str {
        &self.turn_id
    }
}

#[derive(Serialize)]
struct DeliveredMemory<'a> {
    pub(super) owner_id: String,
    pub(super) memory_id: String,
    pub(super) revision: u64,
    pub(super) body_id: String,
    pub(super) title: &'a str,
    pub(super) content: &'a str,
}
#[derive(Serialize)]
struct DeliveryPayload<'a> {
    pub(super) principal: &'a str,
    pub(super) owner_id: String,
    pub(super) turn_id: &'a str,
    pub(super) accepted_turn: u64,
    pub(super) graph_version: u64,
    pub(super) community_generation: Option<u64>,
    pub(super) community_graph_version: Option<u64>,
    pub(super) query: Option<&'a str>,
    pub(super) searched_ids: Vec<String>,
    pub(super) selected_ids: Vec<String>,
    pub(super) memories: Vec<DeliveredMemory<'a>>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DeliveryManifest {
    pub(super) principal: String,
    pub(super) owner_id: String,
    pub(super) turn_id: String,
    pub(super) accepted_turn: u64,
    pub(super) graph_version: u64,
    pub(super) community_generation: Option<u64>,
    pub(super) community_graph_version: Option<u64>,
    pub(super) query: Option<String>,
    pub(super) searched_ids: Vec<String>,
    pub(super) selected_ids: Vec<String>,
    pub(super) memories: Vec<ManifestMemory>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestMemory {
    pub(super) owner_id: String,
    pub(super) memory_id: String,
    pub(super) revision: u64,
    pub(super) body_id: String,
    pub(super) title: String,
    pub(super) content: String,
}

#[path = "instance_retrieval_delivery.rs"]
mod delivery;
#[path = "instance_retrieval_prepare.rs"]
mod prepare;
#[path = "instance_retrieval_recovery.rs"]
mod recovery;
#[cfg(test)]
#[path = "instance_retrieval/tests.rs"]
mod tests;
fn memories_for_bindings(
    memories: &[AgenticMemory],
    ids: &BTreeSet<reliquary_owner_session::model::MemoryId>,
) -> Vec<(
    reliquary_owner_session::model::MemoryId,
    u64,
    reliquary_owner_session::model::MemoryBodyId,
)> {
    memories
        .iter()
        .filter(|m| ids.contains(&m.id))
        .map(|m| (m.id, m.revision, m.body_id))
        .collect()
}
pub(super) fn validate_prepared(
    instance: &Instance,
    turn: &MemoryRetrievalTurn,
    prepared: &PreparedMemoryDelivery,
) -> Result<(), GatewayError> {
    validate_turn_controller(instance, turn, prepared.owner, &prepared.turn_id)?;
    if prepared.instance_id != instance.instance_id
        || prepared.principal != instance.principal.as_ref()
        || prepared.controller_id != turn.controller_id
    {
        return Err(GatewayError::InvalidPreparedRetrieval);
    }
    instance.ensure_credential_active()?;
    if !instance.rel_grants.contains(&prepared.owner) {
        return Err(GatewayError::UnauthorizedRel {
            principal: instance.principal.to_string(),
            owner: prepared.owner,
        });
    }
    Ok(())
}
pub(super) fn validate_turn_binding(
    instance: &Instance,
    turn: &MemoryRetrievalTurn,
    owner: OwnerId,
    turn_id: &str,
) -> Result<bool, GatewayError> {
    validate_turn_controller(instance, turn, owner, turn_id)?;
    instance.ensure_credential_active()?;
    if !instance.rel_grants.contains(&owner) {
        return Err(GatewayError::UnauthorizedRel {
            principal: instance.principal.to_string(),
            owner,
        });
    }
    Ok(true)
}
pub(super) fn validate_turn_controller(
    instance: &Instance,
    turn: &MemoryRetrievalTurn,
    owner: OwnerId,
    turn_id: &str,
) -> Result<(), GatewayError> {
    if turn.instance_id != instance.instance_id
        || turn.principal != instance.principal.as_ref()
        || turn.owner != owner
        || turn.turn_id != turn_id
    {
        return Err(GatewayError::InvalidPreparedRetrieval);
    }
    Ok(())
}
pub(super) fn validate_turn_id(turn_id: &str) -> Result<(), GatewayError> {
    if turn_id.trim().is_empty() || turn_id.len() > 512 || turn_id.chars().any(char::is_control) {
        Err(GatewayError::InvalidPreparedRetrieval)
    } else {
        Ok(())
    }
}
pub(super) fn parse_memory_id(value: &str) -> Result<MemoryId, GatewayError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(GatewayError::InvalidHandoffReceipt);
    }
    let mut id = [0u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let hex = |byte: u8| match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => 255,
        };
        let high = hex(pair[0]);
        let low = hex(pair[1]);
        if high > 15 || low > 15 {
            return Err(GatewayError::InvalidHandoffReceipt);
        }
        id[index] = (high << 4) | low;
    }
    Ok(MemoryId(id))
}
pub(super) fn stable_memory_use_id(
    owner: OwnerId,
    principal: &str,
    turn: &str,
    digest: [u8; 32],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(owner.as_bytes());
    hasher.update((principal.len() as u64).to_le_bytes());
    hasher.update(principal.as_bytes());
    hasher.update((turn.len() as u64).to_le_bytes());
    hasher.update(turn.as_bytes());
    hasher.update(digest);
    format!("agentic-use-{}", hex(&hasher.finalize()))
}
