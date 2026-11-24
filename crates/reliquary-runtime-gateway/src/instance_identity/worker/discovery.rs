use super::*;
use reliquary_application::entity_identity::IdentityDiscoveryIndex;
use reliquary_owner_session::model::{EntityId, identity_evidence::IdentityEvidenceCut};
use std::collections::{BTreeSet, VecDeque};

const SNAPSHOT_PAGE: usize = 32;
const MATCH_PAGE: usize = 8;

type OwnerPair = (OwnerId, OwnerId);
type RecordingAdmissions = BTreeMap<OwnerId, Vec<Weak<Instance>>>;

#[derive(Clone, PartialEq, Eq)]
struct SnapshotStamp {
    mount: RelMount,
    cut: IdentityEvidenceCut,
}

struct OwnerSnapshot {
    stamp: SnapshotStamp,
    index: IdentityDiscoveryIndex,
    after: Option<EntityId>,
    complete: bool,
}

#[derive(Default)]
struct PairScan {
    stamps: Option<(SnapshotStamp, SnapshotStamp)>,
    recording: Option<OwnerId>,
    after: Option<EntityId>,
    complete: bool,
    pending: VecDeque<IdentityPair>,
    retries: BTreeMap<(Instant, IdentityPair), usize>,
}

#[derive(Default)]
pub(super) struct Discovery {
    admissions: RecordingAdmissions,
    scopes: BTreeSet<OwnerPair>,
    after_scope: Option<OwnerPair>,
    snapshots: BTreeMap<OwnerId, OwnerSnapshot>,
    scans: BTreeMap<OwnerPair, PairScan>,
}

impl Discovery {
    pub(super) fn reset_scans(&mut self) {
        self.scans.clear();
    }

    pub(super) fn poll(
        &mut self,
        control: &Control,
        route: &IdentityRoute,
        owners: &RecordingAdmissions,
    ) -> bool {
        self.refresh_scopes(owners);
        let next = self
            .after_scope
            .and_then(|scope| self.scopes.range((Excluded(scope), Unbounded)).next())
            .or_else(|| self.scopes.first());
        let Some(&scope) = next else {
            self.after_scope = None;
            return false;
        };
        self.after_scope = Some(scope);

        let Some((recording, admission)) =
            [scope, (scope.1, scope.0)]
                .into_iter()
                .find_map(|(recording, peer)| {
                    owners.get(&recording)?.iter().find_map(|admission| {
                        let instance = admission.upgrade()?;
                        instance.authorize_rel_access(peer).ok()?;
                        let _topology = instance.rel_topology.read().ok()?;
                        if !matches!(
                            RelMount::capture(&instance, recording).ok()?,
                            RelMount::Write(_)
                        ) {
                            return None;
                        }
                        RelMount::capture(&instance, peer).ok()?;
                        Some((recording, admission))
                    })
                })
        else {
            self.scans.remove(&scope);
            return false;
        };
        let peer = if recording == scope.0 {
            scope.1
        } else {
            scope.0
        };
        let Some(instance) = admission.upgrade() else {
            return false;
        };
        let left = self.read_snapshot(&instance, recording);
        let right = self.read_snapshot(&instance, peer);
        let (Some(left), Some(right)) = (left, right) else {
            self.scans.remove(&scope);
            return false;
        };
        let scan = self.scans.entry(scope).or_default();
        if scan.stamps.as_ref() != Some(&(left.clone(), right.clone()))
            || scan.recording != Some(recording)
        {
            *scan = PairScan {
                stamps: Some((left.clone(), right.clone())),
                recording: Some(recording),
                ..PairScan::default()
            };
        }
        let left_snapshot = &self.snapshots[&recording];
        let right_snapshot = &self.snapshots[&peer];
        if !left_snapshot.complete || !right_snapshot.complete {
            return true;
        }
        if scan.pending.is_empty() && !scan.complete {
            let rows =
                left_snapshot
                    .index
                    .candidate_rows(&right_snapshot.index, scan.after, MATCH_PAGE);
            scan.complete = rows.len() < MATCH_PAGE;
            for (local, peers) in rows {
                scan.after = Some(local);
                for peer_entity in peers {
                    scan.pending.push_back(IdentityPair {
                        local_owner: recording.0,
                        local_entity: local,
                        peer_owner: peer.0,
                        peer_entity,
                    });
                }
            }
        }

        let expected = PairStamp {
            mounts: (left.mount, right.mount),
            cuts: (left.cut, right.cut),
        };
        let operation = Instance::clone(&instance);
        drop(instance);
        let now = Instant::now();
        for job in 0..BATCH_SIZE {
            if control.stopping.load(Ordering::Acquire) || control.route.fence(route).is_err() {
                break;
            }
            // Keep retrying transport failures without starving newly discovered rows.
            let retry = scan
                .retries
                .first_key_value()
                .filter(|((at, _), _)| *at <= now)
                .map(|(key, &attempt)| (*key, attempt));
            let (pair, attempt) = if let Some((key, attempt)) = retry
                && (job % 2 == 0 || scan.pending.is_empty())
            {
                scan.retries.remove(&key);
                (key.1, attempt)
            } else if let Some(pair) = scan.pending.pop_front() {
                (pair, 0)
            } else {
                break;
            };
            let result = compare_discovered(&operation, control, route, admission, pair, &expected);
            let delay = match result {
                Err(GatewayError::Management(ManagementError::Unavailable(_)))
                    if route.endpoint.is_some() =>
                {
                    Some(RETRY_PAUSES[attempt])
                }
                Err(
                    GatewayError::CredentialRevoked
                    | GatewayError::Management(ManagementError::Stale),
                ) => Some(IDLE_PAUSE),
                _ => None,
            };
            if let Some(delay) = delay {
                scan.retries.insert(
                    (Instant::now() + delay, pair),
                    (attempt + 1).min(RETRY_PAUSES.len() - 1),
                );
            }
        }
        !scan.complete || !scan.pending.is_empty()
    }

