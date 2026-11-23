//! Authorized comparison of one explicit pair. Only its recording REL is writable.
use super::*;
use reliquary_application::{
    ManagementError,
    entity_identity::{
        IDENTITY_COMPARISON_POLICY, IdentityComparison, IdentityPairEvidence,
        retained_identity_quotes,
    },
};
use reliquary_owner_session::{
    IdentityPublicationProof,
    model::identity::{IdentityPair, IdentityReceipt},
};
use reliquary_switchboard::GeneralEndpoint;

mod locked_pair;
mod worker;
use locked_pair::{HeldRel, PairStamp};

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub enum IdentityComparisonState {
    Current,
    Pending,
    Historical,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct EntityIdentityInspection {
    pub receipt: IdentityReceipt,
    pub state: IdentityComparisonState,
    /// In receipt citation order. Absent when the observed basis cannot be verified.
    pub cited_quotes: Option<Vec<String>>,
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

impl Instance {
    pub fn compare_entity_identity(
        &self,
        pair: IdentityPair,
        endpoint: Option<&dyn GeneralEndpoint>,
        route_id: &str,
    ) -> Result<IdentityReceipt, GatewayError> {
        let (mut prepared_on, current, (basis, mut proof)) =
            self.with_identity_pair(&pair, true, |local, peer, stamp| {
                let local = local.session();
                Ok((
                    stamp,
                    local.identity_receipt(&pair, None).cloned(),
                    prepare_pair(local, peer, &pair)?,
                ))
            })?;
        let proposal = IdentityComparison::prepare(basis, route_id);
        self.ensure_credential_active()?;
        if let Some(receipt) = current.as_ref().filter(|r| proposal.matches(r)) {
            return Ok(receipt.clone());
        }
        let receipt = proposal
            .evaluate(pair, current.map(|r| r.revision), endpoint)
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

    pub fn inspect_entity_identity(
        &self,
        pair: IdentityPair,
        revision: Option<u64>,
    ) -> Result<Option<EntityIdentityInspection>, GatewayError> {
        let view = self.with_identity_pair(&pair, false, |local, peer, _| {
            let local = local.session();
            let Some(receipt) = local.identity_receipt(&pair, revision) else {
                return Ok(None);
            };
            let is_history = local
                .identity_receipt(&pair, None)
                .is_some_and(|latest| latest.revision != receipt.revision);
            if is_history {
                return Ok(Some(EntityIdentityInspection {
                    receipt: receipt.clone(),
                    state: IdentityComparisonState::Historical,
                    cited_quotes: retained_identity_quotes(local, peer, receipt)?,
                }));
            }
            let basis = evidence(local, peer, &pair)?;
            let matching = basis
                .as_ref()
                .filter(|b| b.fingerprint() == receipt.fingerprint);
            let state = if basis.is_none() {
                IdentityComparisonState::Unavailable
            } else if matching.is_none() || receipt.policy != IDENTITY_COMPARISON_POLICY {
                IdentityComparisonState::Pending
            } else {
                IdentityComparisonState::Current
            };
            let cited_quotes = match matching {
                Some(basis) => receipt
                    .citations
                    .iter()
                    .map(|c| basis.quote(*c).map(str::to_owned))
                    .collect(),
                None => retained_identity_quotes(local, peer, receipt)?,
            };
            Ok(Some(EntityIdentityInspection {
                receipt: receipt.clone(),
                state,
                cited_quotes,
            }))
        })?;
        self.ensure_credential_active()?;
        Ok(view)
    }
}
