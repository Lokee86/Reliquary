//! Retained settlement, work and epoch evidence for physical-copy verification.
use crate::owner_lifecycle::{HistoryReadFailure, HistorySource};
use sha2::{Digest, Sha256};

pub(crate) fn historical_owner_evidence_digest(
    source: &HistorySource<'_>,
) -> Result<[u8; 32], HistoryReadFailure> {
    let mut hash = Sha256::new();
    hash.update(b"reliquary:historical-owner-evidence:v1\0");
    let mut cursor = source.cursor()?;
    while let Some(frame) = cursor.next_frame()? {
        let payload = &frame.payload;
        // Existing concrete owner families contain stable IDs, not relocatable
        // physical references. Preserve their exact accepted byte sequence.
        if payload.starts_with(b"CVAFR")
            || payload.starts_with(b"CVAFSA")
            || payload.starts_with(b"CVDREAM")
            || payload.starts_with(b"CVAINSC")
            || payload.starts_with(b"CVAIEPS1")
            || payload.starts_with(b"CVAIEPC1")
        {
            hash.update((payload.len() as u64).to_le_bytes());
            hash.update(payload);
        }
    }
    source.validate()?;
    Ok(hash.finalize().into())
}
