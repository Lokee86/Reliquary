use super::*;
use reliquary_owner_session::model::identity::MAX_IDENTITY_SEPARATION_OPERATOR_ID_BYTES;
use std::collections::HashMap;

#[derive(Clone)]
pub struct HostCredentialBinding {
    principal: String,
    token: String,
    grants: BTreeSet<OwnerGrant>,
}
impl HostCredentialBinding {
    pub fn new(
        principal: impl Into<String>,
        token: impl Into<String>,
        grants: impl IntoIterator<Item = OwnerGrant>,
    ) -> Self {
        Self {
            principal: principal.into(),
            token: token.into(),
            grants: grants.into_iter().collect(),
        }
    }
}

pub struct TrustedHostAdmission {
    bindings: Vec<HostCredentialBinding>,
}
impl TrustedHostAdmission {
    pub fn new(bindings: impl IntoIterator<Item = HostCredentialBinding>) -> Self {
        Self {
            bindings: bindings.into_iter().collect(),
        }
    }

    /// Install admission and give the trusted user-interaction handler a separate
    /// approval capability. Operator IDs are stable, nonsecret principal names
    /// from this host's credential configuration.
    pub fn install_with_entity_review(
        &self,
        gateway: &Gateway,
        operator_principals: &[&str],
    ) -> Result<
        (
            HostAdmissionAdapter,
            HostAdmissionControl,
            EntityReviewAuthority,
        ),
        ProductError,
    > {
        for operator in operator_principals {
            let configured = self
                .bindings
                .iter()
                .any(|binding| binding.principal == *operator);
            let is_credential = self
                .bindings
                .iter()
                .any(|binding| binding.token == *operator);
            if !configured
                || is_credential
                || operator.trim() != *operator
                || operator.len() > MAX_IDENTITY_SEPARATION_OPERATOR_ID_BYTES
            {
                return Err(ProductError::invalid(
                    "expected a configured nonsecret operator ID",
                ));
            }
        }
        let (adapter, control) = self.install(gateway)?;
        let review = EntityReviewAuthority::new(gateway, &adapter, operator_principals);
        Ok((adapter, control, review))
    }

    pub fn install(
        &self,
        gateway: &Gateway,
    ) -> Result<(HostAdmissionAdapter, HostAdmissionControl), ProductError> {
        let mut product_grants = HashMap::new();
        let mut runtime_bindings = Vec::new();
        for binding in &self.bindings {
            let mut rel = BTreeSet::new();
            let mut phy = BTreeSet::new();
            for grant in &binding.grants {
                if grant.is_rel() {
                    rel.insert(runtime::OwnerId::from(grant.owner()));
                } else {
                    phy.insert(runtime::OwnerId::from(grant.owner()));
                }
            }
            let hash = *blake3::hash(binding.token.as_bytes()).as_bytes();
            if product_grants
                .insert(hash, binding.grants.clone())
                .is_some()
            {
                return Err(ProductError::invalid(
                    "credential is configured more than once",
                ));
            }
            runtime_bindings.push(runtime::HostCredentialBinding::new(
                binding.principal.clone(),
                binding.token.clone(),
                rel,
                phy,
            ));
        }
        let (adapter, control) =
            runtime::TrustedHostAdmission::new(runtime_bindings).install(&gateway.inner)?;
        Ok((
            HostAdmissionAdapter {
                inner: adapter,
                installation: Arc::new(()),
                grants: product_grants,
                gateway: gateway.clone(),
            },
            HostAdmissionControl {
                inner: control,
                gateway: gateway.clone(),
            },
        ))
    }
}

pub struct HostAdmissionControl {
    inner: runtime::HostAdmissionControl,
    gateway: Gateway,
}
impl HostAdmissionControl {
    pub fn revoke(&self, token: &str) -> Result<(), ProductError> {
        self.inner.revoke(token).map_err(Into::into)
    }

    /// Release one read-only PHY mount after draining operations that use it.
    /// The host controls admission and revocation separately.
    pub fn detach_read_only_phylactery(&self, owner: OwnerId) -> Result<(), ProductError> {
        self.gateway.detach_read_only_phylactery(owner)
    }
}

pub struct HostAdmissionAdapter {
    inner: runtime::HostAdmissionAdapter,
    pub(super) installation: Arc<()>,
    grants: HashMap<[u8; 32], BTreeSet<OwnerGrant>>,
    gateway: Gateway,
}
impl HostAdmissionAdapter {
    pub fn authenticate(&self, token: &str) -> Result<Instance, ProductError> {
        let inner = self.inner.authenticate(token)?;
        let grants = self
            .grants
            .get(blake3::hash(token.as_bytes()).as_bytes())
            .cloned()
            .ok_or_else(|| {
                ProductError::new(
                    ProductErrorKind::Unauthorized,
                    "credential has no product grants",
                )
            })?;
        let inner = Arc::new(inner);
        let recording = grants
            .iter()
            .filter_map(|grant| match grant {
                OwnerGrant::ManageRel(owner) => Some((*owner).into()),
                _ => None,
            })
            .collect();
        self.gateway.identity.admit(&inner, recording)?;
        Ok(Instance {
            inner,
            installation: self.installation.clone(),
            grants: Arc::new(grants),
            gateway: self.gateway.clone(),
        })
    }
    pub fn connect_runtime_host(
        &self,
        adapter: Arc<dyn runtime::TrustedHostAdapter>,
    ) -> runtime::CapabilityRouter {
        self.inner.connect_runtime_host(adapter)
    }
}
