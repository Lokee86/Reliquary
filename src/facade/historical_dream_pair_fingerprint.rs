//! Test-only fingerprint for the clockless Dream pair membership set.
//!
//! Pair membership is already a canonical monotone set. This projection gives
//! it an explicit owner/domain identity without inventing a local version.

use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::{Cva, MemoryId, Phylactery};
use serde_json::json;
use std::collections::BTreeSet;

pub(crate) struct DreamPairCut {
    pub(crate) fingerprint: HistoricalFingerprint,
    pub(crate) pairs: BTreeSet<([u8; 32], [u8; 32])>,
}

pub(crate) fn reliquary_dream_pairs(cva: &Cva) -> Result<DreamPairCut, String> {
    dream_pairs(
        cva.owner_uuid()
            .ok_or("Dream pair oracle needs owner UUID")?,
        cva.dream_pair_records(),
    )
}

pub(crate) fn phylactery_dream_pairs(phy: &Phylactery) -> Result<DreamPairCut, String> {
    dream_pairs(
        phy.owner_uuid()
            .ok_or("Dream pair oracle needs owner UUID")?,
        phy.dream_pair_records(),
    )
}

fn dream_pairs(
    owner: [u8; 16],
    records: impl IntoIterator<Item = (MemoryId, MemoryId)>,
) -> Result<DreamPairCut, String> {
    let mut projection = OwnerProjection::new(owner, "dream_pairs", 0, None)?;
    let mut pairs = BTreeSet::new();
    for (left, right) in records {
        let (left, right) = if left.0 <= right.0 {
            (left.0, right.0)
        } else {
            (right.0, left.0)
        };
        if left == right {
            return Err("Dream pair oracle received a self-reference".into());
        }
        if !pairs.insert((left, right)) {
            return Err("Dream pair oracle received duplicate membership".into());
        }
        let mut key = Vec::with_capacity(64);
        key.extend_from_slice(&left);
        key.extend_from_slice(&right);
        projection.insert("pair", key, json!({"left": left, "right": right}))?;
    }
    Ok(DreamPairCut {
        fingerprint: projection.fingerprint()?,
        pairs,
    })
}

#[path = "historical_dream_pair_fingerprint_tests.rs"]
mod tests;
