use super::{EntityResolutionStore, validate_status};
use crate::{
    Container, EntityId, EntityResolutionPending, EntityResolutionReason,
    MAX_ENTITY_RESOLUTION_CANDIDATES, MemoryEntityMentionKey, MemoryEntityResolutionStatus,
    MemoryError,
};

impl EntityResolutionStore {
    pub(crate) fn put_resolved(
        &mut self,
        container: &mut Container,
        key: MemoryEntityMentionKey,
        expected_revision: u32,
        entity_id: EntityId,
        reason: EntityResolutionReason,
        now_ns: i64,
    ) -> Result<bool, MemoryError> {
        self.validate_expected_revision(key, expected_revision)?;
        if let Some(current) = self.current.get(&key) {
            if current.status == (MemoryEntityResolutionStatus::Resolved { entity_id, reason }) {
                return Ok(false);
            }
            reject_terminal(&current.status)?;
        }
        self.append(
            container,
            key,
            now_ns,
            MemoryEntityResolutionStatus::Resolved { entity_id, reason },
        )
    }

    pub(crate) fn put_rejected(
        &mut self,
        container: &mut Container,
        key: MemoryEntityMentionKey,
        expected_revision: u32,
        reason: EntityResolutionReason,
        now_ns: i64,
    ) -> Result<bool, MemoryError> {
        self.validate_expected_revision(key, expected_revision)?;
        if let Some(current) = self.current.get(&key) {
            if current.status == (MemoryEntityResolutionStatus::Rejected { reason }) {
                return Ok(false);
            }
            reject_terminal(&current.status)?;
        }
        self.append(
            container,
            key,
            now_ns,
            MemoryEntityResolutionStatus::Rejected { reason },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn put_unresolved(
        &mut self,
        container: &mut Container,
        key: MemoryEntityMentionKey,
        expected_revision: u32,
        mut candidate_entity_ids: Vec<EntityId>,
        reason: EntityResolutionReason,
        candidate_set_fingerprint: [u8; 32],
        context_fingerprint: [u8; 32],
        now_ns: i64,
    ) -> Result<bool, MemoryError> {
        self.validate_expected_revision(key, expected_revision)?;
        normalize_candidates(&mut candidate_entity_ids)?;
        let (first_seen_at_ns, attempt_count) = match self.current.get(&key) {
            None => (now_ns, 1),
            Some(current) => match &current.status {
                MemoryEntityResolutionStatus::Pending(value) => {
                    if same_evidence(
                        value.candidate_set_fingerprint,
                        value.context_fingerprint,
                        candidate_set_fingerprint,
                        context_fingerprint,
                    ) {
                        return Ok(false);
                    }
                    validate_retry_time(now_ns, value.last_attempt_at_ns)?;
                    (value.first_seen_at_ns, next_attempt(value.attempt_count)?)
                }
                MemoryEntityResolutionStatus::Dormant(value) => {
                    if same_evidence(
                        value.candidate_set_fingerprint,
                        value.context_fingerprint,
                        candidate_set_fingerprint,
                        context_fingerprint,
                    ) {
                        return Ok(false);
                    }
                    validate_retry_time(now_ns, value.last_attempt_at_ns)?;
                    (value.first_seen_at_ns, next_attempt(value.attempt_count)?)
                }
                status => {
                    reject_terminal(status)?;
                    unreachable!()
                }
            },
        };
        self.append(
            container,
            key,
            now_ns,
            MemoryEntityResolutionStatus::Pending(EntityResolutionPending {
                candidate_entity_ids,
                reason,
                candidate_set_fingerprint,
                context_fingerprint,
                first_seen_at_ns,
                last_attempt_at_ns: now_ns,
                attempt_count,
            }),
        )
    }

    pub(crate) fn retarget_resolved(
        &mut self,
        container: &mut Container,
        key: MemoryEntityMentionKey,
        from: EntityId,
        to: EntityId,
        now_ns: i64,
    ) -> Result<bool, MemoryError> {
        let current = self
            .current
            .get(&key)
            .cloned()
            .ok_or(MemoryError::InvalidField("missing Entity resolution"))?;
        match current.status {
            MemoryEntityResolutionStatus::Resolved { entity_id, .. } if entity_id == to => {
                Ok(false)
            }
            MemoryEntityResolutionStatus::Resolved { entity_id, reason } if entity_id == from => {
                self.append(
                    container,
                    key,
                    now_ns.max(current.updated_at_ns),
                    MemoryEntityResolutionStatus::Resolved {
                        entity_id: to,
                        reason,
                    },
                )
            }
            _ => Err(MemoryError::InvalidField(
                "Entity resolution retarget source",
            )),
        }
    }

    pub(crate) fn retarget_entity_references(
        &mut self,
        container: &mut Container,
        from: EntityId,
        to: EntityId,
        now_ns: i64,
    ) -> Result<usize, MemoryError> {
        let values = self.current.values().cloned().collect::<Vec<_>>();
        let mut changed = 0usize;
        for current in values {
            let status = match current.status {
                MemoryEntityResolutionStatus::Resolved { entity_id, reason }
                    if entity_id == from =>
                {
                    Some(MemoryEntityResolutionStatus::Resolved {
                        entity_id: to,
                        reason,
                    })
                }
                MemoryEntityResolutionStatus::Pending(mut value)
                    if value.candidate_entity_ids.contains(&from) =>
                {
                    replace_candidate(&mut value.candidate_entity_ids, from, to);
                    value.candidate_set_fingerprint = [0; 32];
                    Some(MemoryEntityResolutionStatus::Pending(value))
                }
                MemoryEntityResolutionStatus::Dormant(mut value)
                    if value.candidate_entity_ids.contains(&from) =>
                {
                    replace_candidate(&mut value.candidate_entity_ids, from, to);
                    value.candidate_set_fingerprint = [0; 32];
                    Some(MemoryEntityResolutionStatus::Dormant(value))
                }
                _ => None,
            };
            if let Some(status) = status {
                self.append(
                    container,
                    current.key,
                    now_ns.max(current.updated_at_ns),
                    status,
                )?;
                changed += 1;
            }
        }
        Ok(changed)
    }

    pub(crate) fn validate_expected_revision(
        &self,
        key: MemoryEntityMentionKey,
        expected_revision: u32,
    ) -> Result<(), MemoryError> {
        let current = self
            .current
            .get(&key)
            .map(|value| value.revision)
            .unwrap_or(0);
        if current != expected_revision {
            return Err(MemoryError::RevisionConflict);
        }
        Ok(())
    }
}

fn reject_terminal(status: &MemoryEntityResolutionStatus) -> Result<(), MemoryError> {
    if matches!(
        status,
        MemoryEntityResolutionStatus::Resolved { .. }
            | MemoryEntityResolutionStatus::Rejected { .. }
    ) {
        Err(MemoryError::InvalidField("terminal Entity resolution"))
    } else {
        Ok(())
    }
}

fn replace_candidate(values: &mut Vec<EntityId>, from: EntityId, to: EntityId) {
    for value in values.iter_mut() {
        if *value == from {
            *value = to;
        }
    }
    values.sort();
    values.dedup();
}

pub(super) fn normalize_candidates(values: &mut Vec<EntityId>) -> Result<(), MemoryError> {
    values.sort();
    values.dedup();
    if values.len() > MAX_ENTITY_RESOLUTION_CANDIDATES {
        return Err(MemoryError::FieldTooLarge);
    }
    Ok(())
}

fn next_attempt(value: u16) -> Result<u16, MemoryError> {
    value.checked_add(1).ok_or(MemoryError::VersionExhausted)
}

fn validate_retry_time(now_ns: i64, last_ns: i64) -> Result<(), MemoryError> {
    if now_ns < last_ns {
        Err(MemoryError::InvalidField("Entity resolution timestamp"))
    } else {
        Ok(())
    }
}

fn same_evidence(
    old_candidates: [u8; 32],
    old_context: [u8; 32],
    new_candidates: [u8; 32],
    new_context: [u8; 32],
) -> bool {
    old_candidates == new_candidates && old_context == new_context
}
