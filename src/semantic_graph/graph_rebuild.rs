use crate::entity_store::EntityStore;
use crate::graph_codec::{GraphMutationPayload, decode_format, decode_node};
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::{GraphError, GraphNodeRecord, SemanticGraphRelation};
use reliquary_container::ObjectRef;
use std::collections::HashSet;

#[cfg(test)]
#[path = "graph_publication_replay_tests.rs"]
mod publication_replay_tests;

pub(crate) struct GraphOpenState {
    store: GraphStore,
    format_seen: bool,
    data_seen: bool,
}

impl GraphOpenState {
    pub(crate) fn new() -> Self {
        Self {
            store: GraphStore::empty(),
            format_seen: false,
            data_seen: false,
        }
    }

    pub(crate) fn ingest(
        &mut self,
        _chunk: ObjectRef,
        payload: &[u8],
        latest_global: u64,
    ) -> Result<(), GraphError> {
        if let Some(envelope) = reliquary_container::publication_envelope::decode(payload)? {
            self.ingest_publication_envelope(envelope, latest_global)?;
            return Ok(());
        }
        if decode_format(payload)? {
            if self.format_seen {
                return Err(GraphError::ConflictingFormat);
            }
            self.format_seen = true;
            self.store.mark_format_initialized();
            return Ok(());
        }
        if self.store.ingest_rewrite_baseline(payload)? {
            self.data_seen = true;
            return Ok(());
        }
        if let Some(node) = decode_node(payload)? {
            self.data_seen = true;
            self.store.insert_node_rebuilt(GraphNodeRecord {
                semantic_node: node.semantic_node,
                node_id: node.node_id,
            })?;
            return Ok(());
        }
        Ok(())
    }

    pub(crate) fn ingest_publication_envelope(
        &mut self,
        envelope: reliquary_container::publication_envelope::PublicationEnvelope<'_>,
        latest_global: u64,
    ) -> Result<bool, GraphError> {
        let Some(publication) = crate::graph_compact_codec::publication::decode(
            envelope.body,
            envelope.global_version,
        )?
        else {
            return Ok(false);
        };
        if envelope.global_version > latest_global {
            return Err(GraphError::InvalidGraphVersion);
        }
        let mutations = self
            .store
            .resolve_compact_mutations(publication.mutations())?;
        self.apply_mutations(
            mutations,
            envelope.global_version,
            publication.graph_version,
        )?;
        self.data_seen = true;
        Ok(true)
    }

    pub(crate) fn finish(
        mut self,
        memories: &MemoryStore,
        entities: &EntityStore,
    ) -> Result<GraphStore, GraphError> {
        if !self.format_seen {
            if !self.data_seen {
                return Ok(GraphStore::empty());
            }
            return Err(GraphError::MissingFormat);
        }
        self.store.finish_rebuild(memories, entities)?;
        Ok(self.store)
    }

    fn apply_mutations(
        &mut self,
        mutations: Vec<GraphMutationPayload>,
        global_version: u64,
        graph_version: u64,
    ) -> Result<(), GraphError> {
        let mut keys = HashSet::with_capacity(mutations.len());
        let mut transaction = Vec::with_capacity(mutations.len());
        for mutation in mutations {
            if !keys.insert((mutation.source, mutation.target, mutation.kind)) {
                return Err(GraphError::DuplicateRelationChange);
            }
            transaction.push(SemanticGraphRelation {
                source: mutation.source,
                target: mutation.target,
                kind: mutation.kind,
                active: mutation.active,
                origin: mutation.origin,
                global_version,
                graph_version,
            });
        }
        self.store.insert_transaction_rebuilt(&transaction)?;
        Ok(())
    }
}
