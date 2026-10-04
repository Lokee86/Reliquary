use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use uuid::Uuid;

/// Stable identity for one short-lived runtime attachment.
pub type InstanceId = Uuid;

/// An independently checked capability on one exact durable owner.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OwnerGrant {
    ViewRel,
    SubmitConversation,
    CancelOwnPendingSubmission,
    CancelSharedGeneration,
    ManageProject,
    ManageRel,
    UseOwnPhy,
}

/// Immutable attachment coordinates captured for an operation.
///
/// A gateway compares this value again at dispatch, commit, and delivery
/// boundaries; it is not a durable owner ID or an authorization grant by itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstanceAuthorization {
    pub instance_id: InstanceId,
    pub attachment_incarnation: Uuid,
    pub grant_generation: u64,
}

/// Per-instance managed conversation selection for one REL.
///
/// This is selection/reopen state only. It does not own an InteractionRuntime
/// session or represent a shared writable continuation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ManagedConversationState {
    pub conversation_id: Option<String>,
    pub branch_id: Option<String>,
    pub reopened_session_pending: bool,
}

#[derive(Debug)]
struct InstanceState {
    attached: bool,
    attachment_incarnation: Uuid,
    grant_generation: u64,
    owner_grant_generations: BTreeMap<String, u64>,
    active_rel_id: Option<String>,
    grants: BTreeMap<String, BTreeSet<OwnerGrant>>,
    managed_by_rel: BTreeMap<String, ManagedConversationState>,
}

/// Canonical mutable context for one attached instance.
///
/// Shared REL/PHY executions remain owned by the gateway. This handle stores
/// only instance-local selection, grants, and observed managed-conversation
/// state; detaching it never closes or unmounts an owner.
pub struct InstanceRuntime {
    instance_id: InstanceId,
    principal_id: String,
    attached_phy_owner_id: Option<String>,
    state: Mutex<InstanceState>,
}

impl InstanceRuntime {
    /// Create an attached instance context using gateway-verified identity and grants.
    ///
    /// Callers must resolve principal_id, the optional PHY owner, and grants
    /// from trusted host authentication before constructing this value.
    pub(super) fn new(
        principal_id: String,
        attached_phy_owner_id: Option<String>,
        grants: BTreeMap<String, BTreeSet<OwnerGrant>>,
    ) -> Self {
        Self {
            instance_id: Uuid::new_v4(),
            principal_id,
            attached_phy_owner_id,
            state: Mutex::new(InstanceState {
                attached: true,
                attachment_incarnation: Uuid::new_v4(),
                grant_generation: 1,
                owner_grant_generations: grants.keys().map(|owner| (owner.clone(), 1)).collect(),
                active_rel_id: None,
                grants,
                managed_by_rel: BTreeMap::new(),
            }),
        }
    }

    pub fn instance_id(&self) -> InstanceId {
        self.instance_id
    }

    pub fn principal_id(&self) -> &str {
        &self.principal_id
    }

    /// The single PHY resolved from the verified principal binding, if attached.
    pub fn attached_phy_owner_id(&self) -> Option<&str> {
        self.attached_phy_owner_id.as_deref()
    }

    pub(super) fn authorization(&self) -> Result<InstanceAuthorization, InstanceRuntimeError> {
        let state = self
            .state
            .lock()
            .map_err(|_| InstanceRuntimeError::LockPoisoned)?;
        Self::ensure_attached(&state)?;
        Ok(InstanceAuthorization {
            instance_id: self.instance_id,
            attachment_incarnation: state.attachment_incarnation,
            grant_generation: state.grant_generation,
        })
    }

    /// Check a captured attachment and an exact owner-scoped capability.
    pub(super) fn authorize(
        &self,
        owner_id: &str,
        grant: OwnerGrant,
        authorization: InstanceAuthorization,
    ) -> Result<(), InstanceRuntimeError> {
        let state = self
            .state
            .lock()
            .map_err(|_| InstanceRuntimeError::LockPoisoned)?;
        Self::validate_authorization(self.instance_id, &state, authorization)?;
        if !state
            .grants
            .get(owner_id)
            .is_some_and(|owner_grants| owner_grants.contains(&grant))
        {
            return Err(InstanceRuntimeError::GrantDenied {
                owner_id: owner_id.to_owned(),
                grant,
            });
        }
        if grant == OwnerGrant::UseOwnPhy && self.attached_phy_owner_id.as_deref() != Some(owner_id)
        {
            return Err(InstanceRuntimeError::GrantDenied {
                owner_id: owner_id.to_owned(),
                grant,
            });
        }
        Ok(())
    }

