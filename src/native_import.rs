//! Authenticated native source import.

use crate::{EmbeddingEndpoint, Instance, OwnerGrant, OwnerId, ProductError};
use reliquary_application::native_import_source as source;
use reliquary_vectors::SwitchboardEmbeddingEndpoint;
use std::{path::Path, sync::Arc};

pub use reliquary_runtime_gateway::ArchiveVectorPublication as NativeImportArchiveVectorPublication;
pub use source::{
    HermesCopiedAuthor, HermesCopiedEcho, HermesCopiedEchoKind, HermesCopiedMessage,
    HermesCopiedRole, HermesCopiedSnapshot, HermesCopiedSource, HermesCopiedSourceTurn,
    HermesCopiedToolOutcome, HermesCopyScope, HermesCopySourceChange,
    HermesCopySourceChangeReceipt, NativeImportCoverage, NativeImportFormat,
    NativeImportParentMapping, NativeImportSourceError, NativeImportStageResult, NativeImportState,
    NativeImportStatus, NativeImportWindow,
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

    /// Normalize an authorized copy together with redacted, source-keyed Echo.
    pub fn from_hermes_copy_with_echo(
        snapshot: HermesCopiedSnapshot,
        echo: Vec<HermesCopiedEcho>,
    ) -> Result<Self, ProductError> {
        Ok(Self(source::NativeImportPlan::from_hermes_copy_with_echo(
            snapshot, echo,
        )?))
    }

    /// Append a copied suffix and its Echo to an exact completed predecessor.
    pub fn from_hermes_copy_extension_with_echo(
        snapshot: HermesCopiedSnapshot,
        predecessor_sha256: [u8; 32],
        echo: Vec<HermesCopiedEcho>,
    ) -> Result<Self, ProductError> {
        Ok(Self(
            source::NativeImportPlan::from_hermes_copy_extension_with_echo(
                snapshot,
                predecessor_sha256,
                echo,
            )?,
        ))
    }

    /// Bind a trusted host's copied turn to its registered PHY at source admission.
    /// The optional UID names the exact user message verified by that host.
    /// Staging also requires operating grants for both owners.
    pub fn bind_hermes_live_source(
        self,
        phy: OwnerId,
        verified_message_uid: Option<String>,
    ) -> Result<Self, ProductError> {
        Ok(Self(self.0.bind_hermes_live_source(
            phy.as_bytes(),
            verified_message_uid,
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

    /// Resolve copied Hermes source provenance across an exact Archive hit range.
    pub fn read_hermes_archive_range(
        &self,
        owner: OwnerId,
        conversation_id: &str,
        start_node_id: &str,
        end_node_id: &str,
        source_archive_version: u64,
    ) -> Result<Option<Vec<HermesCopiedSourceTurn>>, ProductError> {
        self.require_rel_read(owner)?;
        Ok(self.inner.read_hermes_archive_range(
            owner.into(),
            conversation_id,
            start_node_id,
            end_node_id,
            source_archive_version,
        )?)
    }

    /// Read an exact inclusive message range from this principal's copied receipt.
    /// A current head also admits exact ranges in its surviving ancestry.
    pub fn read_hermes_copy_source(
        &self,
        owner: OwnerId,
        scope: &HermesCopyScope,
        source_sha256: [u8; 32],
        start_message_uid: &str,
        end_message_uid: &str,
    ) -> Result<Option<Vec<HermesCopiedSourceTurn>>, ProductError> {
        self.require_rel_read(owner)?;
        Ok(self.inner.read_hermes_copy_source(
            owner.into(),
            scope,
            source_sha256,
            start_message_uid,
            end_message_uid,
        )?)
    }

    /// Page the current copied source's stable message IDs in newest-first order.
    /// Each page admits up to 128 IDs and checks the exact authorized head again.
    pub fn read_hermes_copy_active_message_uids(
        &self,
        owner: OwnerId,
        scope: &HermesCopyScope,
        source_sha256: [u8; 32],
        from_leaf: usize,
        limit: usize,
    ) -> Result<Option<Vec<String>>, ProductError> {
        self.require_rel_read(owner)?;
        Ok(self.inner.read_hermes_copy_active_message_uids(
            owner.into(),
            scope,
            source_sha256,
            from_leaf,
            limit,
        )?)
    }

    /// Apply one committed host deletion or rewind to an accepted copied-source head.
    /// Retries of the same scoped host event return its original durable receipt.
    pub fn apply_hermes_copy_source_change(
        &self,
        owner: OwnerId,
        scope: &HermesCopyScope,
        source_sha256: [u8; 32],
        event_id: &str,
        change: HermesCopySourceChange,
    ) -> Result<HermesCopySourceChangeReceipt, ProductError> {
        self.require(OwnerGrant::OperateRel(owner))?;
        Ok(self.inner.apply_hermes_copy_source_change(
            owner.into(),
            scope,
            source_sha256,
            event_id,
            change,
        )?)
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

    /// Embed the current accepted Archive while the named native import is open.
    /// The Archive generation is independent of the semantic import plan.
    pub fn publish_native_import_archive_vectors(
        &self,
        owner: OwnerId,
        import_id: &str,
        endpoint: Arc<dyn EmbeddingEndpoint>,
    ) -> Result<NativeImportArchiveVectorPublication, ProductError> {
        self.require(OwnerGrant::OperateRel(owner))?;
        validate_import_id(import_id)?;
        Ok(self.inner.publish_native_import_archive_vectors(
            owner.into(),
            import_id,
            &SwitchboardEmbeddingEndpoint::new(endpoint),
        )?)
    }

    /// Read durable staging state through the currently mounted REL projection.
    pub fn native_import_status(
        &self,
        owner: OwnerId,
        import_id: &str,
    ) -> Result<Option<NativeImportStatus>, ProductError> {
        self.require_rel_read(owner)?;
        validate_import_id(import_id)?;
        Ok(self.inner.native_import_status(owner.into(), import_id)?)
    }
}

fn validate_import_id(import_id: &str) -> Result<(), ProductError> {
    if import_id.len() != 64
        || !import_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ProductError::invalid("invalid native import ID"));
    }
    Ok(())
}
