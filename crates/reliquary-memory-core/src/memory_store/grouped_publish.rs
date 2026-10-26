use super::*;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryGroupedBodyEncoding {
    Raw,
    JsonByteArray,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryGroupedBodySpan {
    pub id: MemoryBodyId,
    pub range: Range<usize>,
    pub encoding: MemoryGroupedBodyEncoding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryGroupedBody {
    pub id: MemoryBodyId,
    pub bytes: Vec<u8>,
}

impl MemoryGroupedBody {
    pub fn validate(&self) -> Result<(), MemoryError> {
        let (title, content) = super::decode_memory_body(&self.bytes)?;
        if memory_body_id(&title, &content) != self.id {
            return Err(MemoryError::CorruptBody);
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct MemoryGroupedBatch {
    pub records: Vec<MemoryRecord>,
    pub bodies: Vec<MemoryGroupedBodySpan>,
    pub routing_metadata: Vec<MemoryRoutingMetadata>,
}

impl MemoryGroupedBatch {
    pub fn new(
        records: Vec<MemoryRecord>,
        bodies: Vec<MemoryGroupedBodySpan>,
        routing_metadata: Vec<MemoryRoutingMetadata>,
    ) -> Self {
        Self {
            records,
            bodies,
            routing_metadata,
        }
    }
}

pub struct PreparedMemoryBatch {
    pub records: Vec<MemoryRecord>,
    pub existing: Vec<Memory>,
    pub bodies: Vec<MemoryGroupedBody>,
    pub routing_metadata: Vec<MemoryRoutingMetadata>,
    pub global_version_start: u64,
}

impl MemoryStore {
    pub fn stage_grouped(
        &mut self,
        container: &mut Container,
        drafts: Vec<(
            MemoryDraft,
            Option<MemoryTemporalInference>,
            Option<MemoryRoutingMetadata>,
        )>,
    ) -> Result<PreparedMemoryBatch, MemoryError> {
        let global_version_start = container.next_version_candidate();
        let mut records = Vec::new();
        let mut existing = Vec::new();
        let mut bodies: Vec<MemoryGroupedBody> = Vec::new();
        let mut routing_metadata = Vec::new();
        let mut body_indexes: HashMap<MemoryBodyId, usize> = HashMap::new();
        let mut seen_mutations = HashSet::new();
        for (draft, temporal_inference, routing) in drafts {
            validate_draft(&draft)?;
            if !seen_mutations.insert(draft.mutation_id.clone()) {
                return Err(MemoryError::MutationConflict);
            }
            if let Some(index) = self.by_mutation.get(&draft.mutation_id).copied() {
                let memory = self.resolve_record(container, &self.records[index])?;
                if !same_draft(&memory, &draft) {
                    return Err(MemoryError::MutationConflict);
                }
                if let Some(routing) = routing.as_ref()
                    && memory.routing_metadata.as_ref() != Some(routing)
                {
                    return Err(MemoryError::RoutingMetadataConflict);
                }
                existing.push(memory);
                continue;
            }
            let id = memory_id(&draft.mutation_id);
            if self.current.contains_key(&id) {
                return Err(MemoryError::RevisionConflict);
            }
            let body_id = memory_body_id(&draft.title, &draft.content);
            if let Some(routing) = routing.as_ref() {
                validate_prepared_routing(&draft, id, body_id, routing)?;
            }
            if let Some(inference) = &temporal_inference {
                validate_temporal_inference_binding(body_id, draft.source_time_ns, inference)?;
            }
            let body_bytes = memory_body_bytes(&draft.title, &draft.content);
            if self.bodies.contains_key(&body_id) {
                if self.body_bytes(container, body_id)? != body_bytes {
                    return Err(MemoryError::HashCollision);
                }
            } else if let Some(index) = body_indexes.get(&body_id).copied() {
                if bodies[index].bytes != body_bytes {
                    return Err(MemoryError::HashCollision);
                }
            } else {
                body_indexes.insert(body_id, bodies.len());
                bodies.push(MemoryGroupedBody {
                    id: body_id,
                    bytes: body_bytes,
                });
            }
            let offset = u64::try_from(records.len()).map_err(|_| MemoryError::VersionExhausted)?;
            let memory_version = self
                .next_memory_version
                .checked_add(offset)
                .ok_or(MemoryError::VersionExhausted)?;
            let global_version = global_version_start
                .checked_add(offset)
                .ok_or(MemoryError::VersionExhausted)?;
            records.push(MemoryRecord {
                id,
                revision: 1,
                body_id,
                category: draft.category,
                memory_type: draft.memory_type,
                authority_kind: draft.authority_kind,
                temporal_status: draft.temporal_status,
                scope: draft.scope,
                lifecycle_state: draft.lifecycle_state,
                archived: draft.archived,
                superseded_by: draft.superseded_by,
                parent_id: draft.parent_id,
                source_node_id: draft.source_node_id,
                content_source_conversation_id: draft.content_source_conversation_id,
                content_source_node_id: draft.content_source_node_id,
                grounding_source_conversation_id: draft.grounding_source_conversation_id,
                grounding_source_node_id: draft.grounding_source_node_id,
                source_episode_id: draft.source_episode_id,
                source_time_ns: draft.source_time_ns,
                source_ref: None,
                temporal_inference,
                mutation_id: draft.mutation_id,
                created_at_ns: draft.created_at_ns,
                updated_at_ns: draft.updated_at_ns,
                global_version,
                memory_version,
            });
            if let Some(routing) = routing {
                routing_metadata.push(routing);
            }
        }
        for record in &records {
            Self::validate_compact_record(record)?;
        }
        Ok(PreparedMemoryBatch {
            records,
            existing,
            bodies,
            routing_metadata,
            global_version_start,
        })
    }

    /// Validate against only incoming identities; never copy accumulated owner history.
    pub fn apply_grouped_batch(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
        batch: MemoryGroupedBatch,
        latest_global_version: u64,
    ) -> Result<(), MemoryError> {
        self.validate_grouped_batch(payload, &batch, latest_global_version)?;
        // All fallible checks precede any mutation of the owner.
        for span in batch.bodies {
            self.bodies
                .entry(span.id)
                .or_insert(BodyLocation::GroupedFrame {
                    frame: chunk,
                    range: span.range,
                    encoding: span.encoding,
                });
        }
        for record in batch.records {
            self.insert_rebuilt(record)
                .expect("prevalidated grouped Memory record");
        }
        for metadata in batch.routing_metadata {
            if !self.routing_metadata.contains_key(&metadata.memory_id) {
                self.index_routing_metadata(&metadata);
                self.routing_metadata.insert(metadata.memory_id, metadata);
            }
        }
        Ok(())
    }

    fn validate_grouped_batch(
        &self,
        payload: &[u8],
        batch: &MemoryGroupedBatch,
        latest_global_version: u64,
    ) -> Result<(), MemoryError> {
        let mut body_ids = HashSet::new();
        for span in &batch.bodies {
            if !body_ids.insert(span.id) {
                return Err(MemoryError::CorruptRecord(
                    "duplicate grouped body identity",
                ));
            }
            let encoded = payload
                .get(span.range.clone())
                .ok_or(MemoryError::CorruptBody)?;
            let bytes = match span.encoding {
                MemoryGroupedBodyEncoding::Raw => encoded.to_vec(),
                MemoryGroupedBodyEncoding::JsonByteArray => {
                    serde_json::from_slice(encoded).map_err(|_| MemoryError::CorruptBody)?
                }
            };
            let (title, content) = decode_memory_body(&bytes)?;
            if memory_body_id(&title, &content) != span.id {
                return Err(MemoryError::CorruptBody);
            }
        }

        let mut next_local = self.next_memory_version;
        let mut next_global = None;
        let mut staged_current: HashMap<MemoryId, &MemoryRecord> = HashMap::new();
        let mut staged_mutations: HashMap<&str, &MemoryRecord> = HashMap::new();
        for record in &batch.records {
            MemoryStore::validate_compact_record(record)?;
            let expected_global = next_global.unwrap_or(record.global_version);
            if record.global_version == 0
                || record.global_version > latest_global_version
                || record.global_version != expected_global
                || record.memory_version != next_local
            {
                return Err(MemoryError::InvalidVersion);
            }
            next_global = Some(
                expected_global
                    .checked_add(1)
                    .ok_or(MemoryError::VersionExhausted)?,
            );
            next_local = next_local
                .checked_add(1)
                .ok_or(MemoryError::VersionExhausted)?;
            if !self.bodies.contains_key(&record.body_id) && !body_ids.contains(&record.body_id) {
                return Err(MemoryError::MissingBody);
            }
            if self.tombstones.contains_key(&record.id) {
                return Err(MemoryError::TerminalTombstone);
            }
            if record.revision == 0 {
                return Err(MemoryError::InvalidVersion);
            }
            if let Some(index) = self.by_mutation.get(&record.mutation_id).copied() {
                if self.records[index] != *record {
                    return Err(MemoryError::MutationConflict);
                }
                continue;
            }
            if let Some(previous) = staged_mutations.get(record.mutation_id.as_str()) {
                if **previous != *record {
                    return Err(MemoryError::MutationConflict);
                }
                continue;
            }
            let previous = staged_current.get(&record.id).copied().or_else(|| {
                self.current
                    .get(&record.id)
                    .map(|&index| &self.records[index])
            });
            let expected_revision = match previous {
                Some(value) => value
                    .revision
                    .checked_add(1)
                    .ok_or(MemoryError::VersionExhausted)?,
                None => 1,
            };
            if record.revision != expected_revision {
                return Err(MemoryError::RevisionConflict);
            }
            if previous.is_some_and(|value| value.body_id != record.body_id) {
                return Err(MemoryError::SemanticMutation);
            }
            staged_mutations.insert(&record.mutation_id, record);
            staged_current.insert(record.id, record);
        }

        let mut staged_routing: HashMap<MemoryId, &MemoryRoutingMetadata> = HashMap::new();
        for metadata in &batch.routing_metadata {
            if metadata.entity_mentions.len() > MAX_MEMORY_ENTITY_MENTIONS {
                return Err(MemoryError::InvalidField("Memory routing metadata count"));
            }
            let bound_in_batch = batch.records.iter().any(|record| {
                record.id == metadata.memory_id && record.body_id == metadata.body_id
            });
            let bound_current = self
                .current
                .get(&metadata.memory_id)
                .is_some_and(|&index| self.records[index].body_id == metadata.body_id);
            if !bound_in_batch
                && !bound_current
                && !self.records.iter().any(|record| {
                    record.id == metadata.memory_id && record.body_id == metadata.body_id
                })
            {
                return Err(MemoryError::InvalidField("Memory routing metadata binding"));
            }
            if metadata.entity_mentions.iter().any(|mention| {
                mention.text.is_empty()
                    || mention.text.len() > MAX_MEMORY_ROUTING_TEXT_BYTES
                    || mention.start_byte >= mention.end_byte
            }) {
                return Err(MemoryError::InvalidField("Memory entity mention"));
            }
            if self
                .routing_metadata
                .get(&metadata.memory_id)
                .is_some_and(|previous| previous != metadata)
                || staged_routing
                    .get(&metadata.memory_id)
                    .is_some_and(|previous| *previous != metadata)
            {
                return Err(MemoryError::RoutingMetadataConflict);
            }
            staged_routing.insert(metadata.memory_id, metadata);
        }
        Ok(())
    }
}

fn validate_prepared_routing(
    draft: &MemoryDraft,
    memory_id: MemoryId,
    body_id: MemoryBodyId,
    metadata: &MemoryRoutingMetadata,
) -> Result<(), MemoryError> {
    if metadata.memory_id != memory_id || metadata.body_id != body_id {
        return Err(MemoryError::InvalidField("Memory routing metadata binding"));
    }
    if metadata.entity_mentions.len() > MAX_MEMORY_ENTITY_MENTIONS {
        return Err(MemoryError::InvalidField("Memory routing metadata count"));
    }
    for mention in &metadata.entity_mentions {
        if mention.text.is_empty()
            || mention.text.len() > MAX_MEMORY_ROUTING_TEXT_BYTES
            || mention.start_byte >= mention.end_byte
        {
            return Err(MemoryError::InvalidField("Memory entity mention"));
        }
        let source = match mention.field {
            MemoryTextField::Title => draft.title.as_str(),
            MemoryTextField::Content => draft.content.as_str(),
        };
        let start = mention.start_byte as usize;
        let end = mention.end_byte as usize;
        if !source.is_char_boundary(start)
            || !source.is_char_boundary(end)
            || source.get(start..end) != Some(mention.text.as_str())
        {
            return Err(MemoryError::InvalidField("Memory entity mention span"));
        }
    }
    Ok(())
}