    /// Current generation for one exact owner's grant set. Unlike the attachment
    /// generation, this stays stable when a different owner's grants change.
    pub(super) fn owner_grant_generation(
        &self,
        owner_id: &str,
    ) -> Result<u64, InstanceRuntimeError> {
        let state = self
            .state
            .lock()
            .map_err(|_| InstanceRuntimeError::LockPoisoned)?;
        Self::ensure_attached(&state)?;
        Ok(state
            .owner_grant_generations
            .get(owner_id)
            .copied()
            .unwrap_or(1))
    }

    /// Validate a gateway token against its owner-specific grant generation.
    /// The attachment incarnation is checked separately from grant generations.
    pub(super) fn authorize_owner_generation(
        &self,
        owner_id: &str,
        grant: OwnerGrant,
        authorization: InstanceAuthorization,
        expected_owner_generation: u64,
    ) -> Result<(), InstanceRuntimeError> {
        let state = self
            .state
            .lock()
            .map_err(|_| InstanceRuntimeError::LockPoisoned)?;
        Self::validate_attachment(self.instance_id, &state, authorization)?;
        if state
            .owner_grant_generations
            .get(owner_id)
            .copied()
            .unwrap_or(1)
            != expected_owner_generation
        {
            return Err(InstanceRuntimeError::StaleAuthorization);
        }
        Self::validate_grant(
            &state,
            owner_id,
            grant,
            self.attached_phy_owner_id.as_deref(),
        )
    }

    /// Replace the gateway-verified owner grants and invalidate previously captured tokens.
    pub(super) fn grants_snapshot(
        &self,
    ) -> Result<BTreeMap<String, BTreeSet<OwnerGrant>>, InstanceRuntimeError> {
        let state = self
            .state
            .lock()
            .map_err(|_| InstanceRuntimeError::LockPoisoned)?;
        Self::ensure_attached(&state)?;
        Ok(state.grants.clone())
    }

