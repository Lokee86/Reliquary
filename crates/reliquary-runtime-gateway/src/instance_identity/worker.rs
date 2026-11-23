use super::*;
use crate::instance_owner::RelMount;
use reliquary_switchboard::GeneralEndpoint;
use std::{
    collections::BTreeMap,
    ops::Bound::{Excluded, Unbounded},
    sync::{
        Condvar, RwLockReadGuard, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const BATCH_SIZE: usize = 8;
const BATCH_PAUSE: Duration = Duration::from_millis(25);
const IDLE_PAUSE: Duration = Duration::from_millis(250);
const RETRY_PAUSES: [Duration; 3] = [
    Duration::from_secs(2),
    Duration::from_secs(10),
    Duration::from_secs(60),
];

#[derive(Default)]
pub(super) struct IdentityRoute {
    pub(super) endpoint: Option<Arc<dyn GeneralEndpoint>>,
    pub(super) id: String,
}

#[derive(Default)]
pub(super) struct RouteBinding(RwLock<Arc<IdentityRoute>>);

impl RouteBinding {
    fn snapshot(&self) -> Result<Arc<IdentityRoute>, GatewayError> {
        self.0
            .read()
            .map(|route| Arc::clone(&route))
            .map_err(|_| GatewayError::RegistryPoisoned)
    }

    pub(super) fn fence(
        &self,
        route: &IdentityRoute,
    ) -> Result<RwLockReadGuard<'_, Arc<IdentityRoute>>, GatewayError> {
        let binding = self.0.read().map_err(|_| GatewayError::RegistryPoisoned)?;
        if !std::ptr::eq(binding.as_ref(), route) {
            return Err(stale());
        }
        Ok(binding)
    }
}

struct Admission {
    instance: Weak<Instance>,
    recording: Vec<OwnerId>,
}

pub(super) struct BackgroundAdmission<'a> {
    instance: &'a Weak<Instance>,
    stopping: &'a AtomicBool,
}

impl BackgroundAdmission<'_> {
    pub(super) fn pin(&self) -> Result<Arc<Instance>, GatewayError> {
        if self.stopping.load(Ordering::Acquire) {
            return Err(stale());
        }
        self.instance.upgrade().ok_or_else(stale)
    }
}

#[derive(Default)]
struct Control {
    admissions: Mutex<Vec<Admission>>,
    wake: Condvar,
    stopping: AtomicBool,
    route: RouteBinding,
}

