use crate::{EntityGlobalId, EntityId};
use sha2::{Digest, Sha256};

pub const PRINCIPAL_ENTITY_KIND: &str = "principal";

pub fn principal_entity_id(principal_id: &str) -> EntityId {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-principal-entity-v1\0");
    hash.update((principal_id.len() as u64).to_le_bytes());
    hash.update(principal_id.as_bytes());
    EntityId(hash.finalize().into())
}

pub fn principal_entity_global_id(principal_id: &str) -> EntityGlobalId {
    let uuid = uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        format!("reliquary-principal-entity:{principal_id}").as_bytes(),
    );
    EntityGlobalId(*uuid.as_bytes())
}

pub fn is_phy_principal_id(value: &str) -> bool {
    value
        .strip_prefix("phy-")
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .is_some()
}
