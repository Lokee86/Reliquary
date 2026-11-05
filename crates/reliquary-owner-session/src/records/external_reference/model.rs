use crate::records::external_index::model::{
    EXTERNAL_SOURCE_CONTRACT, ExternalMessageDescriptor, ExternalSourceError,
    MAX_EXTERNAL_INDEX_BYTES, MAX_EXTERNAL_MANIFEST, identifier,
};
use std::collections::BTreeSet;
pub(super) const MAX_PRINCIPAL_BYTES: usize = 512;
pub(super) const MAX_NAMESPACE_BYTES: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalReferenceSnapshotManifest {
    pub namespace: String,
    pub contract_version: u32,
    pub watermark: u64,
    pub descriptors: Vec<ExternalMessageDescriptor>,
}

impl ExternalReferenceSnapshotManifest {
    /// Check the provider-neutral shape and all record-size bounds before the
    /// owner receives or encodes a durable receipt.
    pub fn validate(&self) -> Result<(), ExternalSourceError> {
        identifier(&self.namespace)?;
        if self.namespace.len() > MAX_NAMESPACE_BYTES
            || self.contract_version != EXTERNAL_SOURCE_CONTRACT
            || self.descriptors.len() > MAX_EXTERNAL_MANIFEST
        {
            return Err(ExternalSourceError::InvalidContract);
        }
        let mut seen = BTreeSet::new();
        for descriptor in &self.descriptors {
            descriptor.validate(&self.namespace)?;
            if !seen.insert((
                descriptor.reference.conversation_id.as_str(),
                descriptor.reference.message_id.as_str(),
            )) {
                return Err(ExternalSourceError::InvalidContract);
            }
        }
        let size = self.encoded_body_len("")?;
        if size > MAX_EXTERNAL_INDEX_BYTES {
            return Err(ExternalSourceError::Bounds);
        }
        Ok(())
    }

    fn encoded_body_len(&self, principal: &str) -> Result<usize, ExternalSourceError> {
        let mut size: usize = 8 + 16 + 8 + 4 + 8;
        for text in [principal, self.namespace.as_str()] {
            size = size
                .checked_add(4)
                .and_then(|size| size.checked_add(text.len()))
                .ok_or(ExternalSourceError::Bounds)?;
        }
        for descriptor in &self.descriptors {
            for text in [
                descriptor.reference.source_namespace.as_str(),
                descriptor.reference.conversation_id.as_str(),
                descriptor.reference.message_id.as_str(),
                descriptor.role.as_str(),
            ] {
                size = size
                    .checked_add(4)
                    .and_then(|size| size.checked_add(text.len()))
                    .ok_or(ExternalSourceError::Bounds)?;
            }
            size = size
                .checked_add(32 + 8 + 1)
                .and_then(|size| {
                    size.checked_add(
                        4 + descriptor
                            .principal_id
                            .as_ref()
                            .map_or(0, |principal| principal.len()),
                    )
                })
                .ok_or(ExternalSourceError::Bounds)?;
        }
        size.checked_add(32).ok_or(ExternalSourceError::Bounds)
    }
}

/// Opaque durable receipt that a caller-bound host manifest was recorded for
/// one REL. It makes no claim that a Memory was created or that source text
/// was copied into the owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalReferenceSnapshotReceipt {
    pub(super) owner_id: [u8; 16],
    pub(super) authorizing_principal: String,
    pub(super) manifest: ExternalReferenceSnapshotManifest,
    pub(super) digest: [u8; 32],
}

impl ExternalReferenceSnapshotReceipt {
    pub fn owner_id(&self) -> [u8; 16] {
        self.owner_id
    }

    pub fn authorizing_principal(&self) -> &str {
        &self.authorizing_principal
    }

    pub fn manifest(&self) -> &ExternalReferenceSnapshotManifest {
        &self.manifest
    }

    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
}
