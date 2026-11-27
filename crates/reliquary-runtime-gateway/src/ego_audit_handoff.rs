//! Scoped handoff from managed REL discovery into each authorized PHY profile.

use super::*;
use crate::instance_owner::RelMount;
use reliquary_application::ego_pending::{EgoBatchKey, EgoPendingWork, EgoRun, EgoRunKind};
use reliquary_application::perception_workflow::{
    AUDIT_CONTRACT_VERSION, EgoCandidate, audit_owner_cut,
};
use reliquary_owner_session::{PerceptionAuditProgress, PerceptionSourceCut};
use std::{
    path::{Path, PathBuf},
    sync::Weak,
};

#[derive(Default)]
pub(crate) struct EgoAuditRegistry {
    admissions: Mutex<HashMap<Uuid, Arc<Admission>>>,
}

struct Admission {
    instance: Weak<Instance>,
    phy: OwnerId,
    mount: Weak<MountedPhyWriter>,
    sources: BTreeSet<OwnerId>,
    path: PathBuf,
    active: AtomicBool,
}

pub(crate) struct EgoAuditRegistration {
    registry: Weak<EgoAuditRegistry>,
    id: Uuid,
    admission: Arc<Admission>,
}

impl Drop for EgoAuditRegistration {
    fn drop(&mut self) {
        self.admission.active.store(false, Ordering::Release);
        if let Some(registry) = self.registry.upgrade()
            && let Ok(mut admissions) = registry.admissions.lock()
        {
            admissions.remove(&self.id);
        }
    }
}

impl EgoAuditRegistry {
    pub(crate) fn admit(
        self: &Arc<Self>,
        instance: &Arc<Instance>,
        phy: OwnerId,
        sources: &[OwnerId],
        pending_path: &Path,
    ) -> Result<EgoAuditRegistration, GatewayError> {
        instance.ensure_credential_active()?;
        authorize_phy(instance, phy)?;
        let owners = instance
            .owners
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let mount = match owners.get(&phy) {
            Some(MountedOwner::PhyWriter(mount)) => Arc::downgrade(mount),
            Some(MountedOwner::Phy(_)) => return Err(GatewayError::WriterUnavailable),
            Some(_) => return Err(GatewayError::WrongOwnerKind(phy)),
            None => return Err(GatewayError::UnknownOwner(phy)),
        };
        for &source in sources {
            instance.authorize_rel_access(source)?;
            match owners.get(&source) {
                Some(owner) if owner.is_rel() => {}
                Some(_) => return Err(GatewayError::WrongOwnerKind(source)),
                None => return Err(GatewayError::UnknownOwner(source)),
            }
        }
        drop(owners);
        let id = Uuid::new_v4();
        let admission = Arc::new(Admission {
            instance: Arc::downgrade(instance),
            phy,
            mount,
            sources: sources.iter().copied().collect(),
            path: pending_path.to_owned(),
            active: AtomicBool::new(true),
        });
        self.admissions
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?
            .insert(id, admission.clone());
        Ok(EgoAuditRegistration {
            registry: Arc::downgrade(self),
            id,
            admission,
        })
    }

    pub(crate) fn targets<'a>(
        &self,
        instance: &'a Instance,
        source: OwnerId,
        mount: &'a RelMount,
    ) -> Result<EgoAuditTargets<'a>, GatewayError> {
        let mut admissions = self
            .admissions
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?
            .values()
            .filter(|entry| entry.sources.contains(&source))
            .filter_map(|entry| {
                let authorized = entry.instance.upgrade()?;
                (Arc::ptr_eq(&instance.owners, &authorized.owners)
                    && entry.is_authorized(&authorized))
                .then_some(entry.clone())
            })
            .collect::<Vec<_>>();
        admissions.sort_unstable_by(|first, second| {
            (first.phy, &first.path).cmp(&(second.phy, &second.path))
        });
        Ok(EgoAuditTargets {
            instance,
            source,
            mount,
            admissions,
        })
    }
}

fn authorize_phy(instance: &Instance, phy: OwnerId) -> Result<(), GatewayError> {
    if instance.phy_grants.contains(&phy) {
        return Ok(());
    }
    Err(GatewayError::UnauthorizedPhy {
        principal: instance.principal.to_string(),
        owner: phy,
    })
}

