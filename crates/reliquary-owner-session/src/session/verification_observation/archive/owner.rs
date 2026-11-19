use super::{ArchiveCutObservation, CurrentOwnerSession, ReadOnlyCurrentOwnerObserver};
use reliquary_archive::{EpisodeId, ProjectBackingBinding, ProjectCorrelation};
use std::collections::BTreeMap;

pub struct ArchiveOwnerHistoryObservation {
    pub history: ArchiveCutObservation,
    pub episode_origins: BTreeMap<EpisodeId, u64>,
    pub project_correlations: Vec<ProjectCorrelation>,
    pub project_backing: Vec<(u64, ProjectBackingBinding)>,
}

impl ReadOnlyCurrentOwnerObserver {
    /// Observe retained Archive values through the exact current owner head.
    pub fn archive_owner_history(&self) -> Result<ArchiveOwnerHistoryObservation, String> {
        let CurrentOwnerSession::Rel(session) = &self.session else {
            return Err("archive_observation_requires_rel".into());
        };
        let history = self.archive_cut(session.archive_version())?;
        let episode_origins = history
            .snapshot
            .episodes
            .iter()
            .map(|episode| {
                session
                    .archive
                    .episode_archive_version(episode.id)
                    .map(|version| (episode.id, version))
                    .ok_or("archive_episode_origin_missing")
            })
            .collect::<Result<_, _>>()?;
        let mut project_origins = BTreeMap::new();
        for (turn, origin) in history.snapshot.turns.iter().zip(&history.turn_origins) {
            for attachment in &turn.attachments {
                if attachment.project_reference.is_some() {
                    project_origins
                        .entry(attachment.id)
                        .or_insert(origin.archive_version);
                }
            }
        }
        let project_backing = session
            .archive
            .project_backing_bindings()
            .iter()
            .map(|binding| {
                let previous = binding
                    .global_version
                    .checked_sub(1)
                    .ok_or("project_backing_has_no_previous_cut")?;
                if session
                    .archive
                    .project_backing_at_global_cut(binding.attachment_id, previous)
                    .is_some()
                    || session.archive.project_backing_at_global_cut(
                        binding.attachment_id,
                        binding.global_version,
                    ) != Some(binding)
                {
                    return Err("project_backing_availability_cut_differs");
                }
                project_origins
                    .get(&binding.attachment_id)
                    .copied()
                    .map(|origin| (origin, *binding))
                    .ok_or("project_backing_has_no_archive_origin")
            })
            .collect::<Result<_, _>>()?;
        Ok(ArchiveOwnerHistoryObservation {
            history,
            episode_origins,
            project_correlations: session.project_revision_correlations().to_vec(),
            project_backing,
        })
    }
}
