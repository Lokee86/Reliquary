//! Authorized comparison of one explicit pair. Only its recording REL is writable.
use super::*;
use reliquary_application::{
    ManagementError,
    entity_identity::{IdentityComparison, IdentityPairEvidence, retained_identity_quotes},
};
use reliquary_owner_session::{
    IdentityPublicationProof,
    model::identity::{IdentityOutcome, IdentityPair, IdentityReceipt},
};
use std::sync::RwLockReadGuard;
mod locked_pair;
mod separation;
mod worker;
use locked_pair::{HeldRel, PairStamp};
use separation::separation_inspection;
pub use separation::{
    IdentitySeparationControl, IdentitySeparationInspection, IdentitySeparationPreview,
    IdentitySeparationRequest, IdentitySeparationReview, IdentitySeparationState,
    PreparedIdentitySeparation,
};
pub use worker::IdentityLifecycle;
use worker::{BackgroundAdmission, IdentityRoute, RouteBinding};

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub enum IdentityComparisonState {
    Current,
    Pending,
    Historical,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub enum IdentityEffectiveDecision {
    ConfirmedSeparate,
    Evaluated(IdentityOutcome),
    Pending,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct EntityIdentityInspection {
    pub receipt: IdentityReceipt,
    pub state: IdentityComparisonState,
    /// In receipt citation order. Absent when the observed basis cannot be verified.
    pub cited_quotes: Option<Vec<String>>,
    /// The current separation head, independent of the selected receipt revision.
    pub separation: Option<IdentitySeparationInspection>,
    /// The current pair decision, also when inspecting a historical receipt.
    pub effective: IdentityEffectiveDecision,
}

fn evidence(
    left: &RelSession,
    right: &RelSession,
    pair: &IdentityPair,
) -> Result<Option<IdentityPairEvidence>, GatewayError> {
    let Some(local) = left.identity_evidence(pair.local_entity)? else {
        return Ok(None);
    };
    let Some(peer) = right.identity_evidence(pair.peer_entity)? else {
        return Ok(None);
    };
    Ok(Some(IdentityPairEvidence::new(local, peer)))
}

fn prepare_pair(
    left: &RelSession,
    right: &RelSession,
    pair: &IdentityPair,
) -> Result<(IdentityPairEvidence, IdentityPublicationProof), GatewayError> {
    let missing = || GatewayError::Management(ManagementError::NotFound);
    let (local, proof) = left
        .identity_evidence_with_proof(pair.local_entity)?
        .ok_or_else(missing)?;
    let peer = right
        .identity_evidence(pair.peer_entity)?
        .ok_or_else(missing)?;
    Ok((IdentityPairEvidence::new(local, peer), proof))
}

fn stale() -> GatewayError {
    GatewayError::Management(ManagementError::Stale)
}

fn comparison_state(
    receipt: &IdentityReceipt,
    basis: Option<&IdentityPairEvidence>,
    route: &IdentityRoute,
) -> IdentityComparisonState {
    match basis {
        None => IdentityComparisonState::Unavailable,
        Some(basis) if IdentityComparison::prepare(basis.clone(), &route.id).matches(receipt) => {
            IdentityComparisonState::Current
        }
        Some(_) => IdentityComparisonState::Pending,
    }
}

fn inspect_comparison(
    local: &RelSession,
    peer: &RelSession,
    receipt: &IdentityReceipt,
    basis: Option<&IdentityPairEvidence>,
    route: &IdentityRoute,
    historical: bool,
) -> Result<EntityIdentityInspection, GatewayError> {
    let pair = &receipt.pair;
    let current = local.identity_receipt(pair, None).unwrap_or(receipt);
    let current_state = comparison_state(current, basis, route);
    let state = if historical {
        IdentityComparisonState::Historical
    } else {
        current_state
    };
    let cited_quotes =
        match basis.filter(|basis| !historical && basis.fingerprint() == receipt.fingerprint) {
            Some(basis) => receipt
                .citations
                .iter()
                .map(|citation| basis.quote(*citation).map(str::to_owned))
                .collect(),
            None => retained_identity_quotes(local, peer, receipt)?,
        };
    let separation = local
        .identity_separation(pair, None)
        .map(|event| separation_inspection(local, peer, event));
    let effective = if separation
        .as_ref()
        .is_some_and(|view| view.state == IdentitySeparationState::Applies)
    {
        IdentityEffectiveDecision::ConfirmedSeparate
    } else {
        match current_state {
            IdentityComparisonState::Current => {
                IdentityEffectiveDecision::Evaluated(current.outcome)
            }
            IdentityComparisonState::Unavailable => IdentityEffectiveDecision::Unavailable,
            _ => IdentityEffectiveDecision::Pending,
        }
    };
    Ok(EntityIdentityInspection {
        receipt: receipt.clone(),
        state,
        cited_quotes,
        separation,
        effective,
    })
}

impl IdentityLifecycle {
    pub fn inspect(
        &self,
        instance: &Instance,
        pair: IdentityPair,
        revision: Option<u64>,
    ) -> Result<Option<EntityIdentityInspection>, GatewayError> {
        instance.with_identity_pair(&pair, false, |local, peer, _| {
            let _publication = instance.fence_identity_access(&pair)?;
            let local = local.session();
            let Some(receipt) = local.identity_receipt(&pair, revision) else {
                return Ok(None);
            };
            let historical = local
                .identity_receipt(&pair, None)
                .is_some_and(|latest| latest.revision != receipt.revision);
            let basis = evidence(local, peer, &pair)?;
            let route = self.route_binding().read()?;
            inspect_comparison(local, peer, receipt, basis.as_ref(), &route, historical).map(Some)
        })
    }
}

impl Instance {
    fn fence_identity_access(
        &self,
        pair: &IdentityPair,
    ) -> Result<RwLockReadGuard<'_, ()>, GatewayError> {
        let publication = self
            .credential_publication
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        self.authorize_rel_access(OwnerId(pair.local_owner))?;
        self.authorize_rel_access(OwnerId(pair.peer_owner))?;
        Ok(publication)
    }

    fn compare_entity_identity(
        &self,
        pair: IdentityPair,
        binding: &RouteBinding,
        route: &IdentityRoute,
        admission: Option<BackgroundAdmission<'_>>,
        before_evaluation: impl FnOnce([u8; 32]) -> Result<(), GatewayError>,
    ) -> Result<IdentityReceipt, GatewayError> {
        let admitted = || admission.as_ref().map(BackgroundAdmission::pin).transpose();
        let _ = admitted()?;
        let (mut prepared_on, current, (basis, mut proof)) =
            self.with_identity_pair(&pair, true, |local, peer, stamp| {
                let local = local.session();
                Ok((
                    stamp,
                    local.identity_receipt(&pair, None).cloned(),
                    prepare_pair(local, peer, &pair)?,
                ))
            })?;
        let fingerprint = basis.fingerprint();
        let proposal = IdentityComparison::prepare(basis, &route.id);
        self.ensure_credential_active()?;
        let _ = admitted()?;
        let route_guard = binding.fence(route)?;
        if let Some(receipt) = current.as_ref().filter(|r| proposal.matches(r)) {
            return Ok(receipt.clone());
        }
        drop(route_guard);
        before_evaluation(fingerprint)?;
        let receipt = proposal
            .evaluate(pair, current.map(|r| r.revision), route.endpoint.as_deref())
            .map_err(GatewayError::Management)?;
        // One cut-only race can reuse the judgment after recapturing its exact inputs.
        // A changed mount or a genuinely changed input must return to preparation.
        for attempt in 0..2 {
            let result = self.with_identity_pair(&pair, true, |local, _, stamp| {
                if stamp.mounts != prepared_on.mounts {
                    return Err(stale());
                }
                if stamp.cuts != prepared_on.cuts {
                    return Ok(None);
                }
                let HeldRel::Writer(writer) = local else {
                    return Err(GatewayError::WriterUnavailable);
                };
                let _publication = self
                    .credential_publication
                    .read()
                    .map_err(|_| GatewayError::RegistryPoisoned)?;
                self.ensure_credential_active()?;
                let _admitted = admitted()?;
                let route_guard = binding.fence(route)?;
                if let Some(current) = writer.read_session().identity_receipt(&pair, None) {
                    if current.fingerprint == receipt.fingerprint
                        && current.policy == receipt.policy
                        && current.route == receipt.route
                    {
                        return Ok(Some(current.clone()));
                    }
                    if Some(current.revision) != receipt.predecessor {
                        return Err(stale());
                    }
                } else if receipt.predecessor.is_some() {
                    return Err(stale());
                }
                let published =
                    writer.publish_prepared_identity_receipt(receipt.clone(), &proof)?;
                drop(route_guard);
                drop(_publication);
                writer.refresh_checkpoint_if_due();
                Ok(Some(published))
            })?;
            if let Some(result) = result {
                return Ok(result);
            }
            if attempt == 0 {
                let (stamp, basis, refreshed) =
                    self.recapture_identity_pair(&pair, &prepared_on)?;
                if basis.fingerprint() != receipt.fingerprint {
                    return Err(stale());
                }
                prepared_on = stamp;
                proof = refreshed;
            }
        }
        Err(stale())
    }

    // Recapture each owner's evidence separately. The next publication attempt checks both
    // cuts together, so neither writer has to wait while the other's bodies are read.
    fn recapture_identity_pair(
        &self,
        pair: &IdentityPair,
        prepared: &PairStamp,
    ) -> Result<(PairStamp, IdentityPairEvidence, IdentityPublicationProof), GatewayError> {
        use crate::instance_owner::RelMount;

        let local = OwnerId(pair.local_owner);
        let peer = OwnerId(pair.peer_owner);
        self.authorize_rel_access(local)?;
        self.authorize_rel_access(peer)?;
        let _topology = self
            .rel_topology
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let mounts = (
            RelMount::capture(self, local)?,
            RelMount::capture(self, peer)?,
        );
        if mounts != prepared.mounts {
            return Err(stale());
        }
        let (left_cut, left) = self.with_reliquary(local, |session| {
            (
                session.identity_evidence_cut(),
                session.identity_evidence_with_proof(pair.local_entity),
            )
        })?;
        let (left, proof) = left?.ok_or_else(stale)?;
        let (right_cut, right) = self.with_reliquary(peer, |session| {
            (
                session.identity_evidence_cut(),
                session.identity_evidence(pair.peer_entity),
            )
        })?;
        let basis = IdentityPairEvidence::new(left, right?.ok_or_else(stale)?);
        Ok((
            PairStamp {
                mounts,
                cuts: (left_cut, right_cut),
            },
            basis,
            proof,
        ))
    }
}
