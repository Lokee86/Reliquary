//! Owner-neutral, non-semantic evidence for destructive transformed-copy maintenance.
//!
//! Domain owners still decide which payloads are eligible. This store only keeps
//! the durable proofs that bind those owner decisions to markers in a file.
#[path = "history_prune_manifest.rs"]
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

    pub(crate) fn validate_source_history(
        &self,
        container: &mut reliquary_container::Container,
        graph: &crate::graph_owner::GraphOwner,
    ) -> Result<(), String> {
        for manifest in self.manifests.values() {
            if Some(manifest.owner_uuid) != container.owner_uuid()
                || manifest.source_global_version > container.latest_version()
                || owner_history_digest(container, manifest.source_global_version, graph)?
                    != manifest.source_history_digest
            {
                return Err("maintenance proof does not match its source owner history".into());
            }
        }
        Ok(())
    }
}
