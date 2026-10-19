//! Internal transformed-copy shell. Owner codec migration and semantic parity
//! must be integrated before this can expose a public upgrade/promotion API.
mod model;
mod shell;
#[cfg(test)]
mod tests;

pub(crate) use model::{
    StorageUpgradeError, StorageUpgradePlan, StorageUpgradeRequest, StorageUpgradeResult,
};
pub(crate) use shell::{plan_storage_upgrade, stage_storage_upgrade};
