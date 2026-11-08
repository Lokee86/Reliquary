use super::*;
use reliquary_memory_core::PayloadState;
use reliquary_perception_store::{
    EntityDraft, MemoryEntityResolutionStatus, MemoryTextField, PRINCIPAL_ENTITY_KIND,
};

/// A source-bound mention publication against one owner and revision.
#[derive(Clone)]
pub struct EntityMentionCommit {
    pub owner_id: [u8; 16],
    pub memory_revision: u64,
    pub body_id: reliquary_memory_core::MemoryBodyId,
    pub key: MemoryEntityMentionKey,
    pub surface: String,
    pub expected_revision: u32,
    pub updated_at_ns: i64,
    pub status: MemoryEntityResolutionStatus,
    pub target_revisions: Vec<(EntityId, u64)>,
    pub new_entity: Option<(String, String)>,
}

fn commit(
    common: &mut CommonState,
    container: &mut Container,
    mut request: EntityMentionCommit,
) -> Result<MemoryEntityResolution, OwnerSessionError> {
    if request.owner_id != container.owner_id() {
        return Err(OwnerSessionError::Record(
            "mention belongs to another owner".into(),
        ));
    }
    let memory = common.memories.current(MemoryId(request.key.memory_id))?;
    let PayloadState::Available(body) = memory.payload else {
        return Err(OwnerSessionError::Record(
            "mention body is unavailable".into(),
        ));
    };
    let text = match request.key.field {
        MemoryTextField::Title => &body.title,
        MemoryTextField::Content => &body.content,
    };
    if memory.record.revision != request.memory_revision
        || memory.record.body_id != request.body_id
        || request.surface.trim().is_empty()
        || text.get(request.key.start_byte as usize..request.key.end_byte as usize)
            != Some(request.surface.as_str())
    {
        return Err(OwnerSessionError::Record("mention source changed".into()));
    }

    if let Some((kind, summary)) = request.new_entity.take() {
        let MemoryEntityResolutionStatus::Unresolved { reason } = request.status else {
            return Err(OwnerSessionError::Record(
                "invalid Entity creation command".into(),
            ));
        };
        if kind == PRINCIPAL_ENTITY_KIND
            || kind == "unknown"
            || !request.target_revisions.is_empty()
        {
            return Err(OwnerSessionError::Record(
                "invalid generic Entity kind or target".into(),
            ));
        }
        let mutation_id = format!(
            "entity-mention:{}:{}:{}:{}",
            request
                .key
                .memory_id
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            match request.key.field {
                MemoryTextField::Title => "title",
                MemoryTextField::Content => "content",
            },
            request.key.start_byte,
            request.key.end_byte,
        );
        let existing = common.perception.mention(request.key);
        let actual = existing.map_or(0, |value| value.revision);
        if actual != request.expected_revision {
            if let Some(current) = existing {
                if current.revision.checked_sub(1) == Some(request.expected_revision)
                    && current.updated_at_ns == request.updated_at_ns
                    && let MemoryEntityResolutionStatus::Resolved {
                        entity_id,
                        reason: stored_reason,
                    } = current.status
                    && stored_reason == reason
                    && common.perception.entity(entity_id).is_some_and(|entity| {
                        entity.mutation_id == mutation_id
                            && entity.canonical_name == request.surface
                            && entity.kind == kind
                            && entity.summary == summary
                    })
                {
                    return Ok(current.clone());
                }
            }
            return Err(OwnerSessionError::Record(
                "mention revision changed before Entity creation".into(),
            ));
        }
        if actual == u32::MAX
            || existing.is_some_and(|value| value.updated_at_ns > request.updated_at_ns)
        {
            return Err(OwnerSessionError::Record(
                "mention cannot advance before Entity creation".into(),
            ));
        }
        let normalized = normalize_surface(&request.surface);
        if common.perception.entities().any(|entity| {
            entity.kind != PRINCIPAL_ENTITY_KIND
                && entity.mutation_id != mutation_id
                && std::iter::once(&entity.canonical_name)
                    .chain(&entity.aliases)
                    .any(|name| normalize_surface(name) == normalized)
        }) {
            return Err(OwnerSessionError::Record(
                "matching Entity appeared before creation".into(),
            ));
        }
        let entity = common.perception.publish_entity(
            container,
            EntityDraft {
                canonical_name: request.surface.clone(),
                aliases: vec![],
                kind,
                summary,
                mutation_id,
                created_at_ns: memory.record.created_at_ns,
                updated_at_ns: memory.record.created_at_ns,
            },
        )?;
        request.status = MemoryEntityResolutionStatus::Resolved {
            entity_id: entity.id,
            reason,
        };
        request.target_revisions.push((entity.id, entity.revision));
    }

    let targets = match &request.status {
        MemoryEntityResolutionStatus::Resolved { entity_id, .. } => std::slice::from_ref(entity_id),
        MemoryEntityResolutionStatus::Ambiguous { candidates } => candidates,
        MemoryEntityResolutionStatus::Unresolved { .. } => &[],
    };
    if targets.len() != request.target_revisions.len()
        || targets
            .iter()
            .zip(&request.target_revisions)
            .any(|(id, (expected_id, revision))| {
                id != expected_id
                    || common.perception.entity(*id).is_none_or(|entity| {
                        entity.kind == PRINCIPAL_ENTITY_KIND || entity.revision != *revision
                    })
            })
    {
        return Err(OwnerSessionError::Record(
            "mention target is unavailable to generic Entity resolution".into(),
        ));
    }
    if let Some(current) = common.perception.mention(request.key) {
        if current.revision.checked_sub(1) == Some(request.expected_revision)
            && current.updated_at_ns == request.updated_at_ns
            && current.status == request.status
        {
            return Ok(current.clone());
        }
        if request.updated_at_ns < current.updated_at_ns {
            return Err(OwnerSessionError::Record(
                "mention timestamp predates current resolution".into(),
            ));
        }
    }
    Ok(common.perception.put_mention_resolution(
        container,
        request.key,
        request.expected_revision,
        request.updated_at_ns,
        request.status,
    )?)
}

