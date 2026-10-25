use super::{
    ConfiguredInsomniaOptions, ConfiguredInsomniaReport, ConfiguredRuntime, ConfiguredRuntimeError,
    InsomniaTerminalFailure, operation,
};
use crate::{
    Cva, EpisodeConfig, InsomniaProgressReporter, InsomniaWorkState, InsomniaWorkerConfig,
    Phylactery,
};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

impl ConfiguredRuntime {
    pub fn run_insomnia_files(
        &self,
        rel_path: &Path,
        phy_path: Option<&Path>,
        options: ConfiguredInsomniaOptions,
    ) -> Result<ConfiguredInsomniaReport, ConfiguredRuntimeError> {
        self.run_insomnia_files_inner(rel_path, phy_path, options, None)
    }

    pub fn run_insomnia_files_with_progress<P: InsomniaProgressReporter>(
        &self,
        rel_path: &Path,
        phy_path: Option<&Path>,
        options: ConfiguredInsomniaOptions,
        progress: &P,
    ) -> Result<ConfiguredInsomniaReport, ConfiguredRuntimeError> {
        self.run_insomnia_files_inner(rel_path, phy_path, options, Some(progress))
    }

    fn run_insomnia_files_inner(
        &self,
        rel_path: &Path,
        phy_path: Option<&Path>,
        options: ConfiguredInsomniaOptions,
        progress: Option<&dyn InsomniaProgressReporter>,
    ) -> Result<ConfiguredInsomniaReport, ConfiguredRuntimeError> {
        let (extractor, embedding) = super::provider_operations::prepare_insomnia(
            &self.switchboard,
            options.embedding_batch_size,
            options.embedding_concurrency,
            phy_path.is_some(),
        )
        .map_err(operation)?;

        let mut rel = Cva::open(rel_path).map_err(operation)?;
        let mut phy = phy_path
            .map(Phylactery::open)
            .transpose()
            .map_err(operation)?;
        let before = rel.insomnia_stats();
        if !options.existing_queue_only {
            rel.finalize_canonical_imports_and_queue(EpisodeConfig::default(), now_ns())
                .map_err(operation)?;
        }
        let queued = rel.insomnia_stats();
        let worker_config = InsomniaWorkerConfig {
            workers: options.workers,
            scope: options.scope,
            ..Default::default()
        };
        let drain = match (phy.as_mut(), progress) {
            (Some(phy), Some(progress)) => rel
                .drain_insomnia_backlog_routed_with_progress(
                    phy,
                    &extractor,
                    &embedding,
                    worker_config,
                    progress,
                )
                .map_err(operation)?,
            (Some(phy), None) => rel
                .drain_insomnia_backlog_routed(phy, &extractor, &embedding, worker_config)
                .map_err(operation)?,
            (None, Some(progress)) => rel
                .drain_insomnia_backlog_with_progress(
                    &extractor,
                    &embedding,
                    worker_config,
                    progress,
                )
                .map_err(operation)?,
            (None, None) => rel
                .drain_insomnia_backlog(&extractor, &embedding, worker_config)
                .map_err(operation)?,
        };
        rel.sync().map_err(operation)?;
        if let Some(phy) = &phy {
            phy.sync().map_err(operation)?;
        }
        let final_stats = rel.insomnia_stats();
        let terminal_failures = rel
            .episodes()
            .into_iter()
            .filter_map(|episode| {
                rel.insomnia_work(episode.id).and_then(|work| {
                    (work.state == InsomniaWorkState::Terminal).then(|| InsomniaTerminalFailure {
                        episode_id: episode.id,
                        error: work
                            .last_error
                            .clone()
                            .unwrap_or_else(|| "<missing>".into()),
                    })
                })
            })
            .collect();
        Ok(ConfiguredInsomniaReport {
            before,
            queued,
            final_stats,
            drain,
            terminal_failures,
        })
    }
}

fn now_ns() -> i64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    i64::try_from(nanos).unwrap_or(i64::MAX)
}
