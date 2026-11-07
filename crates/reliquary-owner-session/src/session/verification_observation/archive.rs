use super::{CurrentOwnerSession, ReadOnlyCurrentOwnerObserver};
use reliquary_archive::ArchiveHistorySnapshot;
use std::collections::BTreeMap;

pub struct ArchiveCutObservation {
    pub archive_head: u64,
    pub snapshot: ArchiveHistorySnapshot,
    pub turn_origins: Vec<ArchiveTurnOrigin>,
}

pub struct ArchiveTurnOrigin {
    pub archive_version: u64,
    pub prune_manifest_digest: Option<[u8; 32]>,
}

impl ReadOnlyCurrentOwnerObserver {
    /// Read one immutable Archive cut under the held current-owner lease.
    /// Retained payload availability is determined at the held file, even for
    /// a cut before maintenance. No pruned body is recovered by shared content ID.
    pub fn archive_cut(&self, version: u64) -> Result<ArchiveCutObservation, String> {
        let CurrentOwnerSession::Rel(session) = &self.session else {
            return Err("archive_observation_requires_rel".into());
        };
        let archive = &session.archive;
        let mut snapshot = archive
            .history_at(version)
            .map_err(|_| "archive_cut_unavailable")?;
        let pruned = archive
            .pruned_turns()
            .into_iter()
            .map(|(node, digest)| ((node.conversation_id, node.id), digest))
            .collect::<BTreeMap<_, _>>();
        let mut turn_origins = Vec::with_capacity(snapshot.turns.len());
        for turn in &mut snapshot.turns {
            let node = &turn.node;
            let archive_version = archive
                .node_archive_version(&node.conversation_id, &node.id)
                .filter(|origin| *origin <= version)
                .ok_or("archive_turn_origin_missing_or_after_cut")?;
            let prune_manifest_digest = pruned
                .get(&(node.conversation_id.clone(), node.id.clone()))
                .copied();
            if prune_manifest_digest.is_some() {
                turn.content = None;
            } else if turn.content.is_none() {
                return Err("archive_turn_body_missing_without_prune_evidence".into());
            }
            turn_origins.push(ArchiveTurnOrigin {
                archive_version,
                prune_manifest_digest,
            });
        }
        Ok(ArchiveCutObservation {
            archive_head: archive.archive_version(),
            snapshot,
            turn_origins,
        })
    }
}
