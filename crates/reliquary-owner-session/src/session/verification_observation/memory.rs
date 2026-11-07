use super::{CurrentOwnerSession, ReadOnlyCurrentOwnerObserver};
use crate::records::PrunedPayload;
use crate::session::historical_proof::grouped::validate_grouped_memory_cut;
use reliquary_memory_core::{MemoryBodyId, MemoryHistory, MemoryId, PayloadState};
use std::collections::BTreeMap;

mod origins;
pub use origins::MemoryPublicationOrigin;
use origins::memory_origins;

pub struct MemoryHistoryCutObservation {
    pub memory_head: u64,
    pub history: Option<MemoryHistory>,
    pub pruned_body_manifests: BTreeMap<MemoryBodyId, [u8; 32]>,
    pub standalone_publication_origins: BTreeMap<u64, MemoryPublicationOrigin>,
}

impl ReadOnlyCurrentOwnerObserver {
    /// Read one Memory's retained revisions through a supported local cut.
    /// Availability is the held file's current retained-body fact.
    pub fn memory_history_cut(
        &self,
        id: MemoryId,
        version: u64,
    ) -> Result<MemoryHistoryCutObservation, String> {
        if version > self.observation.memory_version {
            return Err("memory_cut_unavailable".into());
        }
        let (history, maintenance) = match &self.session {
            CurrentOwnerSession::Rel(session) => {
                validate_grouped_memory_cut(&session.grouped_completions, version)
                    .map_err(|_| "memory_cut_splits_grouped_publication")?;
                (
                    session.memory_history_at_cut(id, version),
                    &session.common.maintenance,
                )
            }
            CurrentOwnerSession::Phy(session) => (
                session.memory_history_at_cut(id, version),
                &session.common.maintenance,
            ),
        };
        let history = history.map_err(|_| "memory_history_unavailable")?;
        let mut pruned_body_manifests = BTreeMap::new();
        if let Some(history) = &history {
            for revision in &history.revisions {
                if !matches!(revision.payload, PayloadState::Absent) {
                    continue;
                }
                let body_id = revision.record.body_id;
                for (digest, manifest) in maintenance.manifests() {
                    let mentions_body = manifest.descriptors.iter().any(|descriptor| {
                        descriptor
                            .payloads
                            .contains(&PrunedPayload::MemoryBody(body_id.0))
                    });
                    if mentions_body
                        && pruned_body_manifests
                            .insert(body_id, *digest)
                            .is_some_and(|previous| previous != *digest)
                    {
                        return Err("memory_body_prune_manifest_ambiguous".into());
                    }
                }
            }
        }
        Ok(MemoryHistoryCutObservation {
            memory_head: self.observation.memory_version,
            history,
            pruned_body_manifests,
            standalone_publication_origins: memory_origins(&self.container, id, version)?,
        })
    }
}
