use crate::{EntityGlobalId, EntityId};
use sha2::{Digest, Sha256};

pub(crate) const PRINCIPAL_ENTITY_KIND: &str = "principal";

pub(crate) fn principal_entity_id(principal_id: &str) -> EntityId {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-principal-entity-v1\0");
    hash.update((principal_id.len() as u64).to_le_bytes());
    hash.update(principal_id.as_bytes());
    EntityId(hash.finalize().into())
}

pub(crate) fn principal_entity_global_id(principal_id: &str) -> EntityGlobalId {
    let uuid = uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        format!("reliquary-principal-entity:{principal_id}").as_bytes(),
    );
    EntityGlobalId(*uuid.as_bytes())
}

pub(crate) fn is_owner_relative_principal_surface(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    let base = normalized
        .strip_suffix("'s")
        .or_else(|| normalized.strip_suffix("’s"))
        .unwrap_or(&normalized);
    matches!(base, "user" | "the user")
}

pub(crate) fn is_phy_principal_id(value: &str) -> bool {
    value
        .strip_prefix("phy-")
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .is_some()
}
