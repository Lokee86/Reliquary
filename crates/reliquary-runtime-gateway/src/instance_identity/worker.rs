use super::*;
use crate::owner_workers::{OwnerWorkerDriver, OwnerWorkerPoll};
use std::collections::BTreeMap;

const BATCH_SIZE: usize = 8;

#[derive(Default)]
struct Cursor {
    after: Option<IdentityPair>,
    mount: Option<crate::instance_owner::RelMount>,
    seen: BTreeMap<IdentityPair, (PairStamp, u64)>,
}

struct IdentityWorker {
    instance: Arc<Instance>,
    owner: OwnerId,
    endpoint: Option<Arc<dyn GeneralEndpoint>>,
    route: String,
    cursor: Mutex<Cursor>,
}

impl Gateway {
    pub fn start_entity_identity_processing(
        &self,
        instance: Arc<Instance>,
        owner: OwnerId,
        endpoint: Option<Arc<dyn GeneralEndpoint>>,
        route: String,
    ) -> Result<OwnerWorkerCoordinator, GatewayError> {
        let driver: Arc<dyn OwnerWorkerDriver> = Arc::new(IdentityWorker {
            instance: Arc::clone(&instance),
            owner,
            endpoint,
            route,
            cursor: Mutex::default(),
        });
        self.start_owner_workers(instance, owner, [driver], OwnerWorkerConfig::default())
    }
}

impl OwnerWorkerDriver for IdentityWorker {
    fn kind(&self) -> OwnerWorkerKind {
        OwnerWorkerKind::EntityIdentity
    }

    fn poll(&self) -> Result<OwnerWorkerPoll, String> {
        self.poll_pairs()
            .map_err(|_| "Entity identity processing unavailable".to_owned())
    }
}

impl IdentityWorker {
    fn poll_pairs(&self) -> Result<OwnerWorkerPoll, GatewayError> {
        // Only this coordinator owns the cursor. No owner lock is held over evaluation.
        let mut cursor = self
            .cursor
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let keys = {
            let _topology = self
                .instance
                .rel_topology
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            let mount = crate::instance_owner::RelMount::capture(&self.instance, self.owner)?;
            if cursor.mount.as_ref() != Some(&mount) {
                cursor.seen.clear();
                cursor.after = None;
                cursor.mount = Some(mount);
            }
            self.instance.with_reliquary(self.owner, |session| {
                session.identity_pairs_after(cursor.after.as_ref(), BATCH_SIZE)
            })?
        };
        for &key in &keys {
            let observed = self
                .instance
                .with_identity_pair(&key, true, |left, _, stamp| {
                    Ok(left
                        .session()
                        .identity_receipt(&key, None)
                        .map(|r| (stamp, r.revision)))
                });
            let Ok(Some((stamp, rev))) = observed else {
                cursor.seen.remove(&key);
                continue;
            };
            if cursor
                .seen
                .get(&key)
                .is_some_and(|(seen, r)| seen == &stamp && *r == rev)
            {
                continue;
            }
            // A denial, held provider, or changing input on one peer must not block the next pair.
            if let Ok(receipt) =
                self.instance
                    .compare_entity_identity(key, self.endpoint.as_deref(), &self.route)
            {
                cursor.seen.insert(key, (stamp, receipt.revision));
            }
        }
        if keys.len() == BATCH_SIZE {
            cursor.after = keys.last().copied();
            Ok(OwnerWorkerPoll::Progress)
        } else {
            cursor.after = None;
            Ok(OwnerWorkerPoll::Idle)
        }
    }
}
