use reliquary_memory::{
    Gateway, HostCredentialBinding, Instance, OwnerGrant, OwnerId, Phylactery, ProductError, Rel,
    TrustedHostAdmission,
};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct LocalHost {
    pub instance: Instance,
    pub rel: OwnerId,
    pub phy: Option<OwnerId>,
}

#[derive(Clone, Copy)]
pub(crate) enum RelAccess {
    View,
    Operate,
    Manage,
}

impl LocalHost {
    pub fn open_phy(path: &str) -> Result<(Instance, OwnerId), ProductError> {
        let gateway = Gateway::new();
        let owner = gateway.mount_phylactery(&Phylactery::open(path)?, false)?;
        let instance = authenticate(&gateway, owner, [OwnerGrant::ViewPhylactery(owner)])?;
        Ok((instance, owner))
    }

    pub fn open(rel: &str, phy: Option<&str>, access: RelAccess) -> Result<Self, ProductError> {
        let gateway = Gateway::new();
        let writable = !matches!(access, RelAccess::View);
        let rel = gateway.mount_rel(&Rel::open(rel)?, writable)?;
        let phy = phy
            .map(|path| gateway.mount_phylactery(&Phylactery::open(path)?, writable))
            .transpose()?;
        let mut grants = vec![match access {
            RelAccess::View => OwnerGrant::ViewRel(rel),
            RelAccess::Operate => OwnerGrant::OperateRel(rel),
            RelAccess::Manage => OwnerGrant::ManageRel(rel),
        }];
        if let Some(phy) = phy {
            grants.push(if writable {
                OwnerGrant::OperatePhylactery(phy)
            } else {
                OwnerGrant::ViewPhylactery(phy)
            });
        }
        let instance = authenticate(&gateway, rel, grants)?;
        Ok(Self { instance, rel, phy })
    }
}

fn authenticate(
    gateway: &Gateway,
    owner: OwnerId,
    grants: impl IntoIterator<Item = OwnerGrant>,
) -> Result<Instance, ProductError> {
    // The admission credential is scoped to the in-process Gateway.
    let token = format!("reliquary-cli-local-{owner}-{}", std::process::id());
    let (adapter, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "reliquary-cli-local",
        &token,
        grants,
    )])
    .install(gateway)?;
    adapter.authenticate(&token)
}

pub(crate) fn now_ns() -> Result<i64, ProductError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_nanos()).ok())
        .ok_or_else(|| super::invalid("system time is outside the supported range"))
}
