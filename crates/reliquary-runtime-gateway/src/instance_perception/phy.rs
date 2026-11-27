//! Active owner admission and scheduling for PHY Perception.

use super::*;
use crate::owner_workers::{OwnerWorkerDriver, OwnerWorkerPoll};

mod fence;
mod poll;
mod progress;

pub struct PhyPerceptionWorker {
    admission: Arc<Admission>,
    coordinator: OwnerWorkerCoordinator,
}

impl PhyPerceptionWorker {
    pub fn stop(&mut self) -> Result<(), GatewayError> {
        self.admission.active.store(false, Ordering::Release);
        self.coordinator.stop()
    }
}

impl Drop for PhyPerceptionWorker {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub(super) struct Admission {
    instance: Arc<Instance>,
    owner: OwnerId,
    mount: Weak<MountedPhyWriter>,
    active: AtomicBool,
}

impl Admission {
    fn check_mount(&self) -> Result<(), GatewayError> {
        if !self.active.load(Ordering::Acquire)
            || !self.mount.ptr_eq(&Arc::downgrade(
                &self.instance.ego_phy_writer_mount(self.owner)?,
            ))
        {
            return Err(OwnerSessionError::ManagementStale.into());
        }
        Ok(())
    }
}

impl Gateway {
    pub fn start_phy_perception(
        &self,
        lifecycle: &PerceptionLifecycle,
        instance: Arc<Instance>,
        owner: OwnerId,
    ) -> Result<PhyPerceptionWorker, GatewayError> {
        if !lifecycle
            .control
            .topology
            .ptr_eq(&Arc::downgrade(&self.owners))
            || !Arc::ptr_eq(&instance.owners, &self.owners)
        {
            return Err(GatewayError::HostAdapter(
                "PHY Perception admission belongs to another Gateway".into(),
            ));
        }
        let mount = instance.ego_phy_writer_mount(owner)?;
        let admission = Arc::new(Admission {
            instance: instance.clone(),
            owner,
            mount: Arc::downgrade(&mount),
            active: AtomicBool::new(true),
        });
        {
            let mut active_phy = lifecycle
                .control
                .active_phy
                .lock()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            if active_phy
                .upgrade()
                .is_some_and(|current| current.active.load(Ordering::Acquire))
            {
                return Err(OwnerSessionError::ManagementConflict.into());
            }
            *active_phy = Arc::downgrade(&admission);
        }
        let driver: Arc<dyn OwnerWorkerDriver> = Arc::new(Driver {
            admission: admission.clone(),
            control: Arc::downgrade(&lifecycle.control),
            state: Mutex::new(DriverState::default()),
        });
        let coordinator =
            self.start_owner_workers(instance, owner, [driver], OwnerWorkerConfig::default())?;
        Ok(PhyPerceptionWorker {
            admission,
            coordinator,
        })
    }
}

struct Driver {
    admission: Arc<Admission>,
    control: Weak<Control>,
    state: Mutex<DriverState>,
}

#[derive(Default)]
struct DriverState {
    route: Weak<PerceptionRoutes>,
    work: poll::State,
    failures: usize,
    retry_at: Option<Instant>,
}

impl OwnerWorkerDriver for Driver {
    fn kind(&self) -> OwnerWorkerKind {
        OwnerWorkerKind::Perception
    }

    fn poll(&self) -> Result<OwnerWorkerPoll, String> {
        let Some(control) = self.control.upgrade() else {
            return Ok(OwnerWorkerPoll::Idle);
        };
        if control.stopping.load(Ordering::Acquire) || self.admission.check_mount().is_err() {
            return Ok(OwnerWorkerPoll::Idle);
        }
        let Some(routes) = control
            .routes
            .read()
            .map_err(|_| "Perception routes are unavailable")?
            .clone()
        else {
            return Ok(OwnerWorkerPoll::Idle);
        };
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Perception work is unavailable")?;
        let route = Arc::downgrade(&routes);
        if !state.route.ptr_eq(&route) {
            *state = DriverState {
                route,
                ..DriverState::default()
            };
        }
        if state.retry_at.is_some_and(|retry| Instant::now() < retry) {
            return Ok(OwnerWorkerPoll::Idle);
        }
        let scope = PhyWork {
            admission: &self.admission,
            control: &control,
            routes: &routes,
        };
        match poll::tick(&scope, &mut state.work) {
            Ok(changed) => {
                state.failures = 0;
                state.retry_at = None;
                Ok(if changed {
                    OwnerWorkerPoll::Progress
                } else {
                    OwnerWorkerPoll::Idle
                })
            }
            Err(error) => {
                let delay = RETRY_INTERVALS[state.failures.min(RETRY_INTERVALS.len() - 1)];
                state.failures = state.failures.saturating_add(1);
                state.retry_at = Some(Instant::now() + delay);
                Err(error.to_string())
            }
        }
    }
}

pub(super) struct PhyWork<'a> {
    admission: &'a Arc<Admission>,
    control: &'a Arc<Control>,
    routes: &'a Arc<PerceptionRoutes>,
}

impl PhyWork<'_> {
    fn route_guard(
        &self,
    ) -> Result<std::sync::RwLockReadGuard<'_, Option<Arc<PerceptionRoutes>>>, GatewayError> {
        self.admission.check_mount()?;
        let routes = self
            .control
            .routes
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        if self.control.stopping.load(Ordering::Acquire)
            || routes
                .as_ref()
                .is_none_or(|live| !Arc::ptr_eq(live, self.routes))
        {
            return Err(OwnerSessionError::ManagementStale.into());
        }
        Ok(routes)
    }

    pub(super) fn read<T>(&self, action: impl FnOnce(&PhyWriter) -> T) -> Result<T, GatewayError> {
        self.with_writer(|writer| Ok(action(writer)))
    }

    pub(super) fn with_writer<T>(
        &self,
        action: impl FnOnce(&mut PhyWriter) -> Result<T, GatewayError>,
    ) -> Result<T, GatewayError> {
        self.admission
            .instance
            .with_ego_phy_writer(self.admission.owner, |writer| {
                Ok(self.route_guard().and_then(|_routes| action(writer)))
            })?
    }
}
