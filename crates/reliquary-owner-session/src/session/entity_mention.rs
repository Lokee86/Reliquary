use super::*;
use reliquary_memory_core::PayloadState;
use reliquary_perception_store::{
    MemoryEntityResolutionStatus, MemoryTextField, PRINCIPAL_ENTITY_KIND,
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
}

fn commit(
    common: &mut CommonState,
    container: &mut Container,
    request: EntityMentionCommit,
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

impl RelWriter {
    pub fn commit_entity_mention(
        &mut self,
        request: EntityMentionCommit,
    ) -> Result<MemoryEntityResolution, OwnerSessionError> {
        self.ensure_write_ready()?;
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
    /// Owner-local inputs for one mention. The caller releases the writer
    /// lock before evaluating the snapshot.
    pub fn entity_mention_inputs(
        &self,
        key: MemoryEntityMentionKey,
    ) -> Result<(MemoryRevision, Vec<Entity>, Option<MemoryEntityResolution>), OwnerSessionError>
    {
        Ok((
            self.common.memories.current(MemoryId(key.memory_id))?,
            self.common.perception.entities().cloned().collect(),
            self.common.perception.mention(key).cloned(),
        ))
    }

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
