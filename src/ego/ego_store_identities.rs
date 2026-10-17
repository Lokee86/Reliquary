use super::validation::{check_revision, next_revision};
use super::{EgoRecord, EgoStore, LEGACY_IDENTITY_ID};
use crate::ego_codec::validate_text;
use crate::{Container, EgoError, EgoIdentity, EgoIdentityId};
use uuid::Uuid;

impl EgoStore {
    pub(crate) fn put_identity(
        &mut self,
        container: &mut Container,
        expected_revision: u64,
        text: String,
    ) -> Result<(EgoIdentity, bool), EgoError> {
        self.require_phylactery("Identity is PHY-owned")?;
        if let Some(id) = self.active_identity_id {
            let current = self.identity(id).ok_or(EgoError::NotFound("Identity"))?;
            return self.update_identity(container, id, expected_revision, current.name, text);
        }
        check_revision(expected_revision, 0)?;
        let identity = self.create_identity(container, "Default".into(), text)?;
        Ok((identity, true))
    }

    pub(crate) fn create_identity(
        &mut self,
        container: &mut Container,
        name: String,
        text: String,
    ) -> Result<EgoIdentity, EgoError> {
        self.require_phylactery("Identity is PHY-owned")?;
        validate_text("identity name", &name)?;
        validate_text("identity", &text)?;
        let id = loop {
            let candidate = EgoIdentityId(*Uuid::new_v4().as_bytes());
            if candidate != LEGACY_IDENTITY_ID && !self.identities.contains_key(&candidate) {
                break candidate;
            }
        };
        let activate = self.active_identity_id.is_none();
        self.append_apply(
            container,
            EgoRecord::Identity {
                ego_version: self.next_version()?,
                id,
                revision: 1,
                deleted: false,
                activate,
                name: name.clone(),
                text: text.clone(),
            },
        )?;
        Ok(EgoIdentity {
            id,
            revision: 1,
            name,
            text,
        })
    }

    pub(crate) fn update_identity(
        &mut self,
        container: &mut Container,
        id: EgoIdentityId,
        expected_revision: u64,
        name: String,
        text: String,
    ) -> Result<(EgoIdentity, bool), EgoError> {
        self.require_phylactery("Identity is PHY-owned")?;
        validate_text("identity name", &name)?;
        validate_text("identity", &text)?;
        self.identity_lifecycle(id)
            .ok_or(EgoError::NotFound("Identity"))?
            .require_active(crate::owner_lifecycle::LifecycleTarget::EgoIdentity(id))?;
        let state = self
            .identities
            .get(&id)
            .filter(|state| !state.deleted)
            .ok_or(EgoError::NotFound("Identity"))?;
        check_revision(expected_revision, state.revision)?;
        if state.name == name && state.text == text {
            return Ok((self.identity(id).unwrap(), false));
        }
        let revision = next_revision(state.revision)?;
        self.append_apply(
            container,
            EgoRecord::Identity {
                ego_version: self.next_version()?,
                id,
                revision,
                deleted: false,
                activate: false,
                name: name.clone(),
                text: text.clone(),
            },
        )?;
        Ok((
            EgoIdentity {
                id,
                revision,
                name,
                text,
            },
            true,
        ))
    }

    pub(crate) fn activate_identity(
        &mut self,
        container: &mut Container,
        id: EgoIdentityId,
    ) -> Result<bool, EgoError> {
        self.require_phylactery("Identity is PHY-owned")?;
        self.require_live_identity(id)?;
        if self.active_identity_id == Some(id) {
            return Ok(false);
        }
        self.append_apply(
            container,
            EgoRecord::IdentityActive {
                ego_version: self.next_version()?,
                id,
            },
        )?;
        Ok(true)
    }

    pub(crate) fn delete_identity(
        &mut self,
        container: &mut Container,
        id: EgoIdentityId,
        expected_revision: u64,
    ) -> Result<bool, EgoError> {
        self.require_phylactery("Identity is PHY-owned")?;
        let state = self
            .identities
            .get(&id)
            .ok_or(EgoError::NotFound("Identity"))?;
        if state.deleted {
            check_revision(expected_revision, state.revision - 1)?;
            return Ok(false);
        }
        check_revision(expected_revision, state.revision)?;
        if self.active_identity_id == Some(id) && self.live_identity_count() > 1 {
            return Err(EgoError::ActiveIdentityDeletion);
        }
        self.append_apply(
            container,
            EgoRecord::Identity {
                ego_version: self.next_version()?,
                id,
                revision: next_revision(state.revision)?,
                deleted: true,
                activate: false,
                name: String::new(),
                text: String::new(),
            },
        )?;
        Ok(true)
    }
}
