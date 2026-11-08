//! Named REL association admission and its durable request lookup.

use super::*;
use reliquary_graph::{AssociationEndpoint, UserAssociation};

#[derive(Clone, Copy, Debug)]
pub struct MemoryAssociationCut {
    pub request_key: [u8; 32],
    pub fingerprint: [u8; 32],
    pub source: MemoryId,
    pub target: MemoryId,
    pub source_revision: u64,
    pub target_revision: u64,
    pub graph_version: u64,
    pub expected_edge_revision: Option<u64>,
    pub active: bool,
}

impl RelSession {
    pub fn memory_association_receipt(
        &self,
        request_key: [u8; 32],
        fingerprint: [u8; 32],
    ) -> Result<Option<Relation>, OwnerSessionError> {
        match self.common.graph.user_association(request_key) {
            Some(receipt) if receipt.fingerprint == fingerprint => Ok(Some(receipt.relation())),
            Some(_) => Err(OwnerSessionError::ManagementConflict),
            None => Ok(None),
        }
    }
}

impl RelWriter {
    pub fn publish_memory_association(
        &mut self,
        cut: MemoryAssociationCut,
    ) -> Result<Relation, OwnerSessionError> {
        self.ensure_write_ready()?;
        let owner = self.owner_id();
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::ManagementStale);
        }
        let endpoint = |id, revision| {
            let (record, body) = self
                .session
                .read_current_association_endpoint(id)
                .ok_or(OwnerSessionError::ManagementMissing)?;
            if record.revision != revision || record.archived || !body {
                return Err(OwnerSessionError::ManagementStale);
            }
            Ok(AssociationEndpoint {
                id: record.id.0,
                revision,
                body: record.body_id.0,
            })
        };
        let effect = UserAssociation {
            request_key: cut.request_key,
            fingerprint: cut.fingerprint,
            source: endpoint(cut.source, cut.source_revision)?,
            target: endpoint(cut.target, cut.target_revision)?,
            graph_version: cut.graph_version,
            expected_edge_revision: cut.expected_edge_revision,
            active: cut.active,
        };
        let graph = &mut self.session.common.graph;
        if graph.graph_version() != cut.graph_version {
            return Err(OwnerSessionError::ManagementStale);
        }
        let previous = graph.relation(
            NodeRef::Memory(effect.source.id),
            NodeRef::Memory(effect.target.id),
            reliquary_graph::RelationKind::Topical,
        );
        if previous.is_some_and(|prior| prior.origin != reliquary_graph::RelationOrigin::User) {
            return Err(OwnerSessionError::ManagementConflict);
        }
        if previous.map(|prior| prior.graph_version) != cut.expected_edge_revision {
            return Err(OwnerSessionError::ManagementStale);
        }
        if previous.is_some_and(|prior| prior.active) == cut.active {
            return Err(OwnerSessionError::ManagementConflict);
        }
        let result = graph.put_user_association(
            &mut self.container,
            &GraphAdmission::for_changes(owner, [effect.source.id, effect.target.id], []),
            effect,
        );
        match result {
            Ok(relation) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(relation)
            }
            Err(error) => {
                if matches!(error, GraphError::Container(_))
                    || before != self.container.writer_lease_proof()
                {
                    self.session.write_poisoned = true;
                }
                Err(error.into())
            }
        }
    }
}

impl RelSession {
    fn read_current_association_endpoint(
        &self,
        id: MemoryId,
    ) -> Option<(&reliquary_memory_core::MemoryRecord, bool)> {
        let memories = &self.common.memories;
        let record = memories.record_at_cut(id, memories.memory_version())?;
        Some((record, memories.current_body_available(id)))
    }
}
