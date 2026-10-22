//! Owner-neutral, non-semantic evidence for destructive transformed-copy maintenance.
//!
//! Domain owners still decide which payloads are eligible. This store only keeps
//! the durable proofs that bind those owner decisions to markers in a file.
#[path = "manifest.rs"]
mod manifest;

pub(crate) use manifest::*;

use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct OwnerMaintenanceStore {
    manifests: BTreeMap<[u8; 32], HistoryPruneManifest>,
}

impl OwnerMaintenanceStore {
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<bool, String> {
        if !payload.starts_with(b"CVAHPR01") {
            return Ok(false);
        }
        let manifest = HistoryPruneManifest::decode(payload)?;
        let digest = manifest.digest()?;
        if self.manifests.insert(digest, manifest).is_some() {
            return Err("duplicate prune manifest".into());
        }
        Ok(true)
    }

    pub(crate) fn get(&self, digest: &[u8; 32]) -> Option<&HistoryPruneManifest> {
        self.manifests.get(digest)
    }

    pub(crate) fn manifests(&self) -> &BTreeMap<[u8; 32], HistoryPruneManifest> {
        &self.manifests
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.manifests.is_empty()
    }
}
