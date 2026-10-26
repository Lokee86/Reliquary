//! Test-only Entity revisions, direct terminals and merge replacement history.

use crate::EntityId;
use crate::entity_model::EntityRecord;
use crate::entity_store::EntityStore;
use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use reliquary_container::Container;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct EntityCut {
    pub(crate) fingerprint: HistoricalFingerprint,
    pub(crate) current: BTreeMap<EntityId, u64>,
    pub(crate) replacements: BTreeMap<EntityId, Option<EntityId>>,
}

pub(crate) fn entity_cut(
    container: &Container,
    store: &EntityStore,
    cut: u64,
) -> Result<EntityCut, String> {
    if cut > store.entity_version() {
        return Err("Entity fingerprint cut exceeds retained head".into());
    }
    let mut projection = OwnerProjection::new(
        container
            .owner_uuid()
            .ok_or("Entity oracle needs owner UUID")?,
        "entity",
        cut,
        None,
    )?;
    let mut current = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for record in store.records() {
        ids.insert(record.id);
        if record.entity_version > cut {
            continue;
        }
        projection.insert(
            "revision",
            record.entity_version.to_be_bytes(),
            revision_value(
                record,
                store
                    .pruned_revisions
                    .contains(&(record.id, record.revision)),
                container.transaction_time_ns(record.global_version),
            ),
        )?;
        current.insert(record.id, record.revision);
    }
    let mut replacements = BTreeMap::new();
    for id in ids {
        if let Ok(crate::owner_lifecycle::LifecycleState::Tombstoned(_)) = store.lifecycle_state(id)
        {
            let descriptor = store.retention_descriptor(id).map_err(|e| e.to_string())?;
            if descriptor.tombstone_entity_version > cut {
                continue;
            }
            projection.insert("terminal", id.0, json!({
                "id": hex(&id.0), "revision": descriptor.tombstone_revision,
                "entity_version": descriptor.tombstone_entity_version,
                "global_version": descriptor.tombstone_global_version,
                "transaction_time_ns": container.transaction_time_ns(descriptor.tombstone_global_version),
                "replacement": descriptor.replacement_id.map(|id| hex(&id.0)),
            }))?;
            replacements.insert(id, descriptor.replacement_id);
            current.remove(&id);
        }
    }
    for (id, revision) in &current {
        projection.insert("current", id.0, json!(revision))?;
    }
    Ok(EntityCut {
        fingerprint: projection.fingerprint()?,
        current,
        replacements,
    })
}

fn revision_value(record: &EntityRecord, pruned: bool, transaction_time_ns: Option<i64>) -> Value {
    let EntityRecord {
        id,
        global_id,
        revision,
        canonical_name,
        aliases,
        kind,
        summary,
        mutation_id,
        created_at_ns,
        updated_at_ns,
        global_version,
        entity_version,
    } = record;
    // Prune placeholders are unavailable payload, never authored empty strings.
    let payload = (!pruned).then(|| {
        json!({"canonical_name": canonical_name,
        "aliases": aliases, "summary": summary})
    });
    json!({"id": hex(&id.0), "global_id": global_id.map(|id| hex(&id.0)),
        "revision": revision, "kind": kind, "payload": payload, "pruned": pruned,
        "mutation_id": mutation_id, "created_at_ns": created_at_ns, "updated_at_ns": updated_at_ns,
        "global_version": global_version, "entity_version": entity_version,
        "transaction_time_ns": transaction_time_ns})
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[path = "historical_entity_fingerprint_tests.rs"]
mod tests;
