//! Owner-scoped Ego controls, separate from the background summary workers.

use super::*;
use reliquary_owner_session::{
    EgoAnchor, EgoAnchorId, EgoAnchorPriority, EgoIdentity, EgoIdentityId, EgoIdentitySelection,
};
use serde::{Deserialize, Serialize};
use std::sync::Weak;

mod context;
pub use context::{EgoContext, EgoContextBinding, EgoQueryVector};

#[derive(Clone, Copy)]
pub enum EgoScope {
    Rel(OwnerId),
    Phy(OwnerId),
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct EgoPinPatch {
    pub text: Option<String>,
    pub priority: Option<EgoAnchorPriority>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct EgoIdentityDraft {
    pub id: EgoIdentityId,
    pub revision: u64,
    pub name: String,
    pub text: String,
}

pub struct PreparedEgoIdentity {
    owner: OwnerId,
    instance: Uuid,
    mount: Weak<MountedPhyWriter>,
    draft: EgoIdentityDraft,
}

impl PreparedEgoIdentity {
    pub fn draft(&self) -> &EgoIdentityDraft {
        &self.draft
    }
}

impl Instance {
    pub(super) fn authorize_ego_phy(&self, owner: OwnerId) -> Result<(), GatewayError> {
        self.ensure_credential_active()?;
        if !self.phy_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedPhy {
                principal: self.principal.to_string(),
                owner,
            });
        }
        Ok(())
    }

    fn ego_phy_mount(&self, owner: OwnerId) -> Result<MountedOwner, GatewayError> {
        self.authorize_ego_phy(owner)?;
        let owners = self
            .owners
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        match owners.get(&owner) {
            Some(mount @ (MountedOwner::Phy(_) | MountedOwner::PhyWriter(_))) => Ok(mount.clone()),
            Some(_) => Err(GatewayError::WrongOwnerKind(owner)),
            None => Err(GatewayError::UnknownOwner(owner)),
        }
    }

    pub(super) fn ego_phy_writer_mount(
        &self,
        owner: OwnerId,
    ) -> Result<Arc<MountedPhyWriter>, GatewayError> {
        match self.ego_phy_mount(owner)? {
            MountedOwner::PhyWriter(mount) => Ok(mount),
            _ => Err(GatewayError::WriterUnavailable),
        }
    }

    pub(super) fn with_ego_phy_writer<T>(
        &self,
        owner: OwnerId,
        operation: impl FnOnce(&mut PhyWriter) -> Result<T, OwnerSessionError>,
    ) -> Result<T, GatewayError> {
        let mount = self.ego_phy_writer_mount(owner)?;
        let mut writer = mount
            .writer
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let _credential = self
            .credential_publication
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        self.authorize_ego_phy(owner)?;
        // A replaced mount cannot accept a write prepared for its predecessor.
        if !Arc::ptr_eq(&mount, &self.ego_phy_writer_mount(owner)?) {
            return Err(OwnerSessionError::ManagementStale.into());
        }
        Ok(operation(&mut writer)?)
    }

