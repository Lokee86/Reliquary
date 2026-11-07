//! Test-only Memory history materialized from accepted owner state, independent
//! of whether publication used standalone, grouped Insomnia or external records.

use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::memory_model::MemoryRecord;
use crate::memory_store::MemoryStore;
use crate::{Container, MemoryId, MemorySourceRef, MemoryTemporalInference};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

struct Revision {
    id: MemoryId,
    revision: u64,
    version: u64,
    body: [u8; 32],
    value: Value,
}

pub(crate) struct MemoryHistory {
    owner: [u8; 16],
    head: u64,
    revisions: Vec<Revision>,
    bodies: BTreeMap<[u8; 32], Value>,
    routing: BTreeMap<MemoryId, Value>,
    external: BTreeMap<MemoryId, Value>,
    terminals: BTreeMap<MemoryId, (u64, Value)>,
}

pub(crate) struct MemoryCut {
    pub(crate) fingerprint: HistoricalFingerprint,
    pub(crate) current: BTreeMap<MemoryId, u64>,
}

impl MemoryHistory {
    pub(crate) fn read(container: &mut Container, store: &MemoryStore) -> Result<Self, String> {
        let mut history = Self {
            owner: container
                .owner_uuid()
                .ok_or("Memory oracle needs owner UUID")?,
            head: store.memory_version(),
            revisions: Vec::new(),
            bodies: BTreeMap::new(),
            routing: BTreeMap::new(),
            external: BTreeMap::new(),
            terminals: BTreeMap::new(),
        };
        for record in store.records() {
            if !history.bodies.contains_key(&record.body_id.0)
                || !history.routing.contains_key(&record.id)
            {
                if let Some(marker) = store.prune.get(record.body_id) {
                    history.bodies.insert(
                        record.body_id.0,
                        json!({"pruned": {
                        "bytes": marker.byte_length, "manifest": hex(&marker.manifest_digest)}}),
                    );
                    // Public historical revision access exposes structural
                    // history, not hydrated routing, after explicit body prune.
                    history.routing.insert(record.id, Value::Null);
                } else {
                    let memory = store
                        .memory_revision(container, record.id, record.revision)
                        .map_err(|e| e.to_string())?;
                    history.bodies.insert(
                        record.body_id.0,
                        json!({"title": memory.title, "content": memory.content}),
                    );
                    history.routing.insert(record.id, memory.routing_metadata.map_or(Value::Null, |r| json!({
                        "memory_id": hex(&r.memory_id.0), "body_id": hex(&r.body_id.0),
                        "mentions": r.entity_mentions.iter().map(|m| json!({
                            "field": m.field.tag(), "start": m.start_byte, "end": m.end_byte, "text": m.text,
                        })).collect::<Vec<_>>()
                    })));
                }
            }
            if let Some(evidence) = store.external_evidence.get(&(record.id, record.body_id)) {
                history.external.insert(
                    record.id,
                    serde_json::to_value(evidence).map_err(|e| e.to_string())?,
                );
            }
            let mut value = record_value(record);
            value.as_object_mut().unwrap().insert(
                "transaction_time_ns".into(),
                json!(container.transaction_time_ns(record.global_version)),
            );
            history.revisions.push(Revision {
                id: record.id,
                revision: record.revision,
                version: record.memory_version,
                body: record.body_id.0,
                value,
            });
        }
        for id in store.tombstoned_ids() {
            let descriptor = store.retention_descriptor(id).map_err(|e| e.to_string())?;
            history.terminals.insert(id, (descriptor.tombstone_memory_version, json!({
                "id": hex(&id.0), "revision": descriptor.tombstone_revision,
                "global_version": descriptor.tombstone_global_version,
                "memory_version": descriptor.tombstone_memory_version,
                "transaction_time_ns": container.transaction_time_ns(descriptor.tombstone_global_version),
            })));
        }
        Ok(history)
    }

    pub(crate) fn at(&self, cut: u64) -> Result<MemoryCut, String> {
        if cut > self.head {
            return Err("Memory fingerprint cut exceeds retained head".into());
        }
        let mut projection = OwnerProjection::new(self.owner, "memory", cut, None)?;
        let mut current = BTreeMap::new();
        let mut bodies = BTreeSet::new();
        let mut ids = BTreeSet::new();
        for revision in self.revisions.iter().filter(|r| r.version <= cut) {
            projection.insert(
                "revision",
                revision.version.to_be_bytes(),
                revision.value.clone(),
            )?;
            bodies.insert(revision.body);
            ids.insert(revision.id);
            current.insert(revision.id, revision.revision);
        }
        for (id, (version, terminal)) in &self.terminals {
            if *version <= cut {
                projection.insert("terminal", id.0, terminal.clone())?;
                current.remove(id);
            }
        }
        self.add_payload(&mut projection, bodies, ids)?;
        for (id, revision) in &current {
            projection.insert("current", id.0, json!(revision))?;
        }
        Ok(MemoryCut {
            fingerprint: projection.fingerprint()?,
            current,
        })
    }

