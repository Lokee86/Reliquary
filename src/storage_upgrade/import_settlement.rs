//! One-time conversion of validated physical import proofs to semantic authority.
use super::StorageUpgradeError;
use crate::freshness_storage::HistoricalImportFreshnessSettlement;
use crate::{Container, Cva, ObjectRef};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct Plan {
    pub replacements: BTreeMap<ObjectRef, Vec<u8>>,
    pub omitted: BTreeSet<ObjectRef>,
}

pub(super) fn prepare(
    source: &mut Container,
    owner: &mut Cva,
) -> Result<Plan, StorageUpgradeError> {
    // Validate against the untouched source, including its physical prefix and
    // completion bindings, before discarding any compatibility evidence.
    owner.validate_historical_import_settlements()?;
    owner.validate_import_completion_settlements()?;
    let mut canonical = BTreeMap::new();
    for old in owner.freshness.import_settlements() {
        let mut scope = owner.semantic_scope_for_import_settlement(old)?;
        scope.predecessor_digest = None;
        let current = HistoricalImportFreshnessSettlement::from_semantic(old.as_semantic(scope));
        canonical.insert(old.import_id, current.encode());
    }
    let digests = canonical
        .iter()
        .map(|(id, bytes)| {
            let id: String = id.iter().map(|byte| format!("{byte:02x}")).collect();
            (id, format!("{:x}", Sha256::digest(bytes)))
        })
        .collect();
    let mut plan = Plan::default();
    let mut emitted = BTreeSet::new();
    source.visit_payloads::<StorageUpgradeError>(|object, payload| {
        let settlement = if payload.starts_with(b"CVAFRS03") {
            Some(HistoricalImportFreshnessSettlement::decode_legacy_frame(
                payload,
            ))
        } else if [b"CVAFRS04", b"CVAFRS05", b"CVAFRS06"]
            .iter()
            .any(|magic| payload.starts_with(*magic))
        {
            Some(HistoricalImportFreshnessSettlement::decode(payload))
        } else {
            None
        };
        if let Some(settlement) = settlement {
            let settlement = settlement.map_err(StorageUpgradeError::Migration)?;
            let bytes = canonical.get(&settlement.import_id).ok_or_else(|| {
                StorageUpgradeError::Migration("unaccepted import settlement in source".into())
            })?;
            if emitted.insert(settlement.import_id) {
                plan.replacements.insert(object, bytes.clone());
            } else {
                plan.omitted.insert(object);
            }
        } else if payload.starts_with(b"CVAILEG1") || payload.starts_with(b"CVAILEG2") {
            plan.omitted.insert(object);
        } else if let Some(bytes) =
            crate::imports::transport_completion_settlement(payload, &digests)?
        {
            plan.replacements.insert(object, bytes);
        }
        Ok(())
    })?;
    if emitted.len() != canonical.len() {
        return Err(StorageUpgradeError::Migration(
            "accepted settlement has no source frame".into(),
        ));
    }
    Ok(plan)
}

#[cfg(test)]
#[path = "import_settlement_tests.rs"]
mod tests;
