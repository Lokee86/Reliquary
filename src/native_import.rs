//! Authenticated native source import.

use crate::{Instance, OwnerGrant, OwnerId, ProductError};
use reliquary_application::native_import_source as source;
use std::path::Path;

pub use source::{
    HermesCopiedAuthor, HermesCopiedMessage, HermesCopiedRole, HermesCopiedSnapshot,
    HermesCopyScope, NativeImportCoverage, NativeImportFormat, NativeImportParentMapping,
    NativeImportSourceError, NativeImportStageResult, NativeImportState, NativeImportStatus,
    NativeImportWindow,
};

impl From<NativeImportSourceError> for ProductError {
    fn from(error: NativeImportSourceError) -> Self {
        match error {
            NativeImportSourceError::Io(error) => error.into(),
            error => Self::invalid(error.to_string()),
        }
    }
}

/// An immutable snapshot of a validated source and selection.
pub struct NativeImportPlan(source::NativeImportPlan);

impl NativeImportPlan {
    pub fn from_path(
        path: impl AsRef<Path>,
        format: NativeImportFormat,
        window: Option<NativeImportWindow>,
    ) -> Result<Self, ProductError> {
        Ok(Self(source::NativeImportPlan::from_path(
            path, format, window,
        )?))
    }

    /// Normalize the authorized, text-only copy for the native import workflow.
    pub fn from_hermes_copy(snapshot: HermesCopiedSnapshot) -> Result<Self, ProductError> {
        Ok(Self(source::NativeImportPlan::from_hermes_copy(snapshot)?))
    }

    /// Append a new-only copied suffix to an exact previously completed Hermes copy.
    /// The mounted REL checks the predecessor receipt and current branch under its writer lock.
    pub fn from_hermes_copy_extension(
        snapshot: HermesCopiedSnapshot,
        predecessor_sha256: [u8; 32],
    ) -> Result<Self, ProductError> {
        Ok(Self(source::NativeImportPlan::from_hermes_copy_extension(
            snapshot,
            predecessor_sha256,
        )?))
    }

    pub fn coverage(&self) -> &NativeImportCoverage {
        self.0.coverage()
    }

    pub fn parent_mappings(&self) -> &[NativeImportParentMapping] {
        self.0.parent_mappings()
    }

    pub fn source_sha256(&self) -> [u8; 32] {
        self.0.source_sha256()
    }

    pub fn import_id(&self, owner: OwnerId) -> String {
        self.0.import_id(owner.as_bytes())
    }
}

impl Instance {
    /// Stage or resume the exact source under the authenticated REL operator.
    pub fn stage_native_import(
        &self,
        owner: OwnerId,
        plan: NativeImportPlan,
    ) -> Result<NativeImportStageResult, ProductError> {
        self.require(OwnerGrant::OperateRel(owner))?;
        Ok(self.inner.stage_native_import(owner.into(), plan.0)?)
    }

    /// Mark a previously accepted Hermes copy terminal using its exact source key.
    pub fn tombstone_hermes_copy(
        &self,
        owner: OwnerId,
        scope: &HermesCopyScope,
        source_sha256: [u8; 32],
    ) -> Result<bool, ProductError> {
        self.require(OwnerGrant::OperateRel(owner))?;
        Ok(self
            .inner
            .tombstone_hermes_copy(owner.into(), scope, source_sha256)?)
    }

    /// Read durable staging state through the currently mounted REL projection.
    pub fn native_import_status(
        &self,
        owner: OwnerId,
        import_id: &str,
    ) -> Result<Option<NativeImportStatus>, ProductError> {
        self.require_rel_read(owner)?;
        if import_id.len() != 64
            || !import_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ProductError::invalid("invalid native import ID"));
        }
        Ok(self.inner.native_import_status(owner.into(), import_id)?)
    }
}
