use super::*;
use crate::instance_owner::RelMount;
use reliquary_owner_session::model::identity_evidence::IdentityEvidenceCut;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct PairStamp {
    pub(super) mounts: (RelMount, RelMount),
    pub(super) cuts: (IdentityEvidenceCut, IdentityEvidenceCut),
}

pub(super) enum HeldRel<'a> {
    Reader(&'a RelSession),
    Writer(&'a mut RelWriter),
}

impl HeldRel<'_> {
    pub(super) fn session(&self) -> &RelSession {
        match self {
            Self::Reader(session) => session,
            Self::Writer(writer) => writer.read_session(),
        }
    }
}

// The topology guard pins both mounts. Order the two writer mutexes by owner,
// including when the peer is mounted writable but used only for reading.
impl Instance {
    pub(super) fn with_identity_pair<R>(
        &self,
        pair: &IdentityPair,
        write: bool,
        operation: impl FnOnce(HeldRel<'_>, &RelSession, PairStamp) -> Result<R, GatewayError>,
    ) -> Result<R, GatewayError> {
        let local_id = OwnerId(pair.local_owner);
        let peer_id = OwnerId(pair.peer_owner);
        self.authorize_rel_access(local_id)?;
        self.authorize_rel_access(peer_id)?;
        if local_id == peer_id {
            return Err(GatewayError::Management(ManagementError::InvalidInput(
                "Entity identity requires two distinct REL owners",
            )));
        }
        let _topology = self
            .rel_topology
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let (local, peer) = {
            let registry = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            let get = |id| {
                registry
                    .get(&id)
                    .cloned()
                    .ok_or(GatewayError::UnknownOwner(id))
            };
            (get(local_id)?, get(peer_id)?)
        };
        let mounts = (
            RelMount::from_mounted(&local, local_id)?,
            RelMount::from_mounted(&peer, peer_id)?,
        );
        if write && !matches!(local, MountedOwner::RelWriter(_)) {
            return Err(GatewayError::WriterUnavailable);
        }
        let call = |left: HeldRel<'_>, right: HeldRel<'_>| {
            self.ensure_credential_active()?;
            let cuts = (
                left.session().identity_evidence_cut(),
                right.session().identity_evidence_cut(),
            );
            operation(left, right.session(), PairStamp { mounts, cuts })
        };
        if local_id < peer_id {
            hold(&local, local_id, |left| {
                hold(&peer, peer_id, |right| call(left, right))
            })
        } else {
            hold(&peer, peer_id, |right| {
                hold(&local, local_id, |left| call(left, right))
            })
        }
    }
}

fn hold<R>(
    owner: &MountedOwner,
    id: OwnerId,
    call: impl FnOnce(HeldRel<'_>) -> Result<R, GatewayError>,
) -> Result<R, GatewayError> {
    match owner {
        MountedOwner::Rel(session) => call(HeldRel::Reader(session)),
        MountedOwner::RelWriter(mount) => {
            let mut writer = mount
                .writer
                .lock()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            call(HeldRel::Writer(&mut writer))
        }
        _ => Err(GatewayError::WrongOwnerKind(id)),
    }
}
