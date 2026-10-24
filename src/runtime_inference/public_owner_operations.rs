//! Native runtime operations for the supported public REL/PHY owner bindings.
use crate::{CvaError, PhylacteryError, RelMetadata, ReliquaryScopeKind};

impl crate::RelExecutionOwner {
    pub(crate) fn runtime_legacy_scope_kind(&self) -> Option<ReliquaryScopeKind> {
        self.owner.legacy_scope_kind()
    }

    pub(crate) fn runtime_owner_uuid(&self) -> Option<[u8; 16]> {
        self.owner.owner_uuid()
    }

    pub(crate) fn runtime_latest_global_version(&self) -> u64 {
        self.owner.latest_global_version()
    }

    pub(crate) fn runtime_transaction_time_ns(&self, version: u64) -> Option<i64> {
        self.owner.transaction_time_ns(version)
    }

    pub(crate) fn runtime_version_at_or_before(&self, transaction_time_ns: i64) -> Option<u64> {
        self.owner.version_at_or_before(transaction_time_ns)
    }

    pub(crate) fn native_put_rel_metadata(
        &mut self,
        type_label: Option<String>,
        dependencies: Vec<String>,
    ) -> Result<bool, CvaError> {
        let type_label = type_label.and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        });
        let mut dependencies: Vec<_> = dependencies
            .into_iter()
            .map(|value| value.trim().to_owned())
            .collect();
        dependencies.sort();
        dependencies.dedup();
        if let Some(owner_id) = self.runtime_owner_id()
            && dependencies
                .iter()
                .any(|dependency| dependency == &owner_id)
        {
            return Err(CvaError::RelMetadata(
                "a REL cannot depend on itself".into(),
            ));
        }
        self.owner
            .put_rel_metadata(RelMetadata {
                type_label,
                dependencies,
            })
            .map_err(CvaError::RelMetadata)
    }

    pub(crate) fn runtime_sync_public_owner(&self) -> Result<(), CvaError> {
        self.sync_owner()
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn runtime_owner_id(&self) -> Option<String> {
        self.owner.owner_id()
    }

    pub(crate) fn runtime_owner_uuid(&self) -> Option<[u8; 16]> {
        self.owner.owner_uuid()
    }

    pub(crate) fn runtime_latest_global_version(&self) -> u64 {
        self.owner.latest_global_version()
    }

    pub(crate) fn runtime_transaction_time_ns(&self, version: u64) -> Option<i64> {
        self.owner.transaction_time_ns(version)
    }

    pub(crate) fn runtime_version_at_or_before(&self, transaction_time_ns: i64) -> Option<u64> {
        self.owner.version_at_or_before(transaction_time_ns)
    }

    pub(crate) fn runtime_profile(&self) -> crate::PhylacteryProfile {
        self.owner.profile.current()
    }

    pub(crate) fn native_set_profile(
        &mut self,
        display_name: Option<String>,
    ) -> Result<bool, PhylacteryError> {
        self.owner
            .set_profile(display_name)
            .map_err(PhylacteryError::Profile)
    }
}
