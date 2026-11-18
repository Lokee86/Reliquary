use reliquary_memory::{
    Gateway, HostCredentialBinding, Instance, OwnerGrant, OwnerId, Phylactery, ProductError, Rel,
    TrustedHostAdmission,
};
use std::collections::BTreeMap;
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
        Self::open_with_rel_mounts(rel, phy, access, &[]).map(|(host, _)| host)
    }

    pub fn open_with_rel_mounts(
        rel_path: &str,
        phy: Option<&str>,
        access: RelAccess,
        paths: &[&str],
    ) -> Result<(Self, Vec<OwnerId>), ProductError> {
        let gateway = Gateway::new();
        let writable = !matches!(access, RelAccess::View);
        let rel = gateway.mount_rel(&Rel::open(rel_path)?, writable)?;
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
        let mut mounted = Vec::with_capacity(paths.len());
        if !paths.is_empty() {
            let mut by_path = BTreeMap::from([(std::fs::canonicalize(rel_path)?, rel)]);
            for path in paths {
                let path = std::fs::canonicalize(path)?;
                let owner = if let Some(owner) = by_path.get(&path) {
                    *owner
                } else {
                    let owner = gateway.mount_rel(&Rel::open(&path)?, false)?;
                    by_path.insert(path, owner);
                    grants.push(OwnerGrant::ViewRel(owner));
                    owner
                };
                mounted.push(owner);
            }
        }
        let instance = authenticate(&gateway, rel, grants)?;
        Ok((Self { instance, rel, phy }, mounted))
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
