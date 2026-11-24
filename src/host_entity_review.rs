use super::*;
use crate::{
    IdentityPair, IdentitySeparationInspection, IdentitySeparationRequest,
    PreparedIdentitySeparation,
};

/// Trusted host capability for deliberate operator actions. Keep this and
/// prepared actions on the server side of the operator interaction.
pub struct EntityReviewAuthority {
    installation: Arc<()>,
    operators: BTreeSet<String>,
    control: runtime::IdentitySeparationControl,
}

impl EntityReviewAuthority {
    pub(super) fn new(
        gateway: &Gateway,
        adapter: &HostAdmissionAdapter,
        operators: &[&str],
    ) -> Self {
        Self {
            installation: adapter.installation.clone(),
            operators: operators.iter().map(|id| (*id).to_owned()).collect(),
            control: gateway.inner.identity_separation_control(),
        }
    }

    /// Bind an authenticated session from this installation to the trusted interaction.
    pub fn bind<'a>(
        &'a self,
        instance: &'a Instance,
    ) -> Result<EntityReviewSession<'a>, ProductError> {
        instance.inner.ensure_credential_active()?;
        if !Arc::ptr_eq(&self.installation, &instance.installation)
            || !self.operators.contains(instance.principal())
        {
            return Err(ProductError::new(
                ProductErrorKind::Unauthorized,
                "operator review is not authorized",
            ));
        }
        Ok(EntityReviewSession {
            instance,
            authority: self,
        })
    }
}

pub struct EntityReviewSession<'a> {
    instance: &'a Instance,
    authority: &'a EntityReviewAuthority,
}

impl EntityReviewSession<'_> {
    fn authorize(&self, pair: &IdentityPair) -> Result<(), ProductError> {
        self.instance
            .require(OwnerGrant::ManageRel(OwnerId::from_bytes(pair.local_owner)))?;
        self.instance
            .require_rel_read(OwnerId::from_bytes(pair.peer_owner))
    }

    /// Prepare the preview to show the operator, including a conflicting evaluator judgment.
    pub fn prepare(
        &self,
        pair: IdentityPair,
        action: IdentitySeparationRequest,
    ) -> Result<PreparedIdentitySeparation, ProductError> {
        self.authorize(&pair)?;
        Ok(self.authority.control.prepare_separation(
            &self.instance.gateway.identity,
            &self.instance.inner,
            pair,
            action,
            *uuid::Uuid::new_v4().as_bytes(),
        )?)
    }

    /// Call only after the operator approves the displayed action in this session.
    pub fn commit(
        &self,
        prepared: &PreparedIdentitySeparation,
    ) -> Result<IdentitySeparationInspection, ProductError> {
        self.authorize(&prepared.preview().pair)?;
        Ok(self.authority.control.commit_separation(
            &self.instance.gateway.identity,
            &self.instance.inner,
            prepared,
        )?)
    }

    /// Recover this operator's durable acknowledgement, including after reconnecting.
    pub fn recover(
        &self,
        pair: IdentityPair,
        action_id: [u8; 16],
    ) -> Result<Option<IdentitySeparationInspection>, ProductError> {
        self.authorize(&pair)?;
        Ok(self
            .authority
            .control
            .lookup_separation(&self.instance.inner, pair, action_id)?)
    }
}
