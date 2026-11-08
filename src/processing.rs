use crate::CompatibilityProfileId;
use crate::{ConfiguredRuntime, Instance, OwnerGrant, OwnerId, ProductError, ProductErrorKind};
use reliquary_owner_session::model::EpisodeId;
use reliquary_runtime_gateway as runtime;
use std::sync::Arc;

pub use reliquary_application::live_processing::{LiveProcessingSelection, LiveStageProgress};
pub use reliquary_application::processing_plan::{
    ImportStage, InsomniaProgress, MAX_PROCESS_EPISODES, StageProgress, StageState,
};
pub use reliquary_application::{FreshnessLifecycleState, FreshnessTransition};
pub use runtime::{ProcessingReport, ProcessingState, VectorCoverage};

#[derive(Clone, Debug)]
pub struct ProcessOptions {
    pub import_id: Option<String>,
    pub destination: Option<OwnerId>,
    pub max_episodes: usize,
}

impl Default for ProcessOptions {
    fn default() -> Self {
        Self {
            import_id: None,
            destination: None,
            max_episodes: 128,
        }
    }
}

pub struct OwnerProcessingJob {
    instance: Instance,
    owner: OwnerId,
    destination: Option<OwnerId>,
    inner: runtime::ProcessingJob,
}

impl OwnerProcessingJob {
    pub fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }

    pub fn poll(&self) -> Result<ProcessingReport, ProductError> {
        self.instance.require(OwnerGrant::OperateRel(self.owner))?;
        let mut report = self.inner.poll()?;
        if report.state == ProcessingState::Running {
            report.insomnia = self
                .instance
                .inner
                .insomnia_progress(self.owner.into(), report.import_id.as_deref())?;
        }
        self.filter_private_counts(&mut report)?;
        Ok(report)
    }

    pub fn request_stop(&self) {
        self.inner.request_stop();
    }

    pub fn join(self) -> Result<ProcessingReport, ProductError> {
        let Self {
            instance,
            owner,
            destination,
            inner,
        } = self;
        let mut report = inner.join()?;
        instance.require(OwnerGrant::OperateRel(owner))?;
        filter_private_counts(&instance, owner, destination, &mut report)?;
        Ok(report)
    }

    fn filter_private_counts(&self, report: &mut ProcessingReport) -> Result<(), ProductError> {
        filter_private_counts(&self.instance, self.owner, self.destination, report)
    }
}

fn filter_private_counts(
    instance: &Instance,
    owner: OwnerId,
    destination: Option<OwnerId>,
    report: &mut ProcessingReport,
) -> Result<(), ProductError> {
    if let Some(phy) = destination {
        instance.require(OwnerGrant::OperatePhylactery(phy))?;
        report.insomnia.phy_counts_visible = true;
        if report.import_id.is_none() {
            report.insomnia.phy_memories = instance
                .inner
                .live_routed_memory_count(owner.into(), phy.into())?;
        }
    } else {
        report.insomnia.phy_memories = 0;
    }
    Ok(())
}

impl Instance {
    pub fn vector_status(
        &self,
        owner: OwnerId,
        profile: CompatibilityProfileId,
    ) -> Result<VectorCoverage, ProductError> {
        self.require_rel_read(owner)?;
        self.inner
            .rel_vector_coverage(owner.into(), profile)
            .map_err(Into::into)
    }

    pub fn build_vectors(
        &self,
        owner: OwnerId,
        runtime: &ConfiguredRuntime,
        limit: usize,
    ) -> Result<VectorCoverage, ProductError> {
        self.require(OwnerGrant::OperateRel(owner))?;
        let endpoint = runtime.embedding_endpoint().ok_or_else(|| {
            ProductError::new(
                ProductErrorKind::Unavailable,
                "vector work requires a configured embedding route",
            )
        })?;
        self.inner
            .build_rel_vectors(owner.into(), endpoint, limit)
            .map_err(Into::into)
    }

    pub fn freshness_due(
        &self,
        owner: OwnerId,
        limit: usize,
    ) -> Result<Vec<FreshnessTransition>, ProductError> {
        self.require_rel_read(owner)?;
        self.inner
            .rel_freshness_due(owner.into(), limit)
            .map_err(Into::into)
    }

