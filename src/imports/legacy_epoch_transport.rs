//! Physical transport of an unchanged legacy import proof. No semantic clock.
use super::{ImportError, LegacyImportEpoch, legacy_epoch};
use sha2::{Digest, Sha256};

const MAGIC: &[u8; 8] = b"CVAILEG2";
const MAX_ORIGINAL: usize = 16 * 1024;

pub(crate) fn original_payload(payload: &[u8]) -> Result<Option<&[u8]>, ImportError> {
    if decode(payload)?.is_none() {
        return Ok(None);
    }
    let len = u32::from_le_bytes(payload[8..12].try_into().unwrap()) as usize;
    Ok(Some(&payload[12..12 + len]))
}

/// A physical copy binds its already-written prefix, retaining original proof.
pub(crate) fn rebind_copy<'a>(
    payload: &'a [u8],
    output: &crate::Container,
) -> Result<std::borrow::Cow<'a, [u8]>, ImportError> {
    let Some(original) = original_payload(payload)? else {
        return Ok(std::borrow::Cow::Borrowed(payload));
    };
    let length = output
        .backing_file()
        .metadata()
        .map_err(crate::ContainerError::Io)?
        .len();
    let hex = super::hash_file_prefix(output.path(), length)?;
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| ImportError::Epoch("invalid copied prefix digest"))?;
    }
    encode(original, &PrefixBinding { length, digest }).map(std::borrow::Cow::Owned)
}

#[derive(Clone)]
pub(crate) struct PrefixBinding {
    pub length: u64,
    pub digest: [u8; 32],
}

pub(crate) fn encode(original: &[u8], binding: &PrefixBinding) -> Result<Vec<u8>, ImportError> {
    legacy_epoch::decode(original)?.ok_or(ImportError::Epoch("missing original legacy proof"))?;
    let mut out = Vec::with_capacity(original.len() + 84);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(original.len() as u32).to_le_bytes());
    out.extend_from_slice(original);
    out.extend_from_slice(&binding.length.to_le_bytes());
    out.extend_from_slice(&binding.digest);
    out.extend_from_slice(&Sha256::digest(&out));
    Ok(out)
}

pub(crate) fn decode(
    payload: &[u8],
) -> Result<Option<(LegacyImportEpoch, PrefixBinding)>, ImportError> {
    if !payload.starts_with(MAGIC) {
        if payload.starts_with(b"CVAILEG") && !payload.starts_with(legacy_epoch::MAGIC) {
            return Err(ImportError::Epoch("unknown legacy epoch variant"));
        }
        return Ok(None);
    }
    let invalid = || ImportError::Epoch("invalid relocated legacy proof");
    if !(128..=MAX_ORIGINAL + 84).contains(&payload.len()) {
        return Err(invalid());
    }
    let len = u32::from_le_bytes(payload[8..12].try_into().unwrap()) as usize;
    if len.checked_add(84) != Some(payload.len()) {
        return Err(invalid());
    }
    let end = payload.len() - 32;
    if Sha256::digest(&payload[..end]).as_slice() != &payload[end..] {
        return Err(invalid());
    }
    let original = legacy_epoch::decode(&payload[12..12 + len])?.ok_or_else(invalid)?;
    let cursor = 12 + len;
    Ok(Some((
        original,
        PrefixBinding {
            length: u64::from_le_bytes(payload[cursor..cursor + 8].try_into().unwrap()),
            digest: payload[cursor + 8..cursor + 40].try_into().unwrap(),
        },
    )))
}
