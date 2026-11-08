use super::*;

/// Exact standard Memory frames carried by an owner-authenticated management write.
pub enum GroupedManagementEffect {
    Correction {
        creation: Vec<u8>,
        supersession: GroupedMemorySupersession,
    },
    Retirement(Vec<u8>),
}

pub struct PreparedMemoryCorrection {
    pub body: Option<Vec<u8>>,
    pub creation: Vec<u8>,
    pub supersession: GroupedMemorySupersession,
}

impl MemoryStore {
    pub fn prepare_management_correction(
        &self,
        older: MemoryId,
        revision: u64,
        draft: MemoryDraft,
    ) -> Result<PreparedMemoryCorrection, MemoryError> {
        validate_draft(&draft)?;
        let newer = memory_id(&draft.mutation_id);
        if self.contains_memory(newer) {
            return Err(MemoryError::MutationConflict);
        }
        let body_id = memory_body_id(&draft.title, &draft.content);
        self.require_unpruned_body(body_id)?;
        let body = MemoryBody {
            title: draft.title.clone(),
            content: draft.content.clone(),
        };
        let body_frame = match self.state.bodies.get(&body_id) {
            Some(previous) if previous != &body => return Err(MemoryError::HashCollision),
            Some(_) => None,
            None => Some(encode_body(body_id, &body)?),
        };
        let creation = encode_creation(&record_for_draft(
            &draft,
            None,
            newer,
            body_id,
            1,
            self.state.next_memory_version,
        )?)?;
        let mut projected = self.clone();
        if let Some(bytes) = &body_frame {
            projected.replay(bytes)?;
        }
        projected.replay(&creation)?;
        let mutation_id = format!("{}:supersession", draft.mutation_id);
        let delta = projected.prepare_grouped_supersession(
            older,
            revision,
            newer,
            mutation_id.clone(),
            draft.updated_at_ns,
        )?;
        Ok(PreparedMemoryCorrection {
            body: body_frame,
            creation,
            supersession: GroupedMemorySupersession {
                older,
                newer,
                previous_revision: revision,
                updated_at_ns: draft.updated_at_ns,
                mutation_id,
                delta,
            },
        })
    }

    pub fn prepare_management_retirement(
        &self,
        id: MemoryId,
        revision: u64,
    ) -> Result<Vec<u8>, MemoryError> {
        let current = self.current(id)?;
        if current.record.revision != revision {
            return Err(MemoryError::RevisionConflict);
        }
        let next = revision
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        let version = self.state.next_memory_version;
        version
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        Ok(encode_tombstone(id, next, version))
    }

    /// Apply the standard body frame and the accepted grouped effect after sync.
    pub fn apply_management_group(
        &mut self,
        body: Option<&[u8]>,
        effect: GroupedManagementEffect,
    ) -> Result<(), MemoryError> {
        let mut next = self.clone();
        if let Some(body) = body {
            if !body.starts_with(BODY) {
                return Err(MemoryError::Corrupt("invalid grouped management body"));
            }
            next.replay(body)?;
        }
        next.apply_management_effect(effect)?;
        *self = next;
        Ok(())
    }

    pub(super) fn apply_management_effect(
        &mut self,
        effect: GroupedManagementEffect,
    ) -> Result<(), MemoryError> {
        if !self.initialized {
            return Err(MemoryError::Corrupt(
                "management write precedes Memory format",
            ));
        }
        match effect {
            GroupedManagementEffect::Correction {
                creation,
                supersession,
            } => {
                let record = decode_grouped_creation(&creation)?;
                if record.id != supersession.newer
                    || record.authority_kind != "direct"
                    || record.source_episode_id.is_some()
                    || record.archived
                    || record.superseded_by.is_some()
                    || record.mutation_id
                        != supersession
                            .mutation_id
                            .strip_suffix(":supersession")
                            .ok_or(MemoryError::Corrupt("invalid correction mutation"))?
                {
                    return Err(MemoryError::Corrupt("invalid correction creation"));
                }
                self.replay(&creation)?;
                self.apply_grouped_supersession(&supersession)
            }
            GroupedManagementEffect::Retirement(tombstone) => {
                if !tombstone.starts_with(TOMBSTONE) {
                    return Err(MemoryError::Corrupt("invalid grouped retirement tombstone"));
                }
                self.replay(&tombstone)
            }
        }
    }
}
