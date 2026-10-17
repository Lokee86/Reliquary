use super::validation::{check_revision, next_revision};
use super::{EgoRecord, EgoStore};
use crate::ego_codec::validate_anchor_text;
use crate::{Container, EgoAnchor, EgoAnchorId, EgoAnchorPriority, EgoError};
use uuid::Uuid;

impl EgoStore {
    pub(crate) fn create_anchor(
        &mut self,
        container: &mut Container,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<EgoAnchor, EgoError> {
        validate_anchor_text(&text)?;
        let id = loop {
            let candidate = EgoAnchorId(*Uuid::new_v4().as_bytes());
            if !self.anchors.contains_key(&candidate) {
                break candidate;
            }
        };
        let record = EgoRecord::Anchor {
            ego_version: self.next_version()?,
            id,
            revision: 1,
            deleted: false,
            priority,
            text: text.clone(),
        };
        self.append_apply(container, record)?;
        Ok(EgoAnchor {
            id,
            revision: 1,
            priority,
            text,
        })
    }

    pub(crate) fn update_anchor(
        &mut self,
        container: &mut Container,
        id: EgoAnchorId,
        expected_revision: u64,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<(EgoAnchor, bool), EgoError> {
        validate_anchor_text(&text)?;
        self.anchor_lifecycle(id)
            .ok_or(EgoError::NotFound("Anchor"))?
            .require_active(crate::owner_lifecycle::LifecycleTarget::EgoAnchor(id))?;
        let state = self
            .anchors
            .get(&id)
            .filter(|state| !state.deleted)
            .ok_or(EgoError::NotFound("Anchor"))?;
        check_revision(expected_revision, state.revision)?;
        if state.priority == priority && state.text == text {
            return Ok((self.anchor(id).unwrap(), false));
        }
        let revision = next_revision(state.revision)?;
        let record = EgoRecord::Anchor {
            ego_version: self.next_version()?,
            id,
            revision,
            deleted: false,
            priority,
            text: text.clone(),
        };
        self.append_apply(container, record)?;
        Ok((
            EgoAnchor {
                id,
                revision,
                priority,
                text,
            },
            true,
        ))
    }

    pub(crate) fn delete_anchor(
        &mut self,
        container: &mut Container,
        id: EgoAnchorId,
        expected_revision: u64,
    ) -> Result<bool, EgoError> {
        let state = self.anchors.get(&id).ok_or(EgoError::NotFound("Anchor"))?;
        if state.deleted {
            check_revision(expected_revision, state.revision - 1)?;
            return Ok(false);
        }
        check_revision(expected_revision, state.revision)?;
        let record = EgoRecord::Anchor {
            ego_version: self.next_version()?,
            id,
            revision: next_revision(state.revision)?,
            deleted: true,
            priority: state.priority,
            text: String::new(),
        };
        self.append_apply(container, record)?;
        Ok(true)
    }
}
