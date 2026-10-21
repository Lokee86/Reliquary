use crate::entity_store::EntityStore;
use crate::graph_codec::{
    GraphMutationPayload, decode_batch, decode_format, decode_mutation, decode_node, decode_version,
};
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::{GraphError, GraphNodeRecord, ObjectRef, SemanticGraphRelation};
use std::collections::{HashMap, HashSet};

#[cfg(test)]
#[path = "graph_publication_replay_tests.rs"]
mod publication_replay_tests;

enum MutationCarrier {
    Stable(Vec<GraphMutationPayload>),
    Ordinal(Vec<crate::graph_compact_contract::GraphOrdinalMutation>),
}

pub(crate) struct GraphOpenState {
    store: GraphStore,
    pending: HashMap<ObjectRef, MutationCarrier>,
    compact_published: bool,
    format_seen: bool,
    data_seen: bool,
}

impl GraphOpenState {
    pub(crate) fn new() -> Self {
        Self {
            store: GraphStore::empty(),
            pending: HashMap::new(),
            compact_published: false,
            format_seen: false,
            data_seen: false,
        }
    }

    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
        latest_global: u64,
    ) -> Result<(), GraphError> {
        if let Some(envelope) = crate::container::publication_envelope::decode(payload)? {
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
        if let Some(version) = decode_version(payload)? {
            self.ingest_version(chunk, version, latest_global)?;
            self.data_seen = true;
            return Ok(());
        }
        if let Some(values) = crate::graph_compact_codec::decode(payload)? {
            self.data_seen = true;
            self.pending.insert(chunk, MutationCarrier::Ordinal(values));
            return Ok(());
        }
        if let Some(batch) = decode_batch(payload)? {
            self.data_seen = true;
            self.pending.insert(chunk, MutationCarrier::Stable(batch));
            return Ok(());
        }
        if let Some(mutation) = decode_mutation(payload)? {
            self.data_seen = true;
            self.pending
                .insert(chunk, MutationCarrier::Stable(vec![mutation]));
        }
        Ok(())
    }

    pub(crate) fn ingest_publication_envelope(
        &mut self,
        envelope: crate::container::publication_envelope::PublicationEnvelope<'_>,
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
        self.compact_published = true;
        self.data_seen = true;
        Ok(true)
    }

    pub(crate) fn validate_storage_version(
        &self,
        version: crate::FormatVersion,
    ) -> Result<(), GraphError> {
        if self.compact_published && version != (crate::FormatVersion { major: 2, minor: 0 }) {
            return Err(GraphError::ConflictingFormat);
        }
        Ok(())
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

    fn ingest_version(
        &mut self,
        chunk: ObjectRef,
        version: crate::graph_codec::GraphVersionPayload,
        latest_global: u64,
    ) -> Result<(), GraphError> {
        if version.global_version == 0
            || version.global_version > latest_global
            || !version.mutation.precedes(chunk)
        {
            return Err(GraphError::InvalidGraphVersion);
        }
        let carrier = self
            .pending
            .get(&version.mutation)
            .ok_or(GraphError::InvalidGraphVersion)?;
        // Resolve only now: admitted V1 ordering permits mappings between a
        // carrier and its marker, but never after the marker.
        let (mutations, compact) = match carrier {
            MutationCarrier::Stable(values) => (values.clone(), false),
            MutationCarrier::Ordinal(values) => {
                (self.store.resolve_compact_mutations(values)?, true)
            }
        };

        self.apply_mutations(mutations, version.global_version, version.graph_version)?;
        self.pending.remove(&version.mutation);
        self.compact_published |= compact;
        Ok(())
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
