//! Test-only projection for the currently persisted Relationship owner. No
//! Observation store or migration-target decision is introduced here.

use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::relationship_model::RelationshipRecord;
use crate::relationship_store::RelationshipStore;
use crate::{Container, RelationshipId};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct RelationshipCut {
    pub(crate) fingerprint: HistoricalFingerprint,
    pub(crate) current: BTreeMap<RelationshipId, u64>,
}

pub(crate) fn relationship_cut(
    container: &Container,
    store: &RelationshipStore,
    cut: u64,
) -> Result<RelationshipCut, String> {
    if cut > store.relationship_version() {
        return Err("Relationship fingerprint cut exceeds retained head".into());
    }
    let mut projection = OwnerProjection::new(
        container
            .owner_uuid()
            .ok_or("Relationship oracle needs owner UUID")?,
        "relationship",
        cut,
        None,
    )?;
    let mut current = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for record in store.records() {
        ids.insert(record.id);
        if record.relationship_version <= cut {
            projection.insert(
                "revision",
                record.relationship_version.to_be_bytes(),
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
    }
    for id in ids {
        if let Ok(crate::owner_lifecycle::LifecycleState::Tombstoned(_)) = store.lifecycle_state(id)
        {
            let descriptor = store.retention_descriptor(id).map_err(|e| e.to_string())?;
            if descriptor.tombstone_relationship_version <= cut {
                projection.insert("terminal", id.0, json!({"id": hex(&id.0),
                    "revision": descriptor.tombstone_revision,
                    "relationship_version": descriptor.tombstone_relationship_version,
                    "global_version": descriptor.tombstone_global_version,
                    "transaction_time_ns": container.transaction_time_ns(descriptor.tombstone_global_version)}))?;
                current.remove(&id);
            }
        }
    }
    for (id, revision) in &current {
        projection.insert("current", id.0, json!(revision))?;
    }
    Ok(RelationshipCut {
        fingerprint: projection.fingerprint()?,
        current,
    })
}

fn revision_value(
    record: &RelationshipRecord,
    pruned: bool,
    transaction_time_ns: Option<i64>,
) -> Value {
    let RelationshipRecord {
        id,
        revision,
        kind,
        participants,
        evidence,
        summary,
        mutation_id,
        created_at_ns,
        updated_at_ns,
        global_version,
        relationship_version,
    } = record;
    json!({"id": hex(&id.0), "revision": revision, "kind": kind,
        "participants": participants.iter().map(|p| {
            let crate::RelationshipParticipant { entity, role } = p;
            let crate::EntityRef { owner_id, entity_id } = entity;
            json!({"owner_id": owner_id, "entity_id": hex(&entity_id.0), "role": role})
        }).collect::<Vec<_>>(),
        "evidence": evidence.iter().map(|r| {
            let crate::MemoryRef { owner_id, memory_id } = r;
            json!({"owner_id": owner_id, "memory_id": hex(&memory_id.0)})
        }).collect::<Vec<_>>(),
        "summary": (!pruned).then_some(summary), "pruned": pruned,
        "mutation_id": mutation_id, "created_at_ns": created_at_ns, "updated_at_ns": updated_at_ns,
        "global_version": global_version, "relationship_version": relationship_version,
        "transaction_time_ns": transaction_time_ns})
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
#[path = "historical_relationship_fingerprint_tests.rs"]
mod tests;