    pub(super) fn replace_grants(
        &self,
        grants: BTreeMap<String, BTreeSet<OwnerGrant>>,
    ) -> Result<u64, InstanceRuntimeError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| InstanceRuntimeError::LockPoisoned)?;
        Self::ensure_attached(&state)?;
        let next_generation = state
            .grant_generation
            .checked_add(1)
            .ok_or(InstanceRuntimeError::GenerationExhausted)?;
        let revoked_owners = state
            .grants
            .iter()
            .filter_map(|(owner_id, previous)| {
                let replacement = grants.get(owner_id);
                previous
                    .iter()
                    .any(|grant| !replacement.is_some_and(|next| next.contains(grant)))
                    .then_some(owner_id.clone())
            })
            .collect::<BTreeSet<_>>();
        let mut next_owner_generations = state.owner_grant_generations.clone();
        for owner_id in revoked_owners {
            let generation = next_owner_generations.get(&owner_id).copied().unwrap_or(1);
            next_owner_generations.insert(
                owner_id,
                generation
                    .checked_add(1)
                    .ok_or(InstanceRuntimeError::GenerationExhausted)?,
            );
        }
        let authorized_rels = grants
            .iter()
            .filter(|(_, owner_grants)| owner_grants.contains(&OwnerGrant::ViewRel))
            .map(|(owner_id, _)| owner_id.clone())
            .collect::<BTreeSet<_>>();
        state.grants = grants;
        state.owner_grant_generations = next_owner_generations;
        state.grant_generation = next_generation;
        if state
            .active_rel_id
            .as_ref()
            .is_some_and(|owner_id| !authorized_rels.contains(owner_id))
        {
            state.active_rel_id = None;
        }
        state
            .managed_by_rel
            .retain(|owner_id, _| authorized_rels.contains(owner_id));
        Ok(next_generation)
    }

    /// Select an authorized REL for ambient operations on this instance.
    ///
    /// The gateway must additionally verify that the REL is mounted and live.
    pub(super) fn select_rel(&self, owner_id: &str) -> Result<bool, InstanceRuntimeError> {
        let authorization = self.authorization()?;
        self.authorize(owner_id, OwnerGrant::ViewRel, authorization)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| InstanceRuntimeError::LockPoisoned)?;
        Self::validate_authorization(self.instance_id, &state, authorization)?;
        if state.active_rel_id.as_deref() == Some(owner_id) {
            return Ok(false);
        }
        state.active_rel_id = Some(owner_id.to_owned());
        Ok(true)
    }

    pub(super) fn selected_rel(&self) -> Result<Option<String>, InstanceRuntimeError> {
        let state = self
            .state
            .lock()
            .map_err(|_| InstanceRuntimeError::LockPoisoned)?;
        Self::ensure_attached(&state)?;
        Ok(state.active_rel_id.clone())
    }

    pub(super) fn managed_conversation_state(
        &self,
        owner_id: &str,
    ) -> Result<ManagedConversationState, InstanceRuntimeError> {
        let authorization = self.authorization()?;
        self.authorize(owner_id, OwnerGrant::ViewRel, authorization)?;
        let state = self
            .state
            .lock()
            .map_err(|_| InstanceRuntimeError::LockPoisoned)?;
        Self::validate_authorization(self.instance_id, &state, authorization)?;
        Ok(state
            .managed_by_rel
            .get(owner_id)
            .cloned()
            .unwrap_or_default())
    }

    /// Replace this instance's selected conversation state for an authorized REL.
    ///
    /// Shared writer/session admission belongs to the gateway coordinator.
    pub(super) fn set_managed_conversation_state(
        &self,
        owner_id: &str,
        managed: ManagedConversationState,
    ) -> Result<(), InstanceRuntimeError> {
        let authorization = self.authorization()?;
        self.authorize(owner_id, OwnerGrant::ViewRel, authorization)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| InstanceRuntimeError::LockPoisoned)?;
        Self::validate_authorization(self.instance_id, &state, authorization)?;
        if managed.conversation_id.is_none() {
            state.managed_by_rel.remove(owner_id);
        } else {
            state.managed_by_rel.insert(owner_id.to_owned(), managed);
        }
        Ok(())
    }

    /// Revoke the current attachment and invalidate all captured authorization values.
    ///
    /// This only detaches this instance context; the gateway retains shared owners.
    pub(super) fn detach(&self) -> Result<bool, InstanceRuntimeError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| InstanceRuntimeError::LockPoisoned)?;
        if !state.attached {
            return Ok(false);
        }
        let next_generation = state
            .grant_generation
            .checked_add(1)
            .ok_or(InstanceRuntimeError::GenerationExhausted)?;
        state.attached = false;
        state.grant_generation = next_generation;
        state.attachment_incarnation = Uuid::new_v4();
        state.active_rel_id = None;
        state.grants.clear();
        state.owner_grant_generations.clear();
        state.managed_by_rel.clear();
        Ok(true)
    }

    pub(super) fn is_attached(&self) -> Result<bool, InstanceRuntimeError> {
        let state = self
            .state
            .lock()
            .map_err(|_| InstanceRuntimeError::LockPoisoned)?;
        Ok(state.attached)
    }

    fn ensure_attached(state: &InstanceState) -> Result<(), InstanceRuntimeError> {
        if state.attached {
            Ok(())
        } else {
            Err(InstanceRuntimeError::Detached)
        }
    }

    fn validate_authorization(
        instance_id: InstanceId,
        state: &InstanceState,
        authorization: InstanceAuthorization,
    ) -> Result<(), InstanceRuntimeError> {
        Self::validate_attachment(instance_id, state, authorization)?;
        if authorization.grant_generation != state.grant_generation {
            return Err(InstanceRuntimeError::StaleAuthorization);
        }
        Ok(())
    }

    fn validate_attachment(
        instance_id: InstanceId,
        state: &InstanceState,
        authorization: InstanceAuthorization,
    ) -> Result<(), InstanceRuntimeError> {
        Self::ensure_attached(state)?;
        if authorization.instance_id != instance_id
            || authorization.attachment_incarnation != state.attachment_incarnation
        {
            return Err(InstanceRuntimeError::StaleAuthorization);
        }
        Ok(())
    }

    fn validate_grant(
        state: &InstanceState,
        owner_id: &str,
        grant: OwnerGrant,
        attached_phy_owner_id: Option<&str>,
    ) -> Result<(), InstanceRuntimeError> {
        if !state
            .grants
            .get(owner_id)
            .is_some_and(|owner_grants| owner_grants.contains(&grant))
        {
            return Err(InstanceRuntimeError::GrantDenied {
                owner_id: owner_id.to_owned(),
                grant,
            });
        }
        if grant == OwnerGrant::UseOwnPhy && attached_phy_owner_id != Some(owner_id) {
            return Err(InstanceRuntimeError::GrantDenied {
                owner_id: owner_id.to_owned(),
                grant,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstanceRuntimeError {
    LockPoisoned,
    Detached,
    StaleAuthorization,
    GrantDenied { owner_id: String, grant: OwnerGrant },
    GenerationExhausted,
}

impl std::fmt::Display for InstanceRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LockPoisoned => f.write_str("instance runtime lock is poisoned"),
            Self::Detached => f.write_str("instance runtime is detached"),
            Self::StaleAuthorization => f.write_str("instance authorization is stale"),
            Self::GrantDenied { owner_id, grant } => {
                write!(f, "instance lacks {grant:?} access to owner {owner_id}")
            }
            Self::GenerationExhausted => f.write_str("instance grant generation is exhausted"),
        }
    }
}

impl std::error::Error for InstanceRuntimeError {}

#[cfg(test)]
#[path = "runtime_host_instance_tests.rs"]
mod tests;
