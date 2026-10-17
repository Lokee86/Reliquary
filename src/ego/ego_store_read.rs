use super::EgoStore;
use crate::{EgoAnchor, EgoAnchorId, EgoIdentity, EgoIdentityId, EgoPersonality, EgoWebSynthesis};

impl EgoStore {
    pub(crate) fn version(&self) -> u64 {
        self.version
    }

    pub(crate) fn active_identity(&self) -> Option<EgoIdentity> {
        self.active_identity_id.and_then(|id| self.identity(id))
    }

    pub(crate) fn active_identity_id(&self) -> Option<EgoIdentityId> {
        self.active_identity_id
    }

    pub(crate) fn identities(&self) -> Vec<EgoIdentity> {
        self.identities
            .iter()
            .filter(|(_, state)| !state.deleted)
            .map(|(id, state)| EgoIdentity {
                id: *id,
                revision: state.revision,
                name: state.name.clone(),
                text: state.text.clone(),
            })
            .collect()
    }

    pub(crate) fn identity(&self, id: EgoIdentityId) -> Option<EgoIdentity> {
        let state = self.identities.get(&id)?;
        (!state.deleted).then(|| EgoIdentity {
            id,
            revision: state.revision,
            name: state.name.clone(),
            text: state.text.clone(),
        })
    }

    pub(crate) fn personality(&self) -> Option<&EgoPersonality> {
        self.personality.as_ref()
    }
    pub(crate) fn synthesis(&self) -> Option<&EgoWebSynthesis> {
        self.synthesis.as_ref()
    }

    pub(crate) fn anchors(&self) -> Vec<EgoAnchor> {
        self.anchors
            .iter()
            .filter(|(_, state)| !state.deleted)
            .map(|(id, state)| EgoAnchor {
                id: *id,
                revision: state.revision,
                priority: state.priority,
                text: state.text.clone(),
            })
            .collect()
    }

    pub(crate) fn anchor(&self, id: EgoAnchorId) -> Option<EgoAnchor> {
        let state = self.anchors.get(&id)?;
        (!state.deleted).then(|| EgoAnchor {
            id,
            revision: state.revision,
            priority: state.priority,
            text: state.text.clone(),
        })
    }
}

impl EgoStore {
    pub(crate) fn identity_lifecycle(
        &self,
        id: EgoIdentityId,
    ) -> Option<crate::owner_lifecycle::LifecycleState> {
        let state = self.identities.get(&id)?;
        Some(lifecycle(
            state.deleted,
            self.identity_deletions.get(&id).copied(),
        ))
    }
    pub(crate) fn anchor_lifecycle(
        &self,
        id: EgoAnchorId,
    ) -> Option<crate::owner_lifecycle::LifecycleState> {
        let state = self.anchors.get(&id)?;
        Some(lifecycle(
            state.deleted,
            self.anchor_deletions.get(&id).copied(),
        ))
    }
    pub(crate) fn historical_identity(
        &self,
        id: EgoIdentityId,
        revision: u64,
    ) -> Option<EgoIdentity> {
        self.identity_history.get(&(id, revision)).cloned()
    }
    pub(crate) fn historical_anchor(&self, id: EgoAnchorId, revision: u64) -> Option<EgoAnchor> {
        self.anchor_history.get(&(id, revision)).cloned()
    }
}
fn lifecycle(deleted: bool, version: Option<u64>) -> crate::owner_lifecycle::LifecycleState {
    if deleted {
        crate::owner_lifecycle::LifecycleState::Tombstoned(
            crate::owner_lifecycle::TombstoneCoordinates {
                global_version: None,
                domain_version: version,
            },
        )
    } else {
        crate::owner_lifecycle::LifecycleState::Active
    }
}
