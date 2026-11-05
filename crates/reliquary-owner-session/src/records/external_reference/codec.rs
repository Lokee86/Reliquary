use super::model::{
    ExternalReferenceSnapshotManifest, ExternalReferenceSnapshotReceipt, MAX_NAMESPACE_BYTES,
    MAX_PRINCIPAL_BYTES,
};
use crate::records::external_index::model::{
    ExternalSourceError, MAX_EXTERNAL_INDEX_BYTES, MAX_EXTERNAL_MANIFEST, identifier,
};
use crate::records::external_index::{ExternalMessageDescriptor, ExternalMessageRef};
use sha2::{Digest, Sha256};

pub(super) const MAGIC: &[u8; 8] = b"RELXHST1";

pub(super) fn make_receipt(
    owner_id: [u8; 16],
    principal: &str,
    manifest: ExternalReferenceSnapshotManifest,
) -> Result<ExternalReferenceSnapshotReceipt, ExternalSourceError> {
    manifest.validate()?;
    let mut receipt = ExternalReferenceSnapshotReceipt {
        owner_id,
        authorizing_principal: principal.to_owned(),
        manifest,
        digest: [0; 32],
    };
    let bytes = encode_without_digest(&receipt)?;
    receipt.digest = Sha256::digest(bytes).into();
    Ok(receipt)
}

pub(super) fn encode(
    receipt: &ExternalReferenceSnapshotReceipt,
) -> Result<Vec<u8>, ExternalSourceError> {
    let mut bytes = encode_without_digest(receipt)?;
    bytes.extend_from_slice(&receipt.digest);
    if bytes.len() > MAX_EXTERNAL_INDEX_BYTES {
        return Err(ExternalSourceError::Bounds);
    }
    Ok(bytes)
}

fn encode_without_digest(
    receipt: &ExternalReferenceSnapshotReceipt,
) -> Result<Vec<u8>, ExternalSourceError> {
    let manifest = &receipt.manifest;
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&receipt.owner_id);
    put_string(&mut out, &receipt.authorizing_principal)?;
    put_string(&mut out, &manifest.namespace)?;
    out.extend_from_slice(&manifest.contract_version.to_le_bytes());
    out.extend_from_slice(&manifest.watermark.to_le_bytes());
    let count =
        u32::try_from(manifest.descriptors.len()).map_err(|_| ExternalSourceError::Bounds)?;
    out.extend_from_slice(&count.to_le_bytes());
    for descriptor in &manifest.descriptors {
        put_string(&mut out, &descriptor.reference.source_namespace)?;
        put_string(&mut out, &descriptor.reference.conversation_id)?;
        put_string(&mut out, &descriptor.reference.message_id)?;
        out.extend_from_slice(&descriptor.reference.content_hash);
        put_string(&mut out, &descriptor.role)?;
        match &descriptor.principal_id {
            Some(principal) => {
                out.push(1);
                put_string(&mut out, principal)?;
            }
            None => out.push(0),
        }
        out.extend_from_slice(&descriptor.timestamp_ns.to_le_bytes());
        out.push(u8::from(descriptor.active));
        if out.len() > MAX_EXTERNAL_INDEX_BYTES {
            return Err(ExternalSourceError::Bounds);
        }
    }
    if out.len() > MAX_EXTERNAL_INDEX_BYTES {
        return Err(ExternalSourceError::Bounds);
    }
    Ok(out)
}