    pub(crate) fn revision(
        &self,
        id: MemoryId,
        revision: u64,
    ) -> Result<HistoricalFingerprint, String> {
        let record = self
            .revisions
            .iter()
            .find(|r| r.id == id && r.revision == revision)
            .ok_or("Memory oracle revision is not retained")?;
        let mut projection =
            OwnerProjection::new(self.owner, "memory_revision", record.version, None)?;
        projection.insert(
            "revision",
            record.version.to_be_bytes(),
            record.value.clone(),
        )?;
        self.add_payload(
            &mut projection,
            BTreeSet::from([record.body]),
            BTreeSet::from([id]),
        )?;
        projection.fingerprint()
    }

    fn add_payload(
        &self,
        projection: &mut OwnerProjection,
        bodies: BTreeSet<[u8; 32]>,
        ids: BTreeSet<MemoryId>,
    ) -> Result<(), String> {
        for body in bodies {
            projection.insert("body", body, self.bodies[&body].clone())?;
        }
        for id in ids {
            projection.insert("routing", id.0, self.routing[&id].clone())?;
            if let Some(evidence) = self.external.get(&id) {
                projection.insert("external_evidence", id.0, evidence.clone())?;
            }
        }
        Ok(())
    }
}

fn record_value(record: &MemoryRecord) -> Value {
    // No `..`: adding an authoritative Memory field must break this oracle
    // until the canonical semantic projection explicitly accounts for it.
    let MemoryRecord {
        id,
        revision,
        body_id,
        category,
        memory_type,
        authority_kind,
        temporal_status,
        scope,
        lifecycle_state,
        archived,
        superseded_by,
        parent_id,
        source_node_id,
        content_source_conversation_id,
        content_source_node_id,
        grounding_source_conversation_id,
        grounding_source_node_id,
        source_episode_id,
        source_time_ns,
        source_ref,
        temporal_inference,
        mutation_id,
        created_at_ns,
        updated_at_ns,
        global_version,
        memory_version,
    } = record;
    json!({"id": hex(&id.0), "revision": revision, "body_id": hex(&body_id.0),
        "category": category, "memory_type": memory_type, "authority_kind": authority_kind,
        "temporal_status": temporal_status, "scope": scope, "lifecycle_state": lifecycle_state,
        "archived": archived, "superseded_by": superseded_by.map(|id| hex(&id.0)),
        "parent_id": parent_id.map(|id| hex(&id.0)), "source_node_id": source_node_id,
        "content_source_conversation_id": content_source_conversation_id,
        "content_source_node_id": content_source_node_id,
        "grounding_source_conversation_id": grounding_source_conversation_id,
        "grounding_source_node_id": grounding_source_node_id,
        "source_episode_id": source_episode_id.map(|id| hex(&id.0)),
        "source_time_ns": source_time_ns, "source_ref": source_ref.as_ref().map(source_value),
        "temporal_inference": temporal_inference.as_ref().map(temporal_value),
        "mutation_id": mutation_id, "created_at_ns": created_at_ns, "updated_at_ns": updated_at_ns,
        "global_version": global_version, "memory_version": memory_version})
}

fn source_value(source: &MemorySourceRef) -> Value {
    let MemorySourceRef {
        owner_id,
        principal_id,
        source_episode_id,
        source_node_id,
        content_source_conversation_id,
        content_source_node_id,
        grounding_source_conversation_id,
        grounding_source_node_id,
    } = source;
    json!({"owner_id": owner_id, "principal_id": principal_id,
        "source_episode_id": hex(&source_episode_id.0), "source_node_id": source_node_id,
        "content_source_conversation_id": content_source_conversation_id,
        "content_source_node_id": content_source_node_id,
        "grounding_source_conversation_id": grounding_source_conversation_id,
        "grounding_source_node_id": grounding_source_node_id})
}

fn temporal_value(value: &MemoryTemporalInference) -> Value {
    let MemoryTemporalInference {
        body_id,
        source_time_ns,
        inference,
    } = value;
    let crate::TemporalInference {
        model,
        contract_version,
        resolutions,
    } = inference;
    json!({"body_id": hex(&body_id.0), "source_time_ns": source_time_ns,
        "model": model, "contract_version": contract_version,
        "resolutions": resolutions.iter().map(|r| {
            let crate::TemporalInferenceResolution { start_byte, end_byte, kind, evidence, canonical_expression } = r;
            json!({"start": start_byte, "end": end_byte, "evidence": evidence,
                "canonical_expression": canonical_expression, "kind": match kind {
                    crate::TemporalIndicationKind::Explicit => "explicit",
                    crate::TemporalIndicationKind::Calendar => "calendar",
                    crate::TemporalIndicationKind::Relative => "relative",
                    crate::TemporalIndicationKind::Boundary => "boundary",
                    crate::TemporalIndicationKind::Recurrence => "recurrence",
                    crate::TemporalIndicationKind::Duration => "duration",
                }})
        }).collect::<Vec<_>>()})
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
#[path = "historical_memory_fingerprint_tests.rs"]
mod tests;