    fn refresh_scopes(&mut self, owners: &RecordingAdmissions) {
        if self.admissions.len() == owners.len()
            && self
                .admissions
                .iter()
                .zip(owners)
                .all(|((previous, left), (owner, right))| {
                    previous == owner
                        && left.len() == right.len()
                        && left.iter().zip(right).all(|(a, b)| a.ptr_eq(b))
                })
        {
            return;
        }
        self.admissions = owners.clone();
        self.scopes.clear();
        let mut observed_owners = BTreeSet::new();
        for (&recording, candidates) in owners {
            for candidate in candidates {
                let Some(instance) = candidate.upgrade() else {
                    continue;
                };
                for &peer in instance
                    .rel_grants
                    .iter()
                    .filter(|&&peer| peer != recording)
                {
                    self.scopes.insert(canonical(recording, peer));
                    observed_owners.extend([recording, peer]);
                }
            }
        }
        self.snapshots
            .retain(|owner, _| observed_owners.contains(owner));
        self.scans.retain(|scope, _| self.scopes.contains(scope));
    }

    fn read_snapshot(&mut self, instance: &Instance, owner: OwnerId) -> Option<SnapshotStamp> {
        let result = (|| {
            let _topology = instance.rel_topology.read().ok()?;
            let mount = RelMount::capture(instance, owner).ok()?;
            instance
                .with_reliquary(owner, |session| {
                    let stamp = SnapshotStamp {
                        mount,
                        cut: session.identity_evidence_cut(),
                    };
                    let snapshot = self
                        .snapshots
                        .get(&owner)
                        .filter(|snapshot| snapshot.stamp == stamp);
                    let rows = if snapshot.is_some_and(|snapshot| snapshot.complete) {
                        None
                    } else {
                        let after = snapshot.and_then(|snapshot| snapshot.after);
                        Some(session.identity_discovery_keys(after, SNAPSHOT_PAGE).ok()?)
                    };
                    Some((stamp, rows))
                })
                .ok()
                .flatten()
        })();
        let Some((stamp, rows)) = result else {
            self.snapshots.remove(&owner);
            return None;
        };
        if let Some(rows) = rows {
            let new_snapshot = || OwnerSnapshot {
                stamp: stamp.clone(),
                index: IdentityDiscoveryIndex::default(),
                after: None,
                complete: false,
            };
            let snapshot = self.snapshots.entry(owner).or_insert_with(new_snapshot);
            if snapshot.stamp != stamp {
                *snapshot = new_snapshot();
            }
            snapshot.complete = rows.len() < SNAPSHOT_PAGE;
            snapshot.after = rows.last().map(|(entity, _)| *entity);
            snapshot.index.extend(rows);
        }
        Some(stamp)
    }
}

fn canonical(a: OwnerId, b: OwnerId) -> OwnerPair {
    (a.min(b), a.max(b))
}

fn compare_discovered(
    instance: &Instance,
    control: &Control,
    route: &IdentityRoute,
    admission: &Weak<Instance>,
    pair: IdentityPair,
    expected: &PairStamp,
) -> Result<(), GatewayError> {
    let admission = BackgroundAdmission {
        instance: admission,
        stopping: &control.stopping,
        discovery: Some(expected),
    };
    instance
        .compare_entity_identity(pair, &control.route, route, Some(admission), |_| Ok(()))
        .map(|_| ())
}
