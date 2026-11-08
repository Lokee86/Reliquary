//! Atomic graph effect and durable request identity for one user association.

use super::*;

pub const USER_ASSOCIATION_MAGIC: &[u8; 8] = b"RELASC01";
const PREFIX: &[u8; 6] = b"RELASC";
const LEN: usize = 8 + 16 + 32 + 32 + 2 * (32 + 8 + 32) + 8 + 8 + 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssociationEndpoint {
    pub id: [u8; 32],
    pub revision: u64,
    pub body: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserAssociation {
    pub request_key: [u8; 32],
    pub fingerprint: [u8; 32],
    pub source: AssociationEndpoint,
    pub target: AssociationEndpoint,
    pub graph_version: u64,
    pub expected_edge_revision: Option<u64>,
    pub active: bool,
}

impl UserAssociation {
    fn validate_shape(self) -> Result<(), GraphError> {
        if self.source.id == self.target.id
            || self.source.revision == 0
            || self.target.revision == 0
            || self.graph_version == u64::MAX
            || self.expected_edge_revision == Some(0)
            || self
                .expected_edge_revision
                .is_some_and(|version| version > self.graph_version)
            || (!self.active && self.expected_edge_revision.is_none())
        {
            return Err(GraphError::CorruptRecord("invalid user association shape"));
        }
        Ok(())
    }

    pub fn relation(self) -> Relation {
        Relation {
            source: NodeRef::Memory(self.source.id),
            target: NodeRef::Memory(self.target.id),
            kind: RelationKind::Topical,
            active: self.active,
            origin: RelationOrigin::User,
            graph_version: self.graph_version + 1,
        }
    }

    fn encode(self, owner: [u8; 16]) -> Vec<u8> {
        let mut out = Vec::with_capacity(LEN);
        out.extend_from_slice(USER_ASSOCIATION_MAGIC);
        out.extend_from_slice(&owner);
        out.extend_from_slice(&self.request_key);
        out.extend_from_slice(&self.fingerprint);
        for endpoint in [self.source, self.target] {
            out.extend_from_slice(&endpoint.id);
            out.extend_from_slice(&endpoint.revision.to_le_bytes());
            out.extend_from_slice(&endpoint.body);
        }
        out.extend_from_slice(&self.graph_version.to_le_bytes());
        out.extend_from_slice(
            &self
                .expected_edge_revision
                .unwrap_or_default()
                .to_le_bytes(),
        );
        out.push(u8::from(self.active));
        out
    }
}

pub fn user_association_in_frame(
    payload: &[u8],
) -> Result<Option<([u8; 16], UserAssociation)>, GraphError> {
    if !payload.starts_with(PREFIX) {
        return Ok(None);
    }
    if !payload.starts_with(USER_ASSOCIATION_MAGIC) || payload.len() != LEN {
        return Err(GraphError::CorruptRecord(
            "invalid user association carrier",
        ));
    }
    let mut p = USER_ASSOCIATION_MAGIC.len();
    let owner = take::<16>(payload, &mut p)?;
    let request_key = take::<32>(payload, &mut p)?;
    let fingerprint = take::<32>(payload, &mut p)?;
    let endpoint = |p: &mut usize| -> Result<AssociationEndpoint, GraphError> {
        Ok(AssociationEndpoint {
            id: take::<32>(payload, p)?,
            revision: u64::from_le_bytes(take::<8>(payload, p)?),
            body: take::<32>(payload, p)?,
        })
    };
    let source = endpoint(&mut p)?;
    let target = endpoint(&mut p)?;
    let graph_version = u64::from_le_bytes(take::<8>(payload, &mut p)?);
    let expected = u64::from_le_bytes(take::<8>(payload, &mut p)?);
    let active = match take::<1>(payload, &mut p)?[0] {
        0 => false,
        1 => true,
        _ => return Err(GraphError::CorruptRecord("invalid user association action")),
    };
    let association = UserAssociation {
        request_key,
        fingerprint,
        source,
        target,
        graph_version,
        expected_edge_revision: (expected != 0).then_some(expected),
        active,
    };
    association.validate_shape()?;
    Ok(Some((owner, association)))
}

impl GraphStore {
    pub fn user_association(&self, request_key: [u8; 32]) -> Option<UserAssociation> {
        self.user_associations.get(&request_key).copied()
    }

    fn validate_user_association(
        &self,
        admission: &GraphAdmission,
        effect: UserAssociation,
        replay: bool,
    ) -> Result<(), GraphError> {
        effect.validate_shape()?;
        if admission.owner_id != Some(self.owner)
            || self.identity.file_kind != reliquary_container::FileKind::Reliquary
        {
            return Err(GraphError::WrongOwner);
        }
        let relation = effect.relation();
        validate_endpoints(admission, relation.source, relation.target, replay)?;
        if effect.graph_version != self.graph_version {
            return Err(GraphError::InvalidRelation(
                "user association graph cut changed",
            ));
        }
        let previous = self.relation(relation.source, relation.target, relation.kind);
        if previous.is_some_and(|prior| prior.origin != RelationOrigin::User) {
            return Err(GraphError::InvalidRelation(
                "user association would replace a non-user edge",
            ));
        }
        if previous.map(|prior| prior.graph_version) != effect.expected_edge_revision {
            return Err(GraphError::InvalidRelation(
                "user association edge revision changed",
            ));
        }
        if previous.is_some_and(|prior| prior.active) == effect.active {
            return Err(GraphError::InvalidRelation(
                "user association must change its edge",
            ));
        }
        if self.user_associations.contains_key(&effect.request_key) {
            return Err(GraphError::CorruptRecord(
                "duplicate user association request",
            ));
        }
        Ok(())
    }

    pub(super) fn replay_user_association(
        &mut self,
        admission: &GraphAdmission,
        effect: UserAssociation,
    ) -> Result<(), GraphError> {
        self.validate_user_association(admission, effect, true)?;
        self.apply_relation(effect.relation())?;
        self.user_associations.insert(effect.request_key, effect);
        Ok(())
    }

    pub fn put_user_association(
        &mut self,
        container: &mut Container,
        admission: &GraphAdmission,
        effect: UserAssociation,
    ) -> Result<Relation, GraphError> {
        self.check_owner(container)?;
        self.validate_user_association(admission, effect, false)?;
        container.append(&effect.encode(self.owner))?;
        container.sync()?;
        let relation = effect.relation();
        self.apply_relation(relation)?;
        self.user_associations.insert(effect.request_key, effect);
        Ok(relation)
    }
}
