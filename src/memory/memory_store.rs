use crate::memory_codec::{
    MemoryVersion, decode_body, encode_body, encode_format, encode_record, encode_version,
};
use crate::memory_model::{MemoryRecord, memory_body_bytes, memory_body_id, memory_id};
use crate::{
    Container, MAX_MEMORY_ENTITY_MENTIONS, MAX_MEMORY_ROUTING_TEXT_BYTES, Memory, MemoryBodyId,
    MemoryDraft, MemoryEntityMentionKey, MemoryError, MemoryId, MemoryRoutingMetadata,
    MemorySourceRef, MemoryStats, MemoryTemporalInference, MemoryTextField, ObjectRef,
};
use std::collections::{HashMap, HashSet};

#[path = "memory_store/grouped_publish.rs"]
mod grouped_publish;

pub(crate) struct MemoryStore {
    bodies: HashMap<MemoryBodyId, ObjectRef>,
    routing_metadata: HashMap<MemoryId, MemoryRoutingMetadata>,
    entity_mentions_by_surface: HashMap<String, HashSet<MemoryEntityMentionKey>>,
    records: Vec<MemoryRecord>,
    current: HashMap<MemoryId, usize>,
    by_mutation: HashMap<String, usize>,
    next_memory_version: u64,
}

pub(crate) enum PublishPreflight {
    Retry(Memory),
    New {
        record: MemoryRecord,
        next_memory_version: u64,
        body_id: MemoryBodyId,
    },
}

impl MemoryStore {
    pub(crate) fn empty() -> Self {
        Self {
            bodies: HashMap::new(),
            routing_metadata: HashMap::new(),
            entity_mentions_by_surface: HashMap::new(),
            records: Vec::new(),
            current: HashMap::new(),
            by_mutation: HashMap::new(),
            next_memory_version: 1,
        }
    }

    pub(crate) fn initialize(&self, container: &mut Container) -> Result<(), MemoryError> {
        container.append(&encode_format())?;
        Ok(())
    }

    pub(crate) fn publish(
        &mut self,
        container: &mut Container,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
    ) -> Result<(Memory, bool), MemoryError> {
        self.publish_with_source_ref_and_temporal_inference(
            container,
            id,
            expected_revision,
            draft,
            None,
            None,
        )
    }

    pub(crate) fn publish_with_source_ref(
        &mut self,
        container: &mut Container,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
        source_ref: Option<MemorySourceRef>,
    ) -> Result<(Memory, bool), MemoryError> {
        self.publish_with_source_ref_and_temporal_inference(
            container,
            id,
            expected_revision,
            draft,
            source_ref,
            None,
        )
    }

    pub(crate) fn publish_with_source_ref_and_temporal_inference(
        &mut self,
        container: &mut Container,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
        source_ref: Option<MemorySourceRef>,
        temporal_inference: Option<MemoryTemporalInference>,
    ) -> Result<(Memory, bool), MemoryError> {
        let preflight = self.preflight_publish(
            container,
            id,
            expected_revision,
            &draft,
            source_ref.as_ref(),
            temporal_inference.as_ref(),
        )?;
        let (mut record, next_memory_version) = match preflight {
            PublishPreflight::Retry(existing) => return Ok((existing, false)),
            PublishPreflight::New {
                record,
                next_memory_version,
                ..
            } => (record, next_memory_version),
        };
        self.put_body(container, &draft.title, &draft.content)?;
        let record_chunk = container.append(&encode_record(&record)?)?;
        let global_version = container.allocate_version()?;
        container.append(&encode_version(MemoryVersion {
            global_version,
            memory_version: record.memory_version,
            record: record_chunk,
        }))?;
        record.global_version = global_version;
        self.insert(record.clone())?;
        self.next_memory_version = next_memory_version;
        self.resolve_record(container, &record)
            .map(|memory| (memory, true))
    }

