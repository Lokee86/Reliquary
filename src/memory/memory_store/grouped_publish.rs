use super::*;
use crate::memory_model::GroupedBodySpan;
use std::collections::{HashMap, HashSet};

pub(crate) struct PreparedMemoryBatch {
    pub(crate) records: Vec<MemoryRecord>,
    pub(crate) existing: Vec<Memory>,
    pub(crate) bodies: Vec<(MemoryBodyId, Vec<u8>)>,
    pub(crate) routing_metadata: Vec<MemoryRoutingMetadata>,
    pub(crate) global_version_start: u64,
}

impl MemoryStore {
    /// Rebase only the storage coordinates of an accepted grouped first
    /// publication. Semantic Memory fields and proof identities remain exact.
    pub(crate) fn rebase_grouped_replay_versions(
        &self,
        container: &Container,
        records: &mut [MemoryRecord],
        body_ids: &[MemoryBodyId],
    ) -> Result<u64, MemoryError> {
        let global_start = container.next_version_candidate();
        if records.is_empty() {
            return Ok(global_start);
        }
        let mut memory_ids = HashSet::new();
        let mut mutation_ids = HashSet::new();
        let memory_start = self.next_memory_version;
        for (index, record) in records.iter_mut().enumerate() {
            if record.revision != 1
                || !memory_ids.insert(record.id)
                || !mutation_ids.insert(record.mutation_id.clone())
                || self.current.contains_key(&record.id)
                || self.tombstones.contains_key(&record.id)
                || self.by_mutation.contains_key(&record.mutation_id)
                || memory_id(&record.mutation_id) != record.id
            {
                return Err(MemoryError::MutationConflict);
            }
            if !body_ids.contains(&record.body_id) && !self.bodies.contains_key(&record.body_id) {
                return Err(MemoryError::MissingBody);
            }
            record.global_version = global_start
                .checked_add(u64::try_from(index).map_err(|_| MemoryError::VersionExhausted)?)
                .ok_or(MemoryError::VersionExhausted)?;
            record.memory_version = memory_start
                .checked_add(u64::try_from(index).map_err(|_| MemoryError::VersionExhausted)?)
                .ok_or(MemoryError::VersionExhausted)?;
        }
        Ok(global_start)
    }

    pub(crate) fn stage_grouped_publication(
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
        let mut bodies: Vec<(MemoryBodyId, Vec<u8>)> = Vec::new();
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
                if bodies[index].1 != body_bytes {
                    return Err(MemoryError::HashCollision);
                }
            } else {
                body_indexes.insert(body_id, bodies.len());
                bodies.push((body_id, body_bytes));
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
        Ok(PreparedMemoryBatch {
            records,
            existing,
            bodies,
            routing_metadata,
            global_version_start,
        })
    }

    pub(crate) fn apply_grouped_spans(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
        bodies: &[GroupedBodySpan],
    ) -> Result<(), MemoryError> {
        for body in bodies {
            let encoded = payload
                .get(body.range.clone())
                .ok_or(MemoryError::CorruptBody)?;
            let raw = match body.encoding {
                crate::memory_model::GroupedBodyEncoding::Raw => encoded.to_vec(),
                crate::memory_model::GroupedBodyEncoding::JsonByteArray => {
                    serde_json::from_slice(encoded).map_err(|_| MemoryError::CorruptBody)?
                }
            };
            let (title, content) = decode_memory_body(&raw)?;
            if memory_body_id(&title, &content) != body.id {
                return Err(MemoryError::CorruptBody);
            }
        }
        let mut seen = HashSet::new();
        for body in bodies {
            if seen.insert(body.id) && !self.bodies.contains_key(&body.id) {
                self.insert_grouped_body_span(body, chunk)?;
            }
        }
        Ok(())
    }

    pub(crate) fn apply_grouped_records(
        &mut self,
        container: &mut Container,
        records: &[MemoryRecord],
    ) -> Result<Vec<Memory>, MemoryError> {
        let mut created = Vec::with_capacity(records.len());
        for record in records {
            if !self.bodies.contains_key(&record.body_id) {
                return Err(MemoryError::MissingBody);
            }
            self.insert_rebuilt(record.clone())?;
            created.push(self.resolve_record(container, record)?);
        }
        Ok(created)
    }

    pub(crate) fn apply_grouped_routing_metadata(
        &mut self,
        metadata: &[MemoryRoutingMetadata],
    ) -> Result<(), MemoryError> {
        for value in metadata {
            self.insert_routing_metadata_rebuilt(value.clone())?;
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

#[cfg(test)]
mod grouped_span_tests {
    use super::*;
    use crate::memory_model::{GroupedBodyEncoding, GroupedBodySpan};

    fn encoded_body(title: &str, content: &str) -> (MemoryBodyId, Vec<u8>) {
        let id = memory_body_id(title, content);
        let bytes = memory_body_bytes(title, content);
        (id, bytes)
    }

    #[test]
    fn grouped_span_admission_accepts_raw_and_json_arrays() {
        let (raw_id, raw) = encoded_body("raw", "frame");
        let (json_id, json_body) = encoded_body("json", "frame");
        let json = serde_json::to_vec(&json_body).unwrap();
        let raw_start = json.iter().position(|byte| *byte == b'[').unwrap();
        let raw_end = json.iter().rposition(|byte| *byte == b']').unwrap() + 1;
        let mut memory = MemoryStore::empty();
        memory
            .apply_grouped_spans(
                ObjectRef::from_legacy_bytes([0; 16]),
                &raw,
                &[GroupedBodySpan {
                    id: raw_id,
                    range: 0..raw.len(),
                    encoding: GroupedBodyEncoding::Raw,
                }],
            )
            .unwrap();
        memory
            .apply_grouped_spans(
                ObjectRef::from_legacy_bytes([1; 16]),
                &json,
                &[GroupedBodySpan {
                    id: json_id,
                    range: raw_start..raw_end,
                    encoding: GroupedBodyEncoding::JsonByteArray,
                }],
            )
            .unwrap();
        assert_eq!(memory.bodies.len(), 2);
    }

    #[test]
    fn grouped_span_validation_is_atomic_for_bad_hash_or_range() {
        let (good_id, good) = encoded_body("good", "body");
        let (real_id, bad) = encoded_body("evil", "body");
        let wrong_id = MemoryBodyId([0xA5; 32]);
        assert_ne!(real_id, wrong_id);
        let mut memory = MemoryStore::empty();
        let chunk = ObjectRef::from_legacy_bytes([2; 16]);
        assert!(
            memory
                .apply_grouped_spans(
                    chunk,
                    &good,
                    &[
                        GroupedBodySpan {
                            id: good_id,
                            range: 0..good.len(),
                            encoding: GroupedBodyEncoding::Raw,
                        },
                        GroupedBodySpan {
                            id: wrong_id,
                            range: 0..bad.len(),
                            encoding: GroupedBodyEncoding::Raw,
                        },
                    ],
                )
                .is_err()
        );
        assert!(memory.bodies.is_empty());
        assert!(
            memory
                .apply_grouped_spans(
                    chunk,
                    &good,
                    &[GroupedBodySpan {
                        id: good_id,
                        range: 0..good.len() + 1,
                        encoding: GroupedBodyEncoding::Raw,
                    }],
                )
                .is_err()
        );
        assert!(memory.bodies.is_empty());
    }
}
