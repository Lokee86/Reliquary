use super::{ArchiveCutObservation, CurrentOwnerSession, ReadOnlyCurrentOwnerObserver};
use reliquary_archive::{EpisodeId, ProjectCorrelation};
use std::collections::BTreeMap;

pub struct ArchiveOwnerHistoryObservation {
    pub history: ArchiveCutObservation,
    pub episode_origins: BTreeMap<EpisodeId, u64>,
    pub project_correlations: Vec<ProjectCorrelation>,
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
        Ok(ArchiveOwnerHistoryObservation {
            history,
            episode_origins,
            project_correlations: session.project_revision_correlations().to_vec(),
        })
    }
}
