use super::FreshnessStore;
use serde::{Deserialize, Serialize};

pub(crate) const ACTIVATION_MAGIC: &[u8; 8] = b"CVAFSA01";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticActivation {
    pub owner_uuid: [u8; 16],
    pub boundary: Option<(String, String)>,
    pub accepted_turn: u64,
    pub policy_version: u16,
    pub policy_digest: [u8; 32],
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActivationRecord {
    owner_uuid: [u8; 16],
    activation: Option<SemanticActivation>,
}

impl FreshnessStore {
    pub(crate) fn semantic_activation(&self) -> Option<&SemanticActivation> {
        self.semantic_activation.as_ref()
    }

    pub(crate) fn semantic_activation_seen(&self) -> bool {
        self.semantic_activation_seen
    }

    pub(crate) fn birth_turn(&self, id: crate::MemoryId) -> Option<u64> {
        self.births.get(&id).map(|(turn, _)| *turn)
    }

    pub(crate) fn ingest_semantic_activation(&mut self, payload: &[u8]) -> Result<(), String> {
        let record: ActivationRecord = serde_json::from_slice(&payload[8..])
            .map_err(|e| format!("invalid semantic activation: {e}"))?;
        if self.owner_uuid != Some(record.owner_uuid) {
            return Err("semantic activation belongs to another owner".into());
        }
        if let Some(a) = &record.activation {
            let policy = crate::freshness::semantic_policy::SemanticPolicy::default();
            if a.owner_uuid != record.owner_uuid
                || a.policy_version != policy.version
                || a.policy_digest != policy.digest().map_err(|e| e.to_string())?
                || (a.accepted_turn == 0) != a.boundary.is_none()
            {
                return Err("unsupported semantic activation evidence".into());
            }
        }
        if self.semantic_completed && self.semantic_activation != record.activation {
            return Err("semantic activation is immutable after completion".into());
        }
        self.semantic_activation = record.activation;
        self.semantic_activation_seen = true;
        Ok(())
    }

    pub(crate) fn persist_semantic_activation(
        &mut self,
        container: &mut crate::Container,
        owner_uuid: [u8; 16],
        activation: Option<SemanticActivation>,
    ) -> Result<(), String> {
        if self.semantic_activation == activation && self.semantic_activation_seen {
            return Ok(());
        }
        let mut payload = ACTIVATION_MAGIC.to_vec();
        payload.extend(
            serde_json::to_vec(&ActivationRecord {
                owner_uuid,
                activation,
            })
            .map_err(|e| e.to_string())?,
        );
        let mut staged = self.clone();
        staged.ingest_semantic_activation(&payload)?;
        self.persist_policy_if_needed(container, owner_uuid)?;
        container.append(&payload).map_err(|e| e.to_string())?;
        container.sync().map_err(|e| e.to_string())?;
        self.ingest_appended(&payload)
    }
}