    pub fn processing_status(
        &self,
        owner: OwnerId,
        import_id: Option<&str>,
    ) -> Result<InsomniaProgress, ProductError> {
        self.require_rel_read(owner)?;
        let mut progress = self.inner.insomnia_progress(owner.into(), import_id)?;
        progress.phy_memories = 0;
        Ok(progress)
    }

    pub fn retry_held_import_episode(
        &self,
        owner: OwnerId,
        import_id: &str,
        episode: EpisodeId,
    ) -> Result<(), ProductError> {
        self.require(OwnerGrant::OperateRel(owner))?;
        self.inner
            .retry_held_import_episode(owner.into(), import_id, episode)
            .map_err(Into::into)
    }

    pub fn live_stage_status(
        &self,
        owner: OwnerId,
        destination: Option<OwnerId>,
        stage: ImportStage,
    ) -> Result<LiveStageProgress, ProductError> {
        self.require_rel_read(owner)?;
        if let Some(phy) = destination {
            self.require_phy_read(phy)?;
        }
        Ok(self
            .inner
            .live_stage_status(owner.into(), destination.map(Into::into), stage)?)
    }

    pub fn retry_held_live_episode(
        &self,
        owner: OwnerId,
        episode: EpisodeId,
    ) -> Result<(), ProductError> {
        self.require(OwnerGrant::OperateRel(owner))?;
        Ok(self.inner.retry_held_live_episode(owner.into(), episode)?)
    }

    pub fn start_live_processing_stage(
        &self,
        owner: OwnerId,
        runtime: &ConfiguredRuntime,
        destination: Option<OwnerId>,
        max_episodes: usize,
        selection: LiveProcessingSelection,
    ) -> Result<OwnerProcessingJob, ProductError> {
        self.start_processing_selected(
            owner,
            runtime,
            ProcessOptions {
                import_id: None,
                destination,
                max_episodes,
            },
            selection,
            None,
        )
    }

    pub fn start_processing(
        &self,
        owner: OwnerId,
        runtime: &ConfiguredRuntime,
        options: ProcessOptions,
    ) -> Result<OwnerProcessingJob, ProductError> {
        self.start_processing_selected(owner, runtime, options, LiveProcessingSelection::All, None)
    }

    /// Retry an exact held Episode after validating the owner and processing plan.
    pub fn resume_held_processing(
        &self,
        owner: OwnerId,
        runtime: &ConfiguredRuntime,
        options: ProcessOptions,
        episode: EpisodeId,
    ) -> Result<OwnerProcessingJob, ProductError> {
        self.start_processing_selected(
            owner,
            runtime,
            options,
            LiveProcessingSelection::All,
            Some(episode),
        )
    }

    fn start_processing_selected(
        &self,
        owner: OwnerId,
        runtime: &ConfiguredRuntime,
        options: ProcessOptions,
        live_selection: LiveProcessingSelection,
        retry_episode: Option<EpisodeId>,
    ) -> Result<OwnerProcessingJob, ProductError> {
        self.require(OwnerGrant::OperateRel(owner))?;
        if let Some(phy) = options.destination {
            self.require(OwnerGrant::OperatePhylactery(phy))?;
        }
        let endpoint = runtime.insomnia_endpoint();
        let route_identity = runtime.processing_route_identity();
        let job = self.gateway.inner.start_processing(
            Arc::clone(&self.inner),
            owner.into(),
            endpoint,
            runtime.insomnia_ownership_endpoint(),
            runtime.dream_endpoint(),
            runtime.entity_endpoint(),
            runtime.embedding_endpoint(),
            route_identity,
            retry_episode,
            runtime::InsomniaRunOptions {
                import_id: options.import_id,
                destination: options.destination.map(Into::into),
                max_episodes: options.max_episodes,
                lease_duration_ns: runtime.worker_lease_duration_ns(),
                live_selection,
            },
        )?;
        Ok(OwnerProcessingJob {
            instance: self.clone(),
            owner,
            destination: options.destination,
            inner: job,
        })
    }

    pub fn process_and_drain(
        &self,
        owner: OwnerId,
        runtime: &ConfiguredRuntime,
        options: ProcessOptions,
    ) -> Result<ProcessingReport, ProductError> {
        self.start_processing(owner, runtime, options)?.join()
    }
}
