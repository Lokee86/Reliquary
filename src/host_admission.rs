use super::*;
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
                grants: product_grants,
                gateway: gateway.clone(),
            },
            HostAdmissionControl { inner: control },
        ))
    }
}

pub struct HostAdmissionControl {
    inner: runtime::HostAdmissionControl,
}
impl HostAdmissionControl {
    pub fn revoke(&self, token: &str) -> Result<(), ProductError> {
        self.inner.revoke(token).map_err(Into::into)
    }
}

pub struct HostAdmissionAdapter {
    inner: runtime::HostAdmissionAdapter,
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
        Ok(Instance {
            inner: Arc::new(inner),
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