fn normalize_surface(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

impl RelWriter {
    pub fn commit_entity_mention(
        &mut self,
        request: EntityMentionCommit,
    ) -> Result<MemoryEntityResolution, OwnerSessionError> {
        self.ensure_write_ready()?;
        self.commit_entity_mention_after_fence(request)
    }

    pub fn commit_native_import_entity_mention(
        &mut self,
        import_id: &str,
        request: EntityMentionCommit,
    ) -> Result<MemoryEntityResolution, OwnerSessionError> {
        self.validate_native_import_memory(import_id, MemoryId(request.key.memory_id))?;
        self.commit_entity_mention_after_fence(request)
    }

    fn commit_entity_mention_after_fence(
        &mut self,
        request: EntityMentionCommit,
    ) -> Result<MemoryEntityResolution, OwnerSessionError> {
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "REL writer requires reopen before mention publication".into(),
            ));
        }
        let result = commit(&mut self.session.common, &mut self.container, request);
        let after = self.container.writer_lease_proof();
        if result.is_ok() {
            self.session.writer_lease_proof = after;
        } else if before != after
            || matches!(
                &result,
                Err(OwnerSessionError::Perception(PerceptionError::Container(_)))
            )
        {
            self.session.write_poisoned = true;
        }
        result
    }
}

impl PhyWriter {
    pub fn commit_entity_mention(
        &mut self,
        request: EntityMentionCommit,
    ) -> Result<MemoryEntityResolution, OwnerSessionError> {
        let before = self.container.writer_lease_proof();
        if self.poisoned || before.is_none() || before != self.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "PHY writer requires reopen before mention publication".into(),
            ));
        }
        let result = commit(&mut self.common, &mut self.container, request);
        let after = self.container.writer_lease_proof();
        if result.is_ok() {
            self.writer_lease_proof = after;
        } else if before != after
            || matches!(
                &result,
                Err(OwnerSessionError::Perception(PerceptionError::Container(_)))
            )
        {
            self.poisoned = true;
        }
        result
    }
}
