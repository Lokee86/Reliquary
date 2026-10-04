//! Credentials cross the request boundary; identity/grants come only from an installed host policy.
use super::*;
pub struct VerifiedInstanceAdmission {
    pub principal_id: String,
    pub phy_id: Option<String>,
    pub grants: BTreeMap<String, BTreeSet<OwnerGrant>>,
}
pub trait GatewayAdmissionAdapter: Send + Sync {
    /// Authenticate opaque credentials and resolve the principal's PHY and exact owner grants.
    /// Reject expired/forged credentials; do not log credentials.
    fn verify(&self, credential: &str) -> Result<VerifiedInstanceAdmission, String>;
}
impl GatewayRuntime {
    /// Configure the trusted application policy before exposing the gateway to request handlers.
    pub fn with_admission_adapter(mut self, adapter: Arc<dyn GatewayAdmissionAdapter>) -> Self {
        self.admission_adapter = Some(adapter);
        self
    }
    pub fn attach_authenticated_instance(
        &self,
        credential: &str,
    ) -> Result<Arc<InstanceRuntime>, ReliquaryRuntimeHostError> {
        let adapter = self
            .admission_adapter
            .as_ref()
            .ok_or_else(|| operation("no trusted admission adapter is configured"))?;
        let verified = adapter
            .verify(credential)
            .map_err(|_| operation("instance authentication failed"))?;
        self.attach_instance(verified.principal_id, verified.phy_id, verified.grants)
    }
    pub fn refresh_authenticated_instance(
        &self,
        instance: &InstanceRuntime,
        credential: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.attachment_entry(instance)?;
        let adapter = self
            .admission_adapter
            .as_ref()
            .ok_or_else(|| operation("no trusted admission adapter is configured"))?;
        let verified = adapter
            .verify(credential)
            .map_err(|_| operation("instance authentication failed"))?;
        if verified.principal_id != instance.principal_id()
            || verified.phy_id.as_deref() != instance.attached_phy_owner_id()
        {
            return Err(operation("credential does not belong to this attachment"));
        }
        self.replace_instance_grants(instance, verified.grants)
    }
}