    pub(crate) fn preflight_publish(
        &self,
        container: &mut Container,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: &MemoryDraft,
        source_ref: Option<&MemorySourceRef>,
        temporal_inference: Option<&MemoryTemporalInference>,
    ) -> Result<PublishPreflight, MemoryError> {
        validate_draft(draft)?;
        validate_source_ref(source_ref)?;
        let body_id = memory_body_id(&draft.title, &draft.content);
        if let Some(inference) = temporal_inference {
            validate_temporal_inference_binding(body_id, draft.source_time_ns, inference)?;
        }
        if let Some(index) = self.by_mutation.get(&draft.mutation_id).copied() {
            let existing = self.resolve_record(container, &self.records[index])?;
            let source_matches = source_ref
                .is_none_or(|source_ref| existing.source_ref.as_ref() == Some(source_ref));
            let inference_matches = temporal_inference
                .is_none_or(|inference| existing.temporal_inference.as_ref() == Some(inference));
            return if same_draft(&existing, draft) && source_matches && inference_matches {
                Ok(PublishPreflight::Retry(existing))
            } else {
                Err(MemoryError::MutationConflict)
            };
        }
        let target_id = id.unwrap_or_else(|| memory_id(&draft.mutation_id));
        let current_revision = self
            .current
            .get(&target_id)
            .map(|index| self.records[*index].revision)
            .unwrap_or(0);
        if current_revision != expected_revision {
            return Err(MemoryError::RevisionConflict);
        }
        if let Some(index) = self.current.get(&target_id).copied()
            && self.records[index].body_id != body_id
        {
            return Err(MemoryError::SemanticMutation);
        }
        let revision = expected_revision
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        let memory_version = self.next_memory_version;
        let next_memory_version = memory_version
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        let bytes = memory_body_bytes(&draft.title, &draft.content);
        if self.bodies.contains_key(&body_id) && self.body_bytes(container, body_id)? != bytes {
            return Err(MemoryError::HashCollision);
        }
        let source_ref = source_ref.cloned().or_else(|| {
            self.current
                .get(&target_id)
                .and_then(|index| self.records[*index].source_ref.clone())
        });
        let temporal_inference = temporal_inference.cloned().or_else(|| {
            self.current.get(&target_id).and_then(|index| {
                self.records[*index]
                    .temporal_inference
                    .clone()
                    .filter(|value| {
                        value.body_id == body_id && value.source_time_ns == draft.source_time_ns
                    })
            })
        });
        let record = MemoryRecord {
            id: target_id,
            revision,
            body_id,
            category: draft.category.clone(),
            memory_type: draft.memory_type.clone(),
            authority_kind: draft.authority_kind.clone(),
            temporal_status: draft.temporal_status.clone(),
            scope: draft.scope.clone(),
            lifecycle_state: draft.lifecycle_state.clone(),
            archived: draft.archived,
            superseded_by: draft.superseded_by,
            parent_id: draft.parent_id,
            source_node_id: draft.source_node_id.clone(),
            content_source_conversation_id: draft.content_source_conversation_id.clone(),
            content_source_node_id: draft.content_source_node_id.clone(),
            grounding_source_conversation_id: draft.grounding_source_conversation_id.clone(),
            grounding_source_node_id: draft.grounding_source_node_id.clone(),
            source_episode_id: draft.source_episode_id,
            source_time_ns: draft.source_time_ns,
            source_ref,
            temporal_inference,
            mutation_id: draft.mutation_id.clone(),
            created_at_ns: draft.created_at_ns,
            updated_at_ns: draft.updated_at_ns,
            global_version: 0,
            memory_version,
        };
        encode_body(body_id, &bytes)?;
        encode_record(&record)?;
        if container.next_version_candidate().checked_add(1).is_none() {
            return Err(MemoryError::VersionExhausted);
        }
        Ok(PublishPreflight::New {
            record,
            next_memory_version,
            body_id,
        })
    }

    pub(crate) fn memory(
        &self,
        container: &mut Container,
        id: MemoryId,
    ) -> Result<Memory, MemoryError> {
        let index = self.current.get(&id).ok_or(MemoryError::MissingMemory)?;
        self.resolve_record(container, &self.records[*index])
    }

