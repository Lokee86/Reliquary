//! The existing Graph node table owns ordinal interpretation.
use super::GraphStore;
use crate::GraphError;
use crate::graph_codec::GraphMutationPayload;
use crate::graph_compact_contract::GraphOrdinalMutation;

impl GraphStore {
    /// Stable full-ID evidence for import comparison, never a persisted writer.
    /// Preserve single/batch shape and mutation order while removing ordinal scope.
    pub(crate) fn normalized_import_evidence(&self, bytes: &[u8]) -> Result<Vec<u8>, GraphError> {
        let (single, values) = self
            .decode_stored_transaction(bytes)?
            .ok_or(GraphError::InvalidGraphVersion)?;
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
    /// of this owner's ordinal assignment. Comparison hash schema tags stay exact.
    pub(crate) fn compact_carrier_digest(
        &self,
        bytes: &[u8],
    ) -> Result<Option<[u8; 32]>, GraphError> {
        use sha2::{Digest, Sha256};
        let (ordinals, magic) =
            if let Some(envelope) = reliquary_container::publication_envelope::decode(bytes)? {
                let Some(publication) = crate::graph_compact_codec::publication::decode(
                    envelope.body,
                    envelope.global_version,
                )?
                else {
                    return Ok(None);
                };
                let magic = if publication.is_single() {
                    b"CVAGCM01"
                } else {
                    b"CVAGCB01"
                };
                (publication.mutations().to_vec(), magic.as_slice())
            } else {
                let Some(publication) = crate::graph_compact_codec::publication::decode(bytes, 1)?
                else {
                    return Ok(None);
                };
                let magic = if publication.is_single() {
                    b"CVAGCM01"
                } else {
                    b"CVAGCB01"
                };
                (publication.mutations().to_vec(), magic.as_slice())
            };
        let values = self.resolve_compact_mutations(&ordinals)?;
        let mut hash = Sha256::new();
        hash.update(b"Reliquary Graph semantic carrier v1\0");
        hash.update(magic);
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

    /// Resolve concrete current publications from an accepted owner.
    pub(crate) fn decode_stored_mutations(
        &self,
        bytes: &[u8],
    ) -> Result<Option<Vec<GraphMutationPayload>>, GraphError> {
        Ok(self
            .decode_stored_transaction(bytes)?
            .map(|(_, values)| values))
    }

    pub(crate) fn decode_stored_transaction(
        &self,
        bytes: &[u8],
    ) -> Result<Option<(bool, Vec<GraphMutationPayload>)>, GraphError> {
        if let Some(envelope) = reliquary_container::publication_envelope::decode(bytes)? {
            let Some(publication) = crate::graph_compact_codec::publication::decode(
                envelope.body,
                envelope.global_version,
            )?
            else {
                return Ok(None);
            };
            return self
                .resolve_compact_mutations(publication.mutations())
                .map(|values| Some((publication.is_single(), values)));
        }
        let Some(publication) = crate::graph_compact_codec::publication::decode(bytes, 1)? else {
            return Ok(None);
        };
        self.resolve_compact_mutations(publication.mutations())
            .map(|values| Some((publication.is_single(), values)))
    }

    #[cfg(test)]
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
