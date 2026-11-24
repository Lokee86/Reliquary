use super::*;
use crate::instance_owner::RelMount;
use reliquary_owner_session::model::identity::{
    IdentitySeparationAction, IdentitySeparationEvent, MAX_IDENTITY_SEPARATION_OPERATOR_ID_BYTES,
};
use std::{
    sync::Weak,
    time::{Duration, Instant},
};

const REVIEW_LIFETIME: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentitySeparationRequest {
    Confirm,
    Undo { target_revision: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub enum IdentitySeparationReview {
    Confirm {
        evaluator: EntityIdentityInspection,
        disagrees: bool,
    },
    Undo {
        confirmation: IdentitySeparationEvent,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct IdentitySeparationPreview {
    pub pair: IdentityPair,
    pub action_id: [u8; 16],
    pub review: IdentitySeparationReview,
}

/// Server-side state created by an authorized review.
pub struct PreparedIdentitySeparation {
    preview: IdentitySeparationPreview,
    event: IdentitySeparationEvent,
    instance_id: Uuid,
    mounts: (RelMount, RelMount),
    route: Option<Weak<IdentityRoute>>,
    expires_at: Instant,
}

impl PreparedIdentitySeparation {
    pub fn preview(&self) -> &IdentitySeparationPreview {
        &self.preview
    }
}

impl fmt::Debug for PreparedIdentitySeparation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedIdentitySeparation")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub enum IdentitySeparationState {
    Applies,
    Withdrawn,
    Historical,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct IdentitySeparationInspection {
    pub event: IdentitySeparationEvent,
    pub state: IdentitySeparationState,
    pub withdrawn_confirmation: Option<IdentitySeparationEvent>,
}

pub(super) fn separation_inspection(
    local: &RelSession,
    peer: &RelSession,
    event: &IdentitySeparationEvent,
) -> IdentitySeparationInspection {
    let historical = local
        .identity_separation(&event.pair, None)
        .is_some_and(|head| head.revision != event.revision);
    let local_entity = event.pair.local_entity;
    let peer_entity = event.pair.peer_entity;
    let (state, withdrawn_confirmation) = match event.action {
        IdentitySeparationAction::Undo { target_revision } => (
            IdentitySeparationState::Withdrawn,
            local
                .identity_separation(&event.pair, Some(target_revision))
                .cloned(),
        ),
        IdentitySeparationAction::Confirm { .. } if historical => {
            (IdentitySeparationState::Historical, None)
        }
        IdentitySeparationAction::Confirm { left, right, .. } => {
            let state = if !local.identity_endpoint_active(local_entity)
                || !peer.identity_endpoint_active(peer_entity)
            {
                IdentitySeparationState::Unavailable
            } else if local.identity_endpoint_proof(local_entity, Some(left.revision)) == Some(left)
                && peer.identity_endpoint_proof(peer_entity, Some(right.revision)) == Some(right)
            {
                IdentitySeparationState::Applies
            } else {
                IdentitySeparationState::Historical
            };
            (state, None)
        }
    };
    IdentitySeparationInspection {
        event: event.clone(),
        state: if historical {
            IdentitySeparationState::Historical
        } else {
            state
        },
        withdrawn_confirmation,
    }
}

impl IdentityLifecycle {
    /// Inspect the latest separation event or a specific revision with both REL grants.
    pub fn inspect_separation(
        &self,
        instance: &Instance,
        pair: IdentityPair,
        revision: Option<u64>,
    ) -> Result<Option<IdentitySeparationInspection>, GatewayError> {
        instance.with_identity_pair(&pair, false, |local, peer, _| {
            let _publication = instance.fence_identity_access(&pair)?;
            let local = local.session();
            Ok(local
                .identity_separation(&pair, revision)
                .map(|event| separation_inspection(local, peer, event)))
        })
    }
}

/// Gateway-bound control for the trusted host's operator interaction.
/// The Root host integration verifies its operator approval and management grant.
pub struct IdentitySeparationControl {
    publication: Arc<RwLock<()>>,
}

impl Gateway {
    pub fn identity_separation_control(&self) -> IdentitySeparationControl {
        IdentitySeparationControl {
            publication: Arc::clone(&self.credential_publication),
        }
    }
}

impl IdentitySeparationControl {
    fn authorize(&self, instance: &Instance) -> Result<(), GatewayError> {
        if !Arc::ptr_eq(&self.publication, &instance.credential_publication) {
            return Err(GatewayError::InvalidCredential);
        }
        instance.ensure_credential_active()
    }

    pub fn prepare_separation(
        &self,
        identity: &IdentityLifecycle,
        instance: &Instance,
        pair: IdentityPair,
        request: IdentitySeparationRequest,
        action_id: [u8; 16],
    ) -> Result<PreparedIdentitySeparation, GatewayError> {
        self.authorize(instance)?;
        instance.with_identity_pair(&pair, true, |local, peer, stamp| {
            let _publication = instance.fence_identity_access(&pair)?;
            let local = local.session();
            let operator_id = instance.principal();
            validate_principal(operator_id)?;
            if operator_id.len() > MAX_IDENTITY_SEPARATION_OPERATOR_ID_BYTES
                || operator_id.trim() != operator_id
            {
                return Err(GatewayError::InvalidPrincipal);
            }
            if action_id == [0; 16] {
                return Err(GatewayError::Management(ManagementError::InvalidInput(
                    "invalid separation action ID",
                )));
            }
            if local
                .identity_separation_by_action(action_id, operator_id)
                .is_some()
            {
                return Err(GatewayError::Management(ManagementError::Conflict));
            }
            let head = local.identity_separation(&pair, None);
            let predecessor = head.map(|event| event.revision);
            let revision = predecessor
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(GatewayError::Management(ManagementError::Conflict))?;
            let (action, review, route) = match request {
                IdentitySeparationRequest::Confirm => {
                    let route = identity.route_binding().read()?;
                    let (receipt, basis) = verify_current_comparison(local, peer, &pair, &route)?;
                    let left = local
                        .identity_endpoint_proof(pair.local_entity, None)
                        .ok_or_else(stale)?;
                    let right = peer
                        .identity_endpoint_proof(pair.peer_entity, None)
                        .ok_or_else(stale)?;
                    let evaluator =
                        inspect_comparison(local, peer, receipt, Some(&basis), &route, false)?;
                    (
                        IdentitySeparationAction::Confirm {
                            receipt_revision: receipt.revision,
                            receipt_fingerprint: receipt.fingerprint,
                            left,
                            right,
                        },
                        IdentitySeparationReview::Confirm {
                            evaluator,
                            disagrees: receipt.outcome == IdentityOutcome::SameIdentity,
                        },
                        Some(Arc::downgrade(&route)),
                    )
                }
                IdentitySeparationRequest::Undo { target_revision } => {
                    let confirmation = head
                        .filter(|event| {
                            event.revision == target_revision
                                && matches!(event.action, IdentitySeparationAction::Confirm { .. })
                        })
                        .ok_or_else(stale)?;
                    (
                        IdentitySeparationAction::Undo { target_revision },
                        IdentitySeparationReview::Undo {
                            confirmation: confirmation.clone(),
                        },
                        None,
                    )
                }
            };
            Ok(PreparedIdentitySeparation {
                preview: IdentitySeparationPreview {
                    pair,
                    action_id,
                    review,
                },
                event: IdentitySeparationEvent {
                    pair,
                    revision,
                    predecessor,
                    action_id,
                    operator_id: operator_id.to_owned(),
                    action,
                },
                instance_id: instance.instance_id,
                mounts: stamp.mounts,
                route,
                expires_at: Instant::now() + REVIEW_LIFETIME,
            })
        })
    }

    pub fn commit_separation(
        &self,
        identity: &IdentityLifecycle,
        instance: &Instance,
        prepared: &PreparedIdentitySeparation,
    ) -> Result<IdentitySeparationInspection, GatewayError> {
        self.authorize(instance)?;
        let event = &prepared.event;
        let pair = &event.pair;
        instance.with_identity_pair(pair, false, |local, peer, stamp| {
            let _publication = instance.fence_identity_access(pair)?;
            if instance.instance_id != prepared.instance_id {
                return Err(GatewayError::InvalidCredential);
            }
            let session = local.session();
            if let Some(existing) =
                session.identity_separation_by_action(event.action_id, instance.principal())
            {
                if existing != event {
                    return Err(GatewayError::Management(ManagementError::Conflict));
                }
                return Ok(separation_inspection(session, peer, existing));
            }
            if Instant::now() >= prepared.expires_at || stamp.mounts != prepared.mounts {
                return Err(stale());
            }
            let revision = session
                .identity_separation(pair, None)
                .map(|head| head.revision);
            if revision != event.predecessor {
                return Err(stale());
            }
            let route = if let Some(expected) = &prepared.route {
                let route = identity.route_binding().read()?;
                if !expected.ptr_eq(&Arc::downgrade(&route)) {
                    return Err(stale());
                }
                Some(route)
            } else {
                None
            };
            if let IdentitySeparationAction::Confirm {
                receipt_revision,
                receipt_fingerprint,
                left,
                right,
            } = event.action
            {
                let route = route.as_ref().ok_or_else(stale)?;
                let (receipt, _) = verify_current_comparison(session, peer, pair, route)?;
                if receipt.revision != receipt_revision
                    || receipt.fingerprint != receipt_fingerprint
                    || session.identity_endpoint_proof(pair.local_entity, None) != Some(left)
                    || peer.identity_endpoint_proof(pair.peer_entity, None) != Some(right)
                {
                    return Err(stale());
                }
            }
            let HeldRel::Writer(writer) = local else {
                return Err(GatewayError::WriterUnavailable);
            };
            if Instant::now() >= prepared.expires_at {
                return Err(stale());
            }
            let published = writer.publish_identity_separation(event.clone())?;
            let result = separation_inspection(writer.read_session(), peer, &published);
            drop(route);
            writer.refresh_checkpoint_if_due();
            Ok(result)
        })
    }

    /// Recover an authorized operator's durable result by action ID and pair.
    pub fn lookup_separation(
        &self,
        instance: &Instance,
        pair: IdentityPair,
        action_id: [u8; 16],
    ) -> Result<Option<IdentitySeparationInspection>, GatewayError> {
        self.authorize(instance)?;
        instance.with_identity_pair(&pair, false, |local, peer, _| {
            let _publication = instance.fence_identity_access(&pair)?;
            let local = local.session();
            Ok(local
                .identity_separation_by_action(action_id, instance.principal())
                .filter(|event| event.pair == pair)
                .map(|event| separation_inspection(local, peer, event)))
        })
    }
}

#[cfg(test)]
mod tests;

fn verify_current_comparison<'a>(
    local: &'a RelSession,
    peer: &RelSession,
    pair: &IdentityPair,
    route: &IdentityRoute,
) -> Result<(&'a IdentityReceipt, IdentityPairEvidence), GatewayError> {
    let receipt = local.identity_receipt(pair, None).ok_or_else(stale)?;
    let basis = evidence(local, peer, pair)?;
    if comparison_state(receipt, basis.as_ref(), route) != IdentityComparisonState::Current {
        return Err(stale());
    }
    Ok((receipt, basis.ok_or_else(stale)?))
}
