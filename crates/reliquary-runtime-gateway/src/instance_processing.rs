use crate::{
    Gateway, GatewayError, Instance, OwnerId,
    insomnia_worker::{SharedEndpoint, now_ns, processing::process_claim},
    live_processing::run_live_stages,
    processing_stages::{ImportProcessingRoutes, run_import_stages},
};
use reliquary_application::live_processing::{LiveProcessingSelection, LiveStageProgress};
use reliquary_application::processing_plan::{
    InsomniaProgress, MAX_PROCESS_EPISODES, StageProgress, import_plan_identity, import_stages,
    inspect_insomnia_work,
};
use reliquary_owner_session::model::EpisodeId;
use reliquary_switchboard::{EmbeddingEndpoint, GeneralEndpoint, ProcessingRouteIdentity};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};

#[derive(Clone, Debug)]
pub struct InsomniaRunOptions {
    pub import_id: Option<String>,
    pub destination: Option<OwnerId>,
    pub max_episodes: usize,
    pub lease_duration_ns: i64,
    pub live_selection: LiveProcessingSelection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingState {
    Running,
    Stopped,
    Incomplete,
    Held,
    Failed,
    Idle,
    Ready,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ProcessingReport {
    pub state: ProcessingState,
    pub import_id: Option<String>,
    pub insomnia: InsomniaProgress,
    pub stages: Vec<StageProgress>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub live_stages: Vec<LiveStageProgress>,
}

pub struct ProcessingJob {
    stop: Arc<AtomicBool>,
    report: Arc<Mutex<ProcessingReport>>,
    worker: Option<JoinHandle<Result<ProcessingReport, GatewayError>>>,
}

impl ProcessingJob {
    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }

    pub fn poll(&self) -> Result<ProcessingReport, GatewayError> {
        self.report
            .lock()
            .map(|report| report.clone())
            .map_err(|_| GatewayError::RegistryPoisoned)
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    pub fn join(mut self) -> Result<ProcessingReport, GatewayError> {
        self.worker
            .take()
            .expect("live processing handle")
            .join()
            .map_err(|_| GatewayError::HostAdapter("owner processing worker panicked".into()))?
    }
}

impl Drop for ProcessingJob {
    fn drop(&mut self) {
        self.request_stop();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

const PROCESSING_LEASE: u8 = 0;

impl Gateway {
    pub fn start_processing(
        &self,
        instance: Arc<Instance>,
        owner: OwnerId,
        endpoint: Option<Arc<dyn GeneralEndpoint>>,
        ownership: Option<Arc<dyn GeneralEndpoint>>,
        dream: Option<Arc<dyn GeneralEndpoint>>,
        entity: Option<Arc<dyn GeneralEndpoint>>,
        embedding: Option<Arc<dyn EmbeddingEndpoint>>,
        route_identity: ProcessingRouteIdentity,
        retry_episode: Option<EpisodeId>,
        options: InsomniaRunOptions,
    ) -> Result<ProcessingJob, GatewayError> {
        if !Arc::ptr_eq(&self.owners, &instance.owners) {
            return Err(GatewayError::HostAdapter(
                "processing Instance belongs to another Gateway".into(),
            ));
        }
        let drains_insomnia =
            options.import_id.is_some() || options.live_selection.drains_insomnia();
        if !(1..=MAX_PROCESS_EPISODES).contains(&options.max_episodes)
            || options.lease_duration_ns <= 0
        {
            return Err(GatewayError::HostAdapter(
                "invalid processing bounds".into(),
            ));
        }
        if options.import_id.is_some() {
            for (available, missing) in [
                (
                    dream.is_some(),
                    "import processing requires a configured Dream route",
                ),
                (
                    entity.is_some(),
                    "import processing requires a configured Entity Resolution route",
                ),
                (
                    embedding.is_some(),
                    "import processing requires a configured embedding route",
                ),
            ] {
                if !available {
                    return Err(GatewayError::RetrievalUnavailable(missing));
                }
            }
        }
        let endpoint = if drains_insomnia {
            Some(endpoint.ok_or(GatewayError::RetrievalUnavailable(
                "owner processing requires a configured Insomnia or general route",
            ))?)
        } else {
            None
        };
        instance.with_writable_reliquary(owner, |_| ())?;
        if options.import_id.is_none() {
            instance.validate_live_processing(owner)?;
        }
        if let Some(phy) = options.destination {
            instance.authorize_routed_owners(owner, phy)?;
            instance.with_entity_phy_writer(phy, |_| ())?;
        }
        if let Some(id) = options.import_id.as_deref() {
            instance.check_import_processing_binding(
                owner,
                id,
                endpoint.as_ref().expect("import needs Insomnia").model(),
                options.destination,
            )?;
        }
        let keys = std::iter::once(owner)
            .chain(options.destination)
            .map(|owner| (owner, PROCESSING_LEASE))
            .collect::<Vec<_>>();
        {
            let mut leases = self
                .owner_worker_leases
                .lock()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            if keys.iter().any(|key| leases.contains(key)) {
                return Err(GatewayError::HostAdapter(
                    "owner processing is already active".into(),
                ));
            }
            leases.extend(keys.iter().copied());
        }
        let lease = ProcessingLease(Arc::clone(&self.owner_worker_leases), keys);
        if let Some(id) = options.import_id.as_deref() {
            instance.with_rel_writer(owner, |writer| {
                writer.bind_native_import_processing(
                    id,
                    import_plan_identity(route_identity.import),
                    options.destination.map(OwnerId::as_bytes),
                )?;
                if let Some(episode) = retry_episode {
                    writer.retry_held_import_episode(id, episode, now_ns())?;
                }
                Ok::<_, crate::OwnerSessionError>(())
            })??;
        } else if let Some(episode) = retry_episode {
            instance.retry_held_live_episode(owner, episode)?;
        }
        let initial = instance.insomnia_progress(owner, options.import_id.as_deref())?;
        let report = Arc::new(Mutex::new(ProcessingReport {
            state: ProcessingState::Running,
            import_id: options.import_id.clone(),
            insomnia: initial,
            stages: options
                .import_id
                .as_ref()
                .map_or_else(Vec::new, |_| import_stages()),
            live_stages: if options.import_id.is_none() {
                options
                    .live_selection
                    .stages()
                    .iter()
                    .copied()
                    .map(LiveStageProgress::new)
                    .collect()
            } else {
                Vec::new()
            },
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_report = Arc::clone(&report);
        let worker = thread::Builder::new()
            .name("reliquary-processing".into())
            .spawn(move || {
                let _lease = lease;
                let result = if let Some(endpoint) = endpoint {
                    instance.drain_insomnia(owner, endpoint, ownership, &options, &thread_stop)
                } else {
                    instance.insomnia_progress(owner, None)
                };
                let mut report = thread_report
                    .lock()
                    .map_err(|_| GatewayError::RegistryPoisoned)?;
                match result {
                    Ok(progress) => {
                        report.state = if thread_stop.load(Ordering::Acquire) {
                            ProcessingState::Stopped
                        } else if progress.held > 0 || progress.processing > 0 {
                            ProcessingState::Held
                        } else if progress.failed > 0 {
                            ProcessingState::Failed
                        } else if progress.pending > 0 || options.import_id.is_some() {
                            ProcessingState::Incomplete
                        } else {
                            ProcessingState::Idle
                        };
                        let postprocess = report.state == ProcessingState::Incomplete
                            && progress.unfinished() == 0
                            && options.import_id.is_some();
                        report.insomnia = progress;
                        if postprocess {
                            report.state = ProcessingState::Running;
                            drop(report);
                            let id = options.import_id.as_deref().expect("postprocessing import");
                            let state = run_import_stages(
                                &instance,
                                owner,
                                options.destination,
                                id,
                                ImportProcessingRoutes {
                                    dream,
                                    entity,
                                    embedding,
                                },
                                &thread_stop,
                                |name, state, completed| {
                                    if let Ok(mut report) = thread_report.lock() {
                                        if let Some(stage) = report
                                            .stages
                                            .iter_mut()
                                            .find(|stage| stage.stage == name)
                                        {
                                            stage.state = state;
                                            stage.completed = completed;
                                        }
                                    }
                                },
                            );
                            let mut report = thread_report
                                .lock()
                                .map_err(|_| GatewayError::RegistryPoisoned)?;
                            match state {
                                Ok(state) => {
                                    report.state = state;
                                    report.insomnia =
                                        instance.insomnia_progress(owner, Some(id))?;
                                    return Ok(report.clone());
                                }
                                Err(error) => {
                                    report.state = ProcessingState::Failed;
                                    return Err(error);
                                }
                            }
                        }
                        if options.import_id.is_none()
                            && report.state != ProcessingState::Stopped
                            && options.live_selection != LiveProcessingSelection::Insomnia
                        {
                            let insomnia_state = report.state;
                            report.state = ProcessingState::Running;
                            drop(report);
                            let result = run_live_stages(
                                &instance,
                                owner,
                                options.destination,
                                ImportProcessingRoutes {
                                    dream,
                                    entity,
                                    embedding,
                                },
                                options.live_selection,
                                options.max_episodes,
                                route_identity.live_dream,
                                &thread_stop,
                                |progress| {
                                    if let Ok(mut report) = thread_report.lock() {
                                        if let Some(stage) = report
                                            .live_stages
                                            .iter_mut()
                                            .find(|stage| stage.stage == progress.stage)
                                        {
                                            *stage = progress;
                                        }
                                    }
                                },
                            );
                            let mut report = thread_report
                                .lock()
                                .map_err(|_| GatewayError::RegistryPoisoned)?;
                            report.state = match result {
                                Ok(state) => {
                                    if drains_insomnia
                                        && insomnia_state != ProcessingState::Idle
                                        && matches!(
                                            state,
                                            ProcessingState::Ready
                                                | ProcessingState::Idle
                                                | ProcessingState::Incomplete
                                        )
                                    {
                                        insomnia_state
                                    } else {
                                        state
                                    }
                                }
                                Err(error) => {
                                    report.state = ProcessingState::Failed;
                                    return Err(error);
                                }
                            };
                            return Ok(report.clone());
                        }
                        Ok(report.clone())
                    }
                    Err(error) => {
                        report.state = ProcessingState::Failed;
                        Err(error)
                    }
                }
            });
        match worker {
            Ok(worker) => Ok(ProcessingJob {
                stop,
                report,
                worker: Some(worker),
            }),
            Err(error) => Err(GatewayError::HostAdapter(error.to_string())),
        }
    }
}

struct ProcessingLease(
    Arc<Mutex<std::collections::BTreeSet<(OwnerId, u8)>>>,
    Vec<(OwnerId, u8)>,
);

impl Drop for ProcessingLease {
    fn drop(&mut self) {
        if let Ok(mut leases) = self.0.lock() {
            for key in &self.1 {
                leases.remove(key);
            }
        }
    }
}

impl Instance {
    fn check_import_processing_binding(
        &self,
        owner: OwnerId,
        import_id: &str,
        model: &str,
        phy: Option<OwnerId>,
    ) -> Result<(), GatewayError> {
        Ok(self.with_writable_reliquary(owner, |rel| {
            let destination = phy.map(OwnerId::as_bytes);
            rel.validate_import_processing_binding(import_id, model, destination, |payload| {
                reliquary_application::NativeInsomniaExtraction::import_checkpoint_matches(
                    payload,
                    model,
                    destination,
                )
            })
        })??)
    }

    pub fn insomnia_progress(
        &self,
        owner: OwnerId,
        import_id: Option<&str>,
    ) -> Result<InsomniaProgress, GatewayError> {
        Ok(self.with_reliquary(owner, |rel| {
            inspect_insomnia_work(rel, import_id, now_ns(), 0).map(|(progress, _)| progress)
        })??)
    }

    pub fn retry_held_import_episode(
        &self,
        owner: OwnerId,
        import_id: &str,
        episode: EpisodeId,
    ) -> Result<(), GatewayError> {
        Ok(self.with_rel_writer(owner, |writer| {
            writer.retry_held_import_episode(import_id, episode, now_ns())
        })??)
    }

    pub fn retry_held_live_episode(
        &self,
        owner: OwnerId,
        episode: EpisodeId,
    ) -> Result<(), GatewayError> {
        Ok(self.with_rel_writer(owner, |writer| {
            writer.retry_held_live_episode(episode, now_ns())
        })??)
    }

    /// Execute a bounded amount of work using the same claim and publication
    /// path as the background worker. No provider call holds an owner lock.
    pub fn drain_insomnia(
        &self,
        owner: OwnerId,
        endpoint: Arc<dyn GeneralEndpoint>,
        ownership: Option<Arc<dyn GeneralEndpoint>>,
        options: &InsomniaRunOptions,
        stop: &AtomicBool,
    ) -> Result<InsomniaProgress, GatewayError> {
        if !(1..=MAX_PROCESS_EPISODES).contains(&options.max_episodes)
            || options.lease_duration_ns <= 0
        {
            return Err(GatewayError::HostAdapter(
                "invalid processing bounds".into(),
            ));
        }
        self.with_writable_reliquary(owner, |_| ())?;
        if let Some(phy) = options.destination {
            self.authorize_routed_owners(owner, phy)?;
            self.with_entity_phy_writer(phy, |_| ())?;
        }
        let import_id = options.import_id.as_deref();
        let endpoint = SharedEndpoint(endpoint);
        let ownership = ownership.map(SharedEndpoint);
        let worker = format!("owner-process-{}", uuid::Uuid::new_v4());
        if let Some(id) = import_id {
            self.check_import_processing_binding(owner, id, endpoint.model(), options.destination)?;
            let (progress, due) = self.with_writable_reliquary(owner, |rel| {
                inspect_insomnia_work(rel, import_id, now_ns(), options.max_episodes)
            })??;
            if progress.import_complete {
                return Ok(progress);
            }
            for episode in due {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                let claim = self.with_rel_writer(owner, |writer| {
                    writer.claim_native_import_episode(
                        id,
                        episode,
                        &worker,
                        now_ns(),
                        options.lease_duration_ns,
                    )
                })??;
                process_claim(
                    self,
                    owner,
                    &endpoint,
                    options.destination,
                    claim,
                    import_id,
                    ownership.as_ref(),
                )?;
            }
        } else {
            for _ in 0..options.max_episodes {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                let Some(claim) = self.claim_next_insomnia_input(
                    owner,
                    &worker,
                    now_ns(),
                    options.lease_duration_ns,
                    16,
                )?
                else {
                    break;
                };
                process_claim(
                    self,
                    owner,
                    &endpoint,
                    options.destination,
                    claim,
                    None,
                    ownership.as_ref(),
                )?;
            }
        }
        self.insomnia_progress(owner, import_id)
    }
}
