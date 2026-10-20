//! Internal transformed-copy shell. Owner codec migration and semantic parity
//! must be integrated before this can expose a public upgrade/promotion API.
mod ego;
#[cfg(all(test, feature = "history-oracle-fixtures"))]
mod ego_authoritative_tests;
#[cfg(test)]
mod ego_tests;
mod entity;
mod graph;
#[cfg(all(test, feature = "history-oracle-fixtures"))]
mod graph_authoritative_tests;
#[cfg(test)]
mod graph_tests;
mod memory;
mod model;
mod relationship;
mod shell;
#[cfg(test)]
mod tests;

pub(crate) use model::{
    StorageUpgradeError, StorageUpgradePlan, StorageUpgradeRequest, StorageUpgradeResult,
};
pub(crate) use shell::{
    plan_storage_upgrade, stage_ego_storage_upgrade, stage_entity_storage_upgrade,
    stage_graph_storage_upgrade, stage_memory_storage_upgrade, stage_relationship_storage_upgrade,
    stage_storage_upgrade,
};

#[cfg(all(test, feature = "history-oracle-fixtures"))]
mod entity_authoritative_tests;
#[cfg(test)]
mod entity_tests;
#[cfg(test)]
mod memory_tests;
#[cfg(test)]
mod prepruned_memory_tests;

#[cfg(all(test, feature = "history-oracle-fixtures"))]
mod memory_authoritative_tests;

#[cfg(test)]
mod relationship_tests;

#[cfg(all(test, feature = "history-oracle-fixtures"))]
mod relationship_authoritative_tests;