impl Admission {
    fn is_authorized(&self, instance: &Instance) -> bool {
        self.active.load(Ordering::Acquire) && instance.ensure_credential_active().is_ok()
    }
}

pub(crate) struct EgoAuditTargets<'a> {
    instance: &'a Instance,
    source: OwnerId,
    mount: &'a RelMount,
    admissions: Vec<Arc<Admission>>,
}

impl EgoAuditTargets<'_> {
    pub(crate) fn is_empty(&self) -> bool {
        self.admissions.is_empty()
    }

    /// Reuse the first accepted batch after a restart or a partial fanout.
    pub(crate) fn cached(&self, run: EgoRun, key: EgoBatchKey) -> Option<Vec<EgoCandidate>> {
        self.admissions.iter().find_map(|target| {
            self.with_pending(target, |db, phy| {
                db.accepted_batch(EgoRun { phy, ..run }, key)
            })
            .ok()
            .flatten()
            .flatten()
            .map(|record| record.candidates)
        })
    }

    pub(crate) fn persist(
        &self,
        run: EgoRun,
        key: EgoBatchKey,
        candidates: &[EgoCandidate],
    ) -> bool {
        let mut succeeded = true;
        for target in &self.admissions {
            let result = self.with_pending(target, |db, phy| {
                db.accept_batch(EgoRun { phy, ..run }, key, candidates)
            });
            succeeded &= result.is_ok();
        }
        succeeded
    }

    fn with_pending<T>(
        &self,
        target: &Admission,
        operation: impl FnOnce(
            &mut EgoPendingWork,
            [u8; 16],
        )
            -> Result<T, reliquary_application::ego_pending::EgoPendingError>,
    ) -> Result<Option<T>, GatewayError> {
        let Some(instance) = target.instance.upgrade() else {
            return Ok(None);
        };
        let _topology = instance
            .rel_topology
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let _grants = instance
            .credential_publication
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        if !target.is_authorized(&instance)
            || instance.authorize_rel_access(self.source).is_err()
            || authorize_phy(&instance, target.phy).is_err()
        {
            return Ok(None);
        }
        self.instance.authorize_rel_access(self.source)?;
        if RelMount::capture(self.instance, self.source)? != *self.mount {
            return Err(OwnerSessionError::ManagementStale.into());
        }
        let owners = instance
            .owners
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let current_phy = match owners.get(&target.phy) {
            Some(MountedOwner::PhyWriter(mount)) => {
                Weak::ptr_eq(&target.mount, &Arc::downgrade(mount))
            }
            _ => false,
        };
        drop(owners);
        if !current_phy {
            return Ok(None);
        }
        let storage_error =
            |_| GatewayError::RetrievalUnavailable("Ego pending-work database unavailable");
        let mut db = EgoPendingWork::open(&target.path).map_err(storage_error)?;
        operation(&mut db, target.phy.as_bytes())
            .map(Some)
            .map_err(storage_error)
    }
}

pub(crate) fn shared_ego_run(
    cut: &PerceptionSourceCut,
    bootstrap: bool,
    route: [u8; 32],
) -> EgoRun {
    let mut source = audit_owner_cut(cut);
    source.frontier = cut.settlements;
    EgoRun {
        phy: [0; 16],
        source,
        kind: if bootstrap {
            EgoRunKind::RelSharedBootstrap
        } else {
            EgoRunKind::RelRecurring
        },
        memory_version: cut.memory,
        route,
        contract: AUDIT_CONTRACT_VERSION,
    }
}

/// Reconcile a closed shared run after a restart using the durable v1 cursor.
pub(crate) fn shared_ego_run_closed(
    rel: &RelSession,
    run: EgoRun,
) -> Result<bool, OwnerSessionError> {
    let bootstrap = match run.kind {
        EgoRunKind::RelSharedBootstrap => true,
        EgoRunKind::RelRecurring => false,
        _ => return Ok(false),
    };
    let Some(row) = rel.observation_progress() else {
        return Ok(false);
    };
    let progress = PerceptionAuditProgress::decode(&row.bytes)?;
    Ok(progress.shared_run_closed(
        bootstrap,
        run.source.owner_id,
        run.source.history_generation,
        run.source.frontier,
        run.memory_version,
    ))
}
