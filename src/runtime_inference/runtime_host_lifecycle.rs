use super::{
    OwnerExecution, ReliquaryRuntimeHost, ReliquaryRuntimeHostError, ReliquaryRuntimeRoutes,
    gateway::mount::MountClaim, notify_work, owner_key,
};
use crate::{EpisodePolicy, InsomniaWorkerConfig, InteractionRuntime, Phylactery};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

impl ReliquaryRuntimeHost {
    pub fn start(
        runtime: InteractionRuntime,
        routes: ReliquaryRuntimeRoutes,
        config: InsomniaWorkerConfig,
        episode_policy: EpisodePolicy,
    ) -> Result<Self, ReliquaryRuntimeHostError> {
        Self::start_inner(runtime, None, routes, config, episode_policy, true)
    }

    pub fn start_inactive(
        runtime: InteractionRuntime,
        routes: ReliquaryRuntimeRoutes,
        config: InsomniaWorkerConfig,
        episode_policy: EpisodePolicy,
    ) -> Result<Self, ReliquaryRuntimeHostError> {
        Self::start_inner(runtime, None, routes, config, episode_policy, false)
    }

    pub fn start_with_phylactery(
        runtime: InteractionRuntime,
        phylactery: Phylactery,
        routes: ReliquaryRuntimeRoutes,
        config: InsomniaWorkerConfig,
        episode_policy: EpisodePolicy,
    ) -> Result<Self, ReliquaryRuntimeHostError> {
        Self::start_inner(
            runtime,
            Some(phylactery),
            routes,
            config,
            episode_policy,
            true,
        )
    }

    pub fn new(
        routes: ReliquaryRuntimeRoutes,
        config: InsomniaWorkerConfig,
        episode_policy: EpisodePolicy,
    ) -> Self {
        Self {
            executions: BTreeMap::new(),
            legacy_rel_claims: BTreeMap::new(),
            legacy_phy_claim: Mutex::new(None),
            active_key: None,
            routes: Arc::new(RwLock::new(routes)),
            phylactery: Arc::new(Mutex::new(None)),
            config,
            episode_policy,
            active_insomnia_enabled: AtomicBool::new(true),
            managed_sessions: Mutex::new(BTreeMap::new()),
        }
    }

    fn start_inner(
        runtime: InteractionRuntime,
        phylactery: Option<Phylactery>,
        routes: ReliquaryRuntimeRoutes,
        config: InsomniaWorkerConfig,
        episode_policy: EpisodePolicy,
        insomnia_enabled: bool,
    ) -> Result<Self, ReliquaryRuntimeHostError> {
        let rel_claim = MountClaim::acquire_legacy_rel(&runtime.cva().container)?;
        let phy_claim = phylactery
            .as_ref()
            .map(|phy| MountClaim::acquire_legacy_phy(&phy.container))
            .transpose()?;
        let owner_id = runtime.cva().owner_id();
        let key = owner_key(owner_id.as_deref());
        let routes = Arc::new(RwLock::new(routes));
        let phylactery = Arc::new(Mutex::new(phylactery));
        let execution = OwnerExecution::start(
            runtime,
            Arc::clone(&routes),
            Arc::clone(&phylactery),
            config.clone(),
            episode_policy,
            insomnia_enabled,
            true,
        );
        let mut executions = BTreeMap::new();
        executions.insert(key.clone(), execution);
        Ok(Self {
            executions,
            legacy_rel_claims: BTreeMap::from([(key.clone(), rel_claim)]),
            legacy_phy_claim: Mutex::new(phy_claim),
            active_key: Some(key),
            routes,
            phylactery,
            config,
            episode_policy,
            active_insomnia_enabled: AtomicBool::new(insomnia_enabled),
            managed_sessions: Mutex::new(BTreeMap::new()),
        })
    }

    pub fn set_routes(
        &self,
        routes: ReliquaryRuntimeRoutes,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        *self
            .routes
            .write()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)? = routes;
        for execution in self.executions.values() {
            execution
                .insomnia_backpressure_until_ns
                .store(0, Ordering::SeqCst);
            notify_work(&execution.signal)?;
        }
        Ok(())
    }

    pub fn wake(&self) -> Result<(), ReliquaryRuntimeHostError> {
        notify_work(&self.active_execution()?.signal)
    }

    pub fn set_insomnia_enabled(&self, enabled: bool) -> Result<(), ReliquaryRuntimeHostError> {
        self.active_insomnia_enabled
            .store(enabled, Ordering::SeqCst);
        let execution = self.active_execution()?;
        execution.insomnia_enabled.store(enabled, Ordering::SeqCst);
        notify_work(&execution.signal)
    }

    pub fn into_cva(self) -> Result<crate::Cva, ReliquaryRuntimeHostError> {
        self.into_cva_and_phylactery().map(|(cva, _)| cva)
    }

    pub fn into_cva_and_phylactery(
        mut self,
    ) -> Result<(crate::Cva, Option<Phylactery>), ReliquaryRuntimeHostError> {
        if self.executions.len() != 1 {
            return Err(ReliquaryRuntimeHostError::Operation(
                "single-CVA extraction requires exactly one mounted REL".into(),
            ));
        }
        self.stop_workers()?;
        let phylactery = self
            .phylactery
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .take();
        let key = self.executions.keys().next().cloned().ok_or_else(|| {
            ReliquaryRuntimeHostError::Operation("Reliquary runtime is unavailable".into())
        })?;
        let execution = self.executions.remove(&key).ok_or_else(|| {
            ReliquaryRuntimeHostError::Operation("Reliquary runtime is unavailable".into())
        })?;
        Ok((execution.into_cva()?, phylactery))
    }

    fn stop_workers(&mut self) -> Result<(), ReliquaryRuntimeHostError> {
        let mut failure = None;
        for execution in self.executions.values_mut() {
            if let Err(error) = execution.stop_workers()
                && failure.is_none()
            {
                failure = Some(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }
}

impl Drop for ReliquaryRuntimeHost {
    fn drop(&mut self) {
        let _ = self.stop_workers();
    }
}
