//! The existing Graph node table owns ordinal interpretation.
use super::GraphStore;
use crate::GraphError;
use crate::graph_codec::{GraphMutationPayload, decode_batch, decode_mutation};
use crate::graph_compact_contract::GraphOrdinalMutation;

impl GraphStore {
    /// Stable full-ID evidence for import comparison, never a persisted writer.
    /// Preserve single/batch shape and mutation order while removing ordinal scope.
    pub(crate) fn normalized_import_evidence(&self, bytes: &[u8]) -> Result<Vec<u8>, GraphError> {
        let values = self
            .decode_stored_mutations(bytes)?
            .ok_or(GraphError::InvalidGraphVersion)?;
        let single = bytes.starts_with(b"CVAGCM01") || decode_mutation(bytes)?.is_some();
        if single {
            if values.len() != 1 {
                return Err(GraphError::InvalidGraphVersion);
            }
            Ok(crate::graph_codec::encode_mutation(values[0]))
        } else {
            Ok(crate::graph_codec::encode_batch(&values))
        }
    }

    /// Bind compact publications to stable endpoint identities, independently
    /// of this owner's ordinal assignment. Legacy carrier digests stay exact.
    pub(crate) fn compact_carrier_digest(
        &self,
        bytes: &[u8],
    ) -> Result<Option<[u8; 32]>, GraphError> {
        use sha2::{Digest, Sha256};
        let Some(ordinals) = crate::graph_compact_codec::decode(bytes)? else {
            return Ok(None);
        };
        let values = self.resolve_compact_mutations(&ordinals)?;
        let mut hash = Sha256::new();
        hash.update(b"Reliquary Graph semantic carrier v1\0");
        hash.update(&bytes[..8]);
        hash.update((values.len() as u64).to_le_bytes());
        for value in values {
            hash.update([value.source.kind.code()]);
            hash.update(value.source.id);
            hash.update([value.target.kind.code()]);
            hash.update(value.target.id);
            hash.update(value.kind.code().to_le_bytes());
            hash.update([u8::from(value.active), value.origin.code()]);
        }
        Ok(Some(hash.finalize().into()))
    }

    pub(crate) fn resolve_compact_mutations(
        &self,
        values: &[GraphOrdinalMutation],
    ) -> Result<Vec<GraphMutationPayload>, GraphError> {
        values
            .iter()
            .map(|value| {
                let source = self
                    .nodes
                    .get(value.source as usize)
                    .ok_or(GraphError::InvalidNodeMapping)?
                    .semantic_node;
                let target = self
                    .nodes
                    .get(value.target as usize)
                    .ok_or(GraphError::InvalidNodeMapping)?
                    .semantic_node;
                Ok(GraphMutationPayload {
                    source,
                    target,
                    kind: value.kind,
                    active: value.active,
                    origin: value.origin,
                })
            })
            .collect()
    }

    /// For readers of an accepted owner. Open replay instead buffers ordinal
    /// carriers until their physical marker before resolving endpoints.
    pub(crate) fn decode_stored_mutations(
        &self,
        bytes: &[u8],
    ) -> Result<Option<Vec<GraphMutationPayload>>, GraphError> {
        if let Some(values) = crate::graph_compact_codec::decode(bytes)? {
            return self.resolve_compact_mutations(&values).map(Some);
        }
        if let Some(batch) = decode_batch(bytes)? {
            return Ok(Some(batch));
        }
        Ok(decode_mutation(bytes)?.map(|value| vec![value]))
    }

    pub(crate) fn encode_compact_mutations(
        &self,
        values: &[GraphMutationPayload],
        single: bool,
    ) -> Result<Vec<u8>, GraphError> {
        let ordinals = values
            .iter()
            .map(|value| {
                let source = self
                    .node_by_semantic
                    .get(&value.source)
                    .ok_or(GraphError::InvalidNodeMapping)?
                    .0;
                let target = self
                    .node_by_semantic
                    .get(&value.target)
                    .ok_or(GraphError::InvalidNodeMapping)?
                    .0;
                Ok(GraphOrdinalMutation {
                    source,
                    target,
                    kind: value.kind,
                    active: value.active,
                    origin: value.origin,
                })
            })
            .collect::<Result<Vec<_>, GraphError>>()?;
        if single {
            if ordinals.len() != 1 {
                return Err(GraphError::CorruptRecord(
                    "compact single Graph cardinality",
                ));
            }
            Ok(crate::graph_compact_codec::encode_mutation(ordinals[0]))
        } else {
            crate::graph_compact_codec::encode_batch(&ordinals)
        }
    }
}