    pub(crate) fn memory_revision(
        &self,
        container: &mut Container,
        id: MemoryId,
        revision: u64,
    ) -> Result<Memory, MemoryError> {
        let record = self
            .records
            .iter()
            .find(|record| record.id == id && record.revision == revision)
            .ok_or(MemoryError::MissingMemory)?;
        self.resolve_record(container, record)
    }

    pub(crate) fn records(&self) -> &[MemoryRecord] {
        &self.records
    }

    pub(crate) fn memory_version(&self) -> u64 {
        self.next_memory_version.saturating_sub(1)
    }

    pub(crate) fn contains_memory(&self, id: MemoryId) -> bool {
        self.current.contains_key(&id)
    }

    pub(crate) fn current_mutation_id(&self, id: MemoryId) -> Option<&str> {
        self.current
            .get(&id)
            .map(|index| self.records[*index].mutation_id.as_str())
    }

    pub(crate) fn first_mutation_id(&self, id: MemoryId) -> Option<&str> {
        self.records
            .iter()
            .find(|record| record.id == id && record.revision == 1)
            .map(|record| record.mutation_id.as_str())
    }

    pub(crate) fn first_global_version(&self, id: MemoryId) -> Option<u64> {
        self.records
            .iter()
            .find(|record| record.id == id && record.revision == 1)
            .map(|record| record.global_version)
    }

    pub(crate) fn contains_body(&self, id: MemoryBodyId) -> bool {
        self.bodies.contains_key(&id)
    }