/// Schedules reconsideration through the credentials of live managed sessions.
/// Root supplies only recording owners that have the product's ManageRel grant.
#[derive(Default)]
pub struct IdentityLifecycle {
    control: Arc<Control>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl IdentityLifecycle {
    pub fn admit(
        &self,
        instance: &Arc<Instance>,
        mut recording: Vec<OwnerId>,
    ) -> Result<(), GatewayError> {
        if recording.is_empty() || instance.rel_grants.len() < 2 {
            return Ok(());
        }
        for &owner in &recording {
            instance.authorize_rel_access(owner)?;
        }
        recording.sort_unstable();
        recording.dedup();
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let mut admissions = self
            .control
            .admissions
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        if worker.is_none() {
            let control = Arc::clone(&self.control);
            *worker = Some(
                thread::Builder::new()
                    .name("reliquary-entity-identity".into())
                    .spawn(move || run(control))
                    .map_err(|error| GatewayError::HostAdapter(error.to_string()))?,
            );
        }
        let instance = Arc::downgrade(instance);
        if let Some(existing) = admissions.iter_mut().find(|a| a.instance.ptr_eq(&instance)) {
            existing.recording = recording;
        } else {
            admissions.push(Admission {
                instance,
                recording,
            });
        }
        self.control.wake.notify_one();
        Ok(())
    }

    pub fn set_route(
        &self,
        endpoint: Option<Arc<dyn GeneralEndpoint>>,
        route: String,
    ) -> Result<(), GatewayError> {
        let previous = {
            let mut binding = self
                .control
                .route
                .0
                .write()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            if binding.id == route
                && match (&binding.endpoint, &endpoint) {
                    (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                    (None, None) => true,
                    _ => false,
                }
            {
                return Ok(());
            }
            std::mem::replace(
                &mut *binding,
                Arc::new(IdentityRoute {
                    endpoint,
                    id: route,
                }),
            )
        };
        self.control.wake.notify_one();
        drop(previous);
        Ok(())
    }

    pub fn compare(
        &self,
        instance: &Instance,
        pair: IdentityPair,
    ) -> Result<IdentityReceipt, GatewayError> {
        let binding = &self.control.route;
        let route = binding.snapshot()?;
        instance.compare_entity_identity(pair, binding, &route, None, |_| Ok(()))
    }
}

impl Drop for IdentityLifecycle {
    fn drop(&mut self) {
        self.control.stopping.store(true, Ordering::Release);
        if let Ok(_admissions) = self.control.admissions.lock() {
            self.control.wake.notify_one();
        }
        let worker = self
            .worker
            .get_mut()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(worker) = worker.take()
            && worker.thread().id() != thread::current().id()
        {
            let _ = worker.join();
        }
    }
}

#[derive(Default)]
struct OwnerCursor {
    mount: Option<RelMount>,
    after: Option<IdentityPair>,
}

struct PairCheck {
    stamp: PairStamp,
    revision: u64,
    credential: Uuid,
    fingerprint: Option<[u8; 32]>,
    retry_at: Option<Instant>,
    retry_count: usize,
}

impl PairCheck {
    fn settled_for_now(&self) -> bool {
        self.retry_at.is_none_or(|at| Instant::now() < at)
    }
}

#[derive(Default)]
struct Sweep {
    after_owner: Option<OwnerId>,
    cursors: BTreeMap<OwnerId, OwnerCursor>,
    checked: BTreeMap<IdentityPair, PairCheck>,
    route: Weak<IdentityRoute>,
}

fn run(control: Arc<Control>) {
    let mut sweep = Sweep::default();
    while !control.stopping.load(Ordering::Acquire) {
        let Some(more) = sweep.poll(&control) else {
            break;
        };
        let Ok(admissions) = control.admissions.lock() else {
            break;
        };
        if admissions.is_empty() {
            if control
                .wake
                .wait_while(admissions, |admissions| {
                    admissions.is_empty() && !control.stopping.load(Ordering::Acquire)
                })
                .is_err()
            {
                break;
            }
            continue;
        }
        if control.stopping.load(Ordering::Acquire) {
            break;
        }
        let pause = if more { BATCH_PAUSE } else { IDLE_PAUSE };
        if control.wake.wait_timeout(admissions, pause).is_err() {
            break;
        }
    }
}

impl Sweep {
    fn poll(&mut self, control: &Control) -> Option<bool> {
        let mut owners = BTreeMap::<OwnerId, Vec<Weak<Instance>>>::new();
        {
            let mut admissions = control.admissions.lock().ok()?;
            admissions.retain(|admission| {
                let Some(instance) = admission.instance.upgrade() else {
                    return false;
                };
                if instance.ensure_credential_active().is_err() {
                    return false;
                }
                for &owner in &admission.recording {
                    owners
                        .entry(owner)
                        .or_default()
                        .push(Weak::clone(&admission.instance));
                }
                true
            });
        }
        let previous_owners = self.cursors.len();
        self.cursors.retain(|owner, _| owners.contains_key(owner));
        // Recording owners are dropped from the cache when their last managed
        // admission expires. A changed mount is invalidated at the pair stamp.
        if self.cursors.len() != previous_owners {
            self.checked
                .retain(|pair, _| owners.contains_key(&OwnerId(pair.local_owner)));
        }
        let next = self
            .after_owner
            .and_then(|owner| owners.range((Excluded(owner), Unbounded)).next())
            .or_else(|| owners.first_key_value());
        let Some((&owner, candidates)) = next else {
            self.after_owner = None;
            self.checked.clear();
            return Some(false);
        };
        let more_owners = owners.range((Excluded(owner), Unbounded)).next().is_some();
        self.after_owner = Some(owner);

        let route = control.route.snapshot().ok()?;
        let route_ref = Arc::downgrade(&route);
        if !self.route.ptr_eq(&route_ref) {
            self.checked.clear();
            self.route = route_ref;
        }
        let cursor = self.cursors.entry(owner).or_default();
        let Some(keys) = candidates
            .iter()
            .find_map(|candidate| Self::recorded_pairs(candidate, owner, cursor))
        else {
            return Some(more_owners);
        };
        cursor.after = if keys.len() == BATCH_SIZE {
            keys.last().copied()
        } else {
            None
        };
        for &pair in &keys {
            if control.stopping.load(Ordering::Acquire) || control.route.fence(&route).is_err() {
                break;
            }
            self.check_pair(control, &route, candidates, pair);
        }
        Some(more_owners || keys.len() == BATCH_SIZE)
    }

    fn recorded_pairs(
        candidate: &Weak<Instance>,
        owner: OwnerId,
        cursor: &mut OwnerCursor,
    ) -> Option<Vec<IdentityPair>> {
        let instance = candidate.upgrade()?;
        let _topology = instance.rel_topology.read().ok()?;
        let mount = RelMount::capture(&instance, owner).ok()?;
        if !matches!(mount, RelMount::Write(_)) {
            return None;
        }
        if cursor.mount.as_ref() != Some(&mount) {
            cursor.after = None;
            cursor.mount = Some(mount);
        }
        instance
            .with_reliquary(owner, |session| {
                session.identity_pairs_after(cursor.after.as_ref(), BATCH_SIZE)
            })
            .ok()
    }

    fn check_pair(
        &mut self,
        control: &Control,
        route: &IdentityRoute,
        candidates: &[Weak<Instance>],
        pair: IdentityPair,
    ) {
        let peer = OwnerId(pair.peer_owner);
        let Some((admission, instance)) = candidates.iter().find_map(|admission| {
            let instance = admission.upgrade()?;
            instance.authorize_rel_access(peer).ok()?;
            // Retain the credential's revocation fence, but release the upgraded
            // admission before inference so expiry can fence publication.
            Some((admission, Instance::clone(&instance)))
        }) else {
            self.checked.remove(&pair);
            return;
        };
        let Ok(Some((stamp, revision))) =
            instance.with_identity_pair(&pair, true, |left, _, stamp| {
                Ok(left
                    .session()
                    .identity_receipt(&pair, None)
                    .map(|r| (stamp, r.revision)))
            })
        else {
            self.checked.remove(&pair);
            return;
        };
        let previous = self.checked.get(&pair).filter(|check| {
            check.stamp.mounts == stamp.mounts
                && check.revision == revision
                && check.credential == instance.instance_id
        });
        if previous.is_some_and(|check| check.stamp == stamp && check.settled_for_now()) {
            return;
        }
        let admission = BackgroundAdmission {
            instance: admission,
            stopping: &control.stopping,
        };
        let mut check = PairCheck {
            stamp,
            revision,
            credential: instance.instance_id,
            fingerprint: None,
            retry_at: None,
            retry_count: 0,
        };
        let mut retained_failure = false;
        let result = instance.compare_entity_identity(
            pair,
            &control.route,
            route,
            Some(admission),
            |fingerprint| {
                check.fingerprint = Some(fingerprint);
                if let Some(previous) = previous.filter(|p| p.fingerprint == check.fingerprint) {
                    check.retry_count = previous.retry_count;
                    if previous.settled_for_now() {
                        check.retry_at = previous.retry_at;
                        retained_failure = true;
                        return Err(stale());
                    }
                }
                Ok(())
            },
        );
        match result {
            Ok(receipt) => check.revision = receipt.revision,
            Err(GatewayError::Management(ManagementError::Unavailable(_)))
                if route.endpoint.is_some() =>
            {
                check.retry_at = Some(Instant::now() + RETRY_PAUSES[check.retry_count]);
                check.retry_count = (check.retry_count + 1).min(RETRY_PAUSES.len() - 1);
            }
            // Another credential, a changed mount or the next stable cut can
            // resolve these races without waiting for transport backoff.
            Err(
                GatewayError::CredentialRevoked | GatewayError::Management(ManagementError::Stale),
            ) if !retained_failure => return,
            Err(_) => {}
        }
        self.checked.insert(pair, check);
    }
}
