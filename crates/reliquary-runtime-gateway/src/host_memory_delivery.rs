use crate::{GatewayError, RuntimeCapability};
use serde::{Deserialize, Serialize};

/// Stable, content-bound host delivery intent. Hosts must make `stable_id`
/// idempotent and report confirmed consumption only after content is handed to
/// the authenticated agent turn.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MemoryDeliveryOperation {
    pub stable_id: String,
    pub payload_digest: [u8; 32],
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryDeliveryReceipt {
    pub stable_id: String,
    pub payload_digest: [u8; 32],
    pub confirmed_consumption: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryDeliveryStatus {
    Confirmed(MemoryDeliveryOperation),
    Pending,
    Rejected,
    NotFound,
    Unknown,
}

pub(crate) fn require_delivery_contract(
    router: &crate::CapabilityRouter,
    binding: usize,
) -> Result<(), GatewayError> {
    if !router.belongs_to(binding) {
        return Err(GatewayError::HostAdapter(
            "host connection belongs to another Gateway".into(),
        ));
    }
    if router.state(RuntimeCapability::MemoryDelivery) != crate::HostCapabilityState::Ready
        || !router
            .contract(RuntimeCapability::MemoryDelivery)
            .is_some_and(|contract| contract.queue.complete())
    {
        return Err(GatewayError::IncompleteHostContract(
            RuntimeCapability::MemoryDelivery,
        ));
    }
    Ok(())
}