    pub(super) fn with_ego_phy<T>(
        &self,
        owner: OwnerId,
        read: impl FnOnce(EgoPhyRead<'_>) -> T,
    ) -> Result<T, GatewayError> {
        let value = match self.ego_phy_mount(owner)? {
            MountedOwner::Phy(session) => read(EgoPhyRead::Session(&session)),
            MountedOwner::PhyWriter(mount) => {
                let writer = mount
                    .writer
                    .lock()
                    .map_err(|_| GatewayError::RegistryPoisoned)?;
                self.authorize_ego_phy(owner)?;
                read(EgoPhyRead::Writer(&writer))
            }
            _ => return Err(GatewayError::WrongOwnerKind(owner)),
        };
        self.authorize_ego_phy(owner)?;
        Ok(value)
    }

    pub fn ego_pins(&self, scope: EgoScope) -> Result<Vec<EgoAnchor>, GatewayError> {
        match scope {
            EgoScope::Rel(owner) => {
                let pins =
                    self.with_reliquary(owner, |session| session.ego_anchors().cloned().collect())?;
                self.authorize_rel_access(owner)?;
                Ok(pins)
            }
            EgoScope::Phy(owner) => self.with_ego_phy(owner, |phy| phy.pins()),
        }
    }

    pub fn create_ego_pin(
        &self,
        scope: EgoScope,
        text: String,
        priority: EgoAnchorPriority,
    ) -> Result<EgoAnchor, GatewayError> {
        let id = EgoAnchorId(*Uuid::new_v4().as_bytes());
        match scope {
            EgoScope::Rel(owner) => self.with_rel_writer(owner, |writer| {
                let _credential = self
                    .credential_publication
                    .read()
                    .map_err(|_| GatewayError::RegistryPoisoned)?;
                self.authorize_rel_access(owner)?;
                Ok(writer.create_ego_anchor(id, priority, text)?)
            })?,
            EgoScope::Phy(owner) => self
                .with_ego_phy_writer(owner, |writer| writer.create_ego_anchor(id, priority, text)),
        }
    }

    pub fn modify_ego_pin(
        &self,
        scope: EgoScope,
        id: EgoAnchorId,
        expected_revision: u64,
        patch: EgoPinPatch,
    ) -> Result<EgoAnchor, GatewayError> {
        let parts = |current: Option<&EgoAnchor>| {
            let current = current.ok_or(OwnerSessionError::ManagementStale)?;
            Ok::<_, OwnerSessionError>((
                patch.priority.unwrap_or(current.priority),
                patch.text.unwrap_or_else(|| current.text.clone()),
            ))
        };
        match scope {
            EgoScope::Rel(owner) => self.with_rel_writer(owner, |writer| {
                let _credential = self
                    .credential_publication
                    .read()
                    .map_err(|_| GatewayError::RegistryPoisoned)?;
                self.authorize_rel_access(owner)?;
                let (priority, text) = parts(writer.read_session().ego_anchor(id))?;
                Ok(writer.update_ego_anchor(id, expected_revision, priority, text)?)
            })?,
            EgoScope::Phy(owner) => self.with_ego_phy_writer(owner, |writer| {
                let (priority, text) = parts(writer.ego_anchor(id))?;
                writer.update_ego_anchor(id, expected_revision, priority, text)
            }),
        }
    }

    pub fn delete_ego_pin(
        &self,
        scope: EgoScope,
        id: EgoAnchorId,
        expected_revision: u64,
    ) -> Result<(), GatewayError> {
        match scope {
            EgoScope::Rel(owner) => self.with_rel_writer(owner, |writer| {
                let _credential = self
                    .credential_publication
                    .read()
                    .map_err(|_| GatewayError::RegistryPoisoned)?;
                self.authorize_rel_access(owner)?;
                Ok(writer.delete_ego_anchor(id, expected_revision)?)
            })?,
            EgoScope::Phy(owner) => self.with_ego_phy_writer(owner, |writer| {
                writer.delete_ego_anchor(id, expected_revision)
            }),
        }
    }

    pub fn ego_identities(
        &self,
        owner: OwnerId,
    ) -> Result<(Option<EgoIdentitySelection>, Vec<EgoIdentity>), GatewayError> {
        self.with_ego_phy(owner, |phy| phy.identities())
    }

    pub fn create_ego_identity(
        &self,
        owner: OwnerId,
        name: String,
        text: String,
    ) -> Result<EgoIdentity, GatewayError> {
        self.with_ego_phy_writer(owner, |writer| {
            writer.create_ego_identity(EgoIdentityId(*Uuid::new_v4().as_bytes()), name, text)
        })
    }

    pub fn edit_ego_identity(
        &self,
        owner: OwnerId,
        draft: EgoIdentityDraft,
    ) -> Result<EgoIdentity, GatewayError> {
        self.with_ego_phy_writer(owner, |writer| {
            writer.update_ego_identity(draft.id, draft.revision, draft.name, draft.text)
        })
    }

    pub fn select_ego_identity(
        &self,
        owner: OwnerId,
        expected_selection: u64,
        target: Option<(EgoIdentityId, u64)>,
    ) -> Result<EgoIdentitySelection, GatewayError> {
        self.with_ego_phy_writer(owner, |writer| {
            writer.select_ego_identity(expected_selection, target)
        })
    }

    pub fn prepare_ego_identity(
        &self,
        owner: OwnerId,
        draft: EgoIdentityDraft,
    ) -> Result<PreparedEgoIdentity, GatewayError> {
        let mount = self.ego_phy_writer_mount(owner)?;
        self.with_ego_phy_writer(owner, |writer| {
            if writer.ego_identity(draft.id).map(|value| value.revision) != Some(draft.revision) {
                return Err(OwnerSessionError::ManagementStale);
            }
            Ok(())
        })?;
        Ok(PreparedEgoIdentity {
            owner,
            instance: self.instance_id,
            mount: Arc::downgrade(&mount),
            draft,
        })
    }

    /// The trusted host calls this only after displaying and receiving approval
    /// for the exact draft returned by the preparation.
    pub fn approve_ego_identity(
        &self,
        owner: OwnerId,
        approval: PreparedEgoIdentity,
    ) -> Result<EgoIdentity, GatewayError> {
        if owner != approval.owner || approval.instance != self.instance_id {
            return Err(OwnerSessionError::ManagementStale.into());
        }
        self.with_ego_phy_writer(owner, |writer| {
            if !self
                .ego_phy_writer_mount(owner)
                .is_ok_and(|mount| approval.mount.ptr_eq(&Arc::downgrade(&mount)))
            {
                return Err(OwnerSessionError::ManagementStale);
            }
            let draft = approval.draft;
            writer.update_ego_identity(draft.id, draft.revision, draft.name, draft.text)
        })
    }
}

pub(super) enum EgoPhyRead<'a> {
    Session(&'a PhySession),
    Writer(&'a PhyWriter),
}

impl EgoPhyRead<'_> {
    fn pins(self) -> Vec<EgoAnchor> {
        match self {
            Self::Session(session) => session.ego_anchors().cloned().collect(),
            Self::Writer(writer) => writer.ego_anchors().cloned().collect(),
        }
    }

    fn identities(self) -> (Option<EgoIdentitySelection>, Vec<EgoIdentity>) {
        match self {
            Self::Session(session) => (
                session.ego_identity_selection(),
                session.ego_identities().cloned().collect(),
            ),
            Self::Writer(writer) => (
                writer.ego_identity_selection(),
                writer.ego_identities().cloned().collect(),
            ),
        }
    }
}