    pub(crate) fn current_ids(&self) -> Vec<MemoryId> {
        let mut ids: Vec<_> = self.current.keys().copied().collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub(crate) fn current_body_ids(&self) -> Vec<MemoryBodyId> {
        let mut ids: Vec<_> = self
            .current
            .values()
            .map(|index| self.records[*index].body_id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids.dedup();
        ids
    }

    pub(crate) fn current_body_id(&self, id: MemoryId) -> Result<MemoryBodyId, MemoryError> {
        let index = self.current.get(&id).ok_or(MemoryError::MissingMemory)?;
        Ok(self.records[*index].body_id)
    }

    pub(crate) fn routing_metadata(&self, id: MemoryId) -> Option<&MemoryRoutingMetadata> {
        self.routing_metadata.get(&id)
    }

    pub(crate) fn entity_mention_keys_for_surface(
        &self,
        surface: &str,
    ) -> Vec<MemoryEntityMentionKey> {
        let normalized = normalize_entity_surface(surface);
        let mut keys: Vec<_> = self
            .entity_mentions_by_surface
            .get(&normalized)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        keys.sort_by_key(|key| {
            (
                key.memory_id.0,
                key.field.tag(),
                key.start_byte,
                key.end_byte,
            )
        });
        keys
    }

    pub(crate) fn validate_entity_mention_key(
        &self,
        key: MemoryEntityMentionKey,
    ) -> Result<(), MemoryError> {
        let metadata = self
            .routing_metadata
            .get(&key.memory_id)
            .ok_or(MemoryError::InvalidField("Entity resolution mention"))?;
        if metadata.entity_mentions.iter().any(|mention| {
            mention.field == key.field
                && mention.start_byte == key.start_byte
                && mention.end_byte == key.end_byte
        }) {
            Ok(())
        } else {
            Err(MemoryError::InvalidField("Entity resolution mention"))
        }
    }

    pub(crate) fn resolution_source_inactive(&self, id: MemoryId) -> bool {
        self.current
            .get(&id)
            .map(|index| {
                let record = &self.records[*index];
                record.archived || record.superseded_by.is_some()
            })
            .unwrap_or(true)
    }

    pub(crate) fn put_routing_metadata(
        &mut self,
        container: &mut Container,
        metadata: MemoryRoutingMetadata,
    ) -> Result<bool, MemoryError> {
        self.validate_routing_binding(&metadata)?;
        self.validate_routing_text(container, &metadata)?;
        if let Some(existing) = self.routing_metadata.get(&metadata.memory_id) {
            return if existing == &metadata {
                Ok(false)
            } else {
                Err(MemoryError::RoutingMetadataConflict)
            };
        }
        container.append(&crate::memory_routing_codec::encode(&metadata)?)?;
        self.index_routing_metadata(&metadata);
        self.routing_metadata.insert(metadata.memory_id, metadata);
        Ok(true)
    }

    pub(crate) fn insert_routing_metadata_rebuilt(
        &mut self,
        metadata: MemoryRoutingMetadata,
    ) -> Result<(), MemoryError> {
        self.validate_routing_binding(&metadata)?;
        match self.routing_metadata.get(&metadata.memory_id) {
            Some(existing) if existing == &metadata => Ok(()),
            Some(_) => Err(MemoryError::RoutingMetadataConflict),
            None => {
                self.index_routing_metadata(&metadata);
                self.routing_metadata.insert(metadata.memory_id, metadata);
                Ok(())
            }
        }
    }

    fn index_routing_metadata(&mut self, metadata: &MemoryRoutingMetadata) {
        for mention in &metadata.entity_mentions {
            let key = MemoryEntityMentionKey::new(metadata.memory_id, mention);
            self.entity_mentions_by_surface
                .entry(normalize_entity_surface(&mention.text))
                .or_default()
                .insert(key);
        }
    }

    pub(crate) fn validate_routing_metadata(
        &self,
        container: &mut Container,
    ) -> Result<(), MemoryError> {
        for metadata in self.routing_metadata.values() {
            self.validate_routing_binding(metadata)?;
            self.validate_routing_text(container, metadata)?;
        }
        Ok(())
    }

    pub(crate) fn body_embedding_text(
        &self,
        container: &mut Container,
        id: MemoryBodyId,
    ) -> Result<String, MemoryError> {
        let body = self.body_bytes(container, id)?;
        let (title, content) = decode_memory_body(&body)?;
        Ok(format!("{title}\n\n{content}"))
    }

    pub(crate) fn stats(&self) -> MemoryStats {
        MemoryStats {
            memories: self.current.len(),
            revisions: self.records.len(),
            bodies: self.bodies.len(),
            memory_version: self.memory_version(),
        }
    }

    pub(crate) fn insert_body(&mut self, id: MemoryBodyId, chunk: ObjectRef) -> bool {
        if self.bodies.contains_key(&id) {
            return false;
        }
        self.bodies.insert(id, chunk);
        true
    }

    pub(crate) fn insert_rebuilt(&mut self, record: MemoryRecord) -> Result<(), MemoryError> {
        if record.memory_version != self.next_memory_version {
            return Err(MemoryError::InvalidVersion);
        }
        self.insert(record)?;
        self.next_memory_version = self
            .next_memory_version
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        Ok(())
    }

    pub(crate) fn validate_bodies(&self) -> Result<(), MemoryError> {
        for record in &self.records {
            if !self.bodies.contains_key(&record.body_id) {
                return Err(MemoryError::MissingBody);
            }
            if let Some(inference) = &record.temporal_inference {
                validate_temporal_inference_binding(
                    record.body_id,
                    record.source_time_ns,
                    inference,
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn validate_provenance(&self, archive: &crate::Archive) -> Result<(), MemoryError> {
        let mut episode_nodes: HashMap<_, HashSet<&str>> = HashMap::new();
        for record in &self.records {
            match (record.source_episode_id, record.source_node_id.as_deref()) {
                (Some(episode_id), Some(node_id)) => {
                    episode_nodes.entry(episode_id).or_default().insert(node_id);
                }
                (None, None) => {}
                _ => return Err(MemoryError::InvalidProvenance),
            }
            match (
                record.content_source_conversation_id.as_deref(),
                record.content_source_node_id.as_deref(),
            ) {
                (Some(conversation_id), Some(node_id))
                    if archive.has_node(conversation_id, node_id) => {}
                (None, None) => {}
                _ => return Err(MemoryError::InvalidProvenance),
            }
            match (
                record.grounding_source_conversation_id.as_deref(),
                record.grounding_source_node_id.as_deref(),
            ) {
                (Some(conversation_id), Some(node_id))
                    if archive.has_node(conversation_id, node_id) => {}
                (None, None) => {}
                _ => return Err(MemoryError::InvalidProvenance),
            }
        }
        for (episode_id, node_ids) in episode_nodes {
            if !archive
                .episode_contains_all_nodes(episode_id, &node_ids)
                .map_err(|_| MemoryError::InvalidProvenance)?
            {
                return Err(MemoryError::InvalidProvenance);
            }
        }
        Ok(())
    }

    fn insert(&mut self, record: MemoryRecord) -> Result<(), MemoryError> {
        if record.revision == 0 || record.memory_version == 0 || record.global_version == 0 {
            return Err(MemoryError::InvalidVersion);
        }
        if let Some(index) = self.by_mutation.get(&record.mutation_id).copied() {
            return if self.records[index] == record {
                Ok(())
            } else {
                Err(MemoryError::MutationConflict)
            };
        }
        let expected = self
            .current
            .get(&record.id)
            .map(|index| self.records[*index].revision + 1)
            .unwrap_or(1);
        if record.revision != expected {
            return Err(MemoryError::RevisionConflict);
        }
        if let Some(index) = self.current.get(&record.id).copied() {
            if self.records[index].body_id != record.body_id {
                return Err(MemoryError::SemanticMutation);
            }
        }
        let index = self.records.len();
        self.by_mutation.insert(record.mutation_id.clone(), index);
        self.current.insert(record.id, index);
        self.records.push(record);
        Ok(())
    }

    fn body_bytes(
        &self,
        container: &mut Container,
        id: MemoryBodyId,
    ) -> Result<Vec<u8>, MemoryError> {
        let chunk = self.bodies.get(&id).ok_or(MemoryError::MissingBody)?;
        let payload = container.read(*chunk)?;
        if let Some((stored_id, body)) = decode_body(&payload)? {
            return if stored_id == id {
                Ok(body)
            } else {
                Err(MemoryError::CorruptBody)
            };
        }
        if let Some(completion) = crate::insomnia::completion::decode_completion(&payload)
            .map_err(MemoryError::CorruptRecord)?
        {
            return completion
                .bodies
                .into_iter()
                .find(|body| body.id == id)
                .map(|body| body.bytes)
                .ok_or(MemoryError::MissingBody);
        }
        Err(MemoryError::CorruptBody)
    }

    fn validate_routing_binding(
        &self,
        metadata: &MemoryRoutingMetadata,
    ) -> Result<(), MemoryError> {
        if metadata.entity_mentions.len() > MAX_MEMORY_ENTITY_MENTIONS {
            return Err(MemoryError::InvalidField("Memory routing metadata count"));
        }
        if self.current_body_id(metadata.memory_id)? != metadata.body_id {
            return Err(MemoryError::InvalidField("Memory routing metadata binding"));
        }
        for mention in &metadata.entity_mentions {
            if mention.text.is_empty()
                || mention.text.len() > MAX_MEMORY_ROUTING_TEXT_BYTES
                || mention.start_byte >= mention.end_byte
            {
                return Err(MemoryError::InvalidField("Memory entity mention"));
            }
        }
        Ok(())
    }

    fn validate_routing_text(
        &self,
        container: &mut Container,
        metadata: &MemoryRoutingMetadata,
    ) -> Result<(), MemoryError> {
        let body = self.body_bytes(container, metadata.body_id)?;
        let (title, content) = decode_memory_body(&body)?;
        for mention in &metadata.entity_mentions {
            let source = match mention.field {
                MemoryTextField::Title => title.as_str(),
                MemoryTextField::Content => content.as_str(),
            };
            let start = usize::try_from(mention.start_byte)
                .map_err(|_| MemoryError::InvalidField("Memory entity mention span"))?;
            let end = usize::try_from(mention.end_byte)
                .map_err(|_| MemoryError::InvalidField("Memory entity mention span"))?;
            if !source.is_char_boundary(start)
                || !source.is_char_boundary(end)
                || source.get(start..end) != Some(mention.text.as_str())
            {
                return Err(MemoryError::InvalidField("Memory entity mention span"));
            }
        }
        Ok(())
    }

    fn put_body(
        &mut self,
        container: &mut Container,
        title: &str,
        content: &str,
    ) -> Result<MemoryBodyId, MemoryError> {
        let id = memory_body_id(title, content);
        let bytes = memory_body_bytes(title, content);
        if self.bodies.contains_key(&id) {
            let stored = self.body_bytes(container, id)?;
            return if stored == bytes {
                Ok(id)
            } else {
                Err(MemoryError::HashCollision)
            };
        }
        let chunk = container.append(&encode_body(id, &bytes)?)?;
        self.bodies.insert(id, chunk);
        Ok(id)
    }

    fn resolve_record(
        &self,
        container: &mut Container,
        record: &MemoryRecord,
    ) -> Result<Memory, MemoryError> {
        let body = self.body_bytes(container, record.body_id)?;
        let (title, content) = decode_memory_body(&body)?;
        Ok(Memory {
            id: record.id,
            revision: record.revision,
            category: record.category.clone(),
            memory_type: record.memory_type.clone(),
            authority_kind: record.authority_kind.clone(),
            temporal_status: record.temporal_status.clone(),
            title,
            content,
            scope: record.scope.clone(),
            lifecycle_state: record.lifecycle_state.clone(),
            archived: record.archived,
            superseded_by: record.superseded_by,
            parent_id: record.parent_id,
            source_node_id: record.source_node_id.clone(),
            content_source_conversation_id: record.content_source_conversation_id.clone(),
            content_source_node_id: record.content_source_node_id.clone(),
            grounding_source_conversation_id: record.grounding_source_conversation_id.clone(),
            grounding_source_node_id: record.grounding_source_node_id.clone(),
            source_episode_id: record.source_episode_id,
            source_time_ns: record.source_time_ns,
            source_ref: record.source_ref.clone(),
            temporal_inference: record.temporal_inference.clone(),
            routing_metadata: self.routing_metadata.get(&record.id).cloned(),
            mutation_id: record.mutation_id.clone(),
            created_at_ns: record.created_at_ns,
            updated_at_ns: record.updated_at_ns,
            global_version: record.global_version,
            memory_version: record.memory_version,
        })
    }
}

fn normalize_entity_surface(value: &str) -> String {
    value.trim().to_lowercase()
}

fn decode_memory_body(bytes: &[u8]) -> Result<(String, String), MemoryError> {
    if bytes.len() < 16 {
        return Err(MemoryError::CorruptBody);
    }
    let title_len = u64::from_le_bytes(bytes[0..8].try_into().unwrap()) as usize;
    let title_end = 8usize
        .checked_add(title_len)
        .ok_or(MemoryError::FieldTooLarge)?;
    let content_len_end = title_end.checked_add(8).ok_or(MemoryError::FieldTooLarge)?;
    let title = bytes.get(8..title_end).ok_or(MemoryError::CorruptBody)?;
    let content_len_raw = bytes
        .get(title_end..content_len_end)
        .ok_or(MemoryError::CorruptBody)?;
    let content_len = u64::from_le_bytes(content_len_raw.try_into().unwrap()) as usize;
    let content_end = content_len_end
        .checked_add(content_len)
        .ok_or(MemoryError::FieldTooLarge)?;
    let content = bytes
        .get(content_len_end..content_end)
        .ok_or(MemoryError::CorruptBody)?;
    if content_end != bytes.len() {
        return Err(MemoryError::CorruptBody);
    }
    Ok((
        String::from_utf8(title.to_vec()).map_err(|_| MemoryError::InvalidUtf8)?,
        String::from_utf8(content.to_vec()).map_err(|_| MemoryError::InvalidUtf8)?,
    ))
}

fn validate_draft(draft: &MemoryDraft) -> Result<(), MemoryError> {
    for (value, field) in [
        (&draft.category, "category"),
        (&draft.memory_type, "memory type"),
        (&draft.authority_kind, "authority kind"),
        (&draft.temporal_status, "temporal status"),
        (&draft.title, "title"),
        (&draft.content, "content"),
        (&draft.scope, "scope"),
        (&draft.lifecycle_state, "lifecycle state"),
        (&draft.mutation_id, "mutation id"),
    ] {
        if value.trim().is_empty() {
            return Err(MemoryError::InvalidField(field));
        }
    }
    if !matches!(
        draft.authority_kind.as_str(),
        "direct" | "correction" | "adoption" | "retention" | "unknown"
    ) {
        return Err(MemoryError::InvalidField("authority kind"));
    }
    if !matches!(
        draft.temporal_status.as_str(),
        "current" | "future" | "historical" | "unknown"
    ) {
        return Err(MemoryError::InvalidField("temporal status"));
    }
    if draft.updated_at_ns < draft.created_at_ns {
        return Err(MemoryError::InvalidField("updated_at_ns"));
    }
    Ok(())
}

fn validate_temporal_inference_binding(
    body_id: MemoryBodyId,
    source_time_ns: Option<i64>,
    inference: &MemoryTemporalInference,
) -> Result<(), MemoryError> {
    if inference.body_id != body_id
        || inference.source_time_ns != source_time_ns
        || inference.inference.resolutions.is_empty()
    {
        return Err(MemoryError::InvalidField("temporal inference binding"));
    }
    Ok(())
}

fn validate_source_ref(source_ref: Option<&MemorySourceRef>) -> Result<(), MemoryError> {
    let Some(source_ref) = source_ref else {
        return Ok(());
    };
    if source_ref.owner_id.trim().is_empty() || source_ref.source_node_id.trim().is_empty() {
        return Err(MemoryError::InvalidProvenance);
    }
    for (conversation_id, node_id) in [
        (
            source_ref.content_source_conversation_id.as_deref(),
            source_ref.content_source_node_id.as_deref(),
        ),
        (
            source_ref.grounding_source_conversation_id.as_deref(),
            source_ref.grounding_source_node_id.as_deref(),
        ),
    ] {
        match (conversation_id, node_id) {
            (None, None) => {}
            (Some(conversation_id), Some(node_id))
                if !conversation_id.trim().is_empty() && !node_id.trim().is_empty() => {}
            _ => return Err(MemoryError::InvalidProvenance),
        }
    }
    Ok(())
}

fn same_draft(memory: &Memory, draft: &MemoryDraft) -> bool {
    memory.category == draft.category
        && memory.memory_type == draft.memory_type
        && memory.authority_kind == draft.authority_kind
        && memory.temporal_status == draft.temporal_status
        && memory.title == draft.title
        && memory.content == draft.content
        && memory.scope == draft.scope
        && memory.lifecycle_state == draft.lifecycle_state
        && memory.archived == draft.archived
        && memory.superseded_by == draft.superseded_by
        && memory.parent_id == draft.parent_id
        && memory.source_node_id == draft.source_node_id
        && memory.content_source_conversation_id == draft.content_source_conversation_id
        && memory.content_source_node_id == draft.content_source_node_id
        && memory.grounding_source_conversation_id == draft.grounding_source_conversation_id
        && memory.grounding_source_node_id == draft.grounding_source_node_id
        && memory.source_episode_id == draft.source_episode_id
        && memory.source_time_ns == draft.source_time_ns
        && memory.mutation_id == draft.mutation_id
        && memory.created_at_ns == draft.created_at_ns
        && memory.updated_at_ns == draft.updated_at_ns
}