pub(super) fn decode(
    bytes: &[u8],
) -> Result<ExternalReferenceSnapshotReceipt, ExternalSourceError> {
    if bytes.len() > MAX_EXTERNAL_INDEX_BYTES || bytes.len() < 8 + 16 + 4 + 4 + 4 + 8 + 4 + 32 {
        return Err(ExternalSourceError::Bounds);
    }
    let digest_offset = bytes.len() - 32;
    let expected_digest: [u8; 32] = Sha256::digest(&bytes[..digest_offset]).into();
    if bytes[digest_offset..] != expected_digest {
        return Err(ExternalSourceError::InvalidContract);
    }
    let mut cursor = Cursor {
        bytes: &bytes[..digest_offset],
        offset: 0,
    };
    if cursor.take(8)? != MAGIC {
        return Err(ExternalSourceError::InvalidContract);
    }
    let owner_id = cursor.array16()?;
    let authorizing_principal = cursor.string(MAX_PRINCIPAL_BYTES)?;
    let namespace = cursor.string(MAX_NAMESPACE_BYTES)?;
    let contract_version = cursor.u32()?;
    let watermark = cursor.u64()?;
    let count = cursor.count(MAX_EXTERNAL_MANIFEST, 4 + 4 + 4 + 32 + 4 + 1 + 8 + 1)?;
    let mut descriptors = Vec::with_capacity(count);
    for _ in 0..count {
        let source_namespace = cursor.string(MAX_NAMESPACE_BYTES)?;
        let conversation_id = cursor.string(512)?;
        let message_id = cursor.string(512)?;
        let content_hash = cursor.array32()?;
        let role = cursor.string(64)?;
        let principal_id = match cursor.byte()? {
            0 => None,
            1 => Some(cursor.string(MAX_PRINCIPAL_BYTES)?),
            _ => return Err(ExternalSourceError::InvalidContract),
        };
        let timestamp_ns = cursor.i64()?;
        let active = cursor.boolean()?;
        descriptors.push(ExternalMessageDescriptor {
            reference: ExternalMessageRef {
                source_namespace,
                conversation_id,
                message_id,
                content_hash,
            },
            role,
            principal_id,
            timestamp_ns,
            active,
        });
    }
    if !cursor.done() {
        return Err(ExternalSourceError::InvalidContract);
    }
    let manifest = ExternalReferenceSnapshotManifest {
        namespace,
        contract_version,
        watermark,
        descriptors,
    };
    manifest.validate()?;
    identifier(&authorizing_principal)?;
    let receipt = ExternalReferenceSnapshotReceipt {
        owner_id,
        authorizing_principal,
        manifest,
        digest: expected_digest,
    };
    if encode_without_digest(&receipt)? != bytes[..digest_offset] {
        return Err(ExternalSourceError::InvalidContract);
    }
    Ok(receipt)
}

fn put_string(out: &mut Vec<u8>, value: &str) -> Result<(), ExternalSourceError> {
    let length = u32::try_from(value.len()).map_err(|_| ExternalSourceError::Bounds)?;
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    if out.len() > MAX_EXTERNAL_INDEX_BYTES {
        return Err(ExternalSourceError::Bounds);
    }
    Ok(())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, size: usize) -> Result<&'a [u8], ExternalSourceError> {
        let end = self
            .offset
            .checked_add(size)
            .ok_or(ExternalSourceError::Bounds)?;
        let result = self
            .bytes
            .get(self.offset..end)
            .ok_or(ExternalSourceError::Bounds)?;
        self.offset = end;
        Ok(result)
    }

    fn string(&mut self, max: usize) -> Result<String, ExternalSourceError> {
        let size = usize::try_from(self.u32()?).map_err(|_| ExternalSourceError::Bounds)?;
        if size > max {
            return Err(ExternalSourceError::Bounds);
        }
        String::from_utf8(self.take(size)?.to_vec())
            .map_err(|_| ExternalSourceError::InvalidContract)
    }

    fn count(&mut self, max: usize, minimum_item: usize) -> Result<usize, ExternalSourceError> {
        let count = usize::try_from(self.u32()?).map_err(|_| ExternalSourceError::Bounds)?;
        if count > max
            || count
                .checked_mul(minimum_item)
                .is_none_or(|size| size > self.bytes.len() - self.offset)
        {
            return Err(ExternalSourceError::Bounds);
        }
        Ok(count)
    }

    fn byte(&mut self) -> Result<u8, ExternalSourceError> {
        Ok(self.take(1)?[0])
    }

    fn boolean(&mut self) -> Result<bool, ExternalSourceError> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(ExternalSourceError::InvalidContract),
        }
    }

    fn u32(&mut self) -> Result<u32, ExternalSourceError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, ExternalSourceError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn i64(&mut self) -> Result<i64, ExternalSourceError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn array16(&mut self) -> Result<[u8; 16], ExternalSourceError> {
        Ok(self.take(16)?.try_into().unwrap())
    }

    fn array32(&mut self) -> Result<[u8; 32], ExternalSourceError> {
        Ok(self.take(32)?.try_into().unwrap())
    }

    fn done(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
