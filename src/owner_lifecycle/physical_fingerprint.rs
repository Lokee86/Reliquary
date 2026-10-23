//! Exact physical-file digest used by held offline import operations.
use super::OwnerError;
use sha2::{Digest, Sha256};
use std::io::Read;
pub(crate) fn file_digest(path: &std::path::Path) -> Result<[u8; 32], OwnerError> {
    let mut file = std::fs::File::open(path).map_err(|e| OwnerError::Freshness(e.to_string()))?;
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 65536];
    loop {
        let size = file
            .read(&mut bytes)
            .map_err(|e| OwnerError::Freshness(e.to_string()))?;
        if size == 0 {
            break;
        }
        hash.update(&bytes[..size]);
    }
    Ok(hash.finalize().into())
}
