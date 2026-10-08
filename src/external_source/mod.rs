//! Framework-neutral external canonical transcript integration.
//! Host text is transient. This lane owns rebuildable index state, not history.
pub(crate) mod model;
pub(crate) mod publication;
use model::identifier;
pub use model::{
    EXTERNAL_SOURCE_CONTRACT, ExternalChangeBatch, ExternalChangeKind, ExternalChunkRef,
    ExternalConversationChange, ExternalConversationSource, ExternalIndexStatus,
    ExternalMessageDescriptor, ExternalMessageRef, ExternalSearchHit, ExternalSnapshotManifest,
    ExternalSourceError, HydratedExternalChunk, HydratedExternalMessage, MAX_EXTERNAL_BATCH,
    MAX_EXTERNAL_INDEX_BYTES, MAX_EXTERNAL_MANIFEST, MAX_EXTERNAL_MESSAGE_BYTES,
    MAX_EXTERNAL_RESULTS, external_content_hash,
};
pub(crate) mod index;
mod provenance;
pub(crate) mod store;
pub use provenance::{ExternalMemoryEvidence, SourceEvidenceRef};
pub(crate) mod processing;
pub use processing::{ExternalProcessingResult, PreparedExternalProcessing};
#[cfg(test)]
pub(crate) mod tests;

use crate::{CompatibilityProfileId, Cva, PackedVectorId};
use index::PreparedIndex;
impl Cva {
    pub(crate) fn external_index_state(
        &self,
        namespace: &str,
    ) -> Result<Option<store::SourceState>, ExternalSourceError> {
        identifier(namespace)?;
        if let Some(state) = self.external_index.sources.get(namespace) {
            return Ok(Some(state.clone()));
        }
        if self.external_index.damaged {
            return Err(ExternalSourceError::RebuildRequired);
        }
        Ok(None)
    }
    pub(crate) fn external_install(
        &mut self,
        prepared: PreparedIndex,
    ) -> Result<ExternalIndexStatus, ExternalSourceError> {
        if self
            .external_index
            .sources
            .get(&prepared.state.namespace)
            .map(|s| s.revision)
            != prepared.expected_revision
        {
            return Err(ExternalSourceError::Conflict);
        }
        if prepared.expected_revision == Some(prepared.state.revision) {
            return Ok(prepared.state.status());
        }
        self.compatibility_profiles
            .put(&mut self.container, prepared.profile)
            .map_err(|_| ExternalSourceError::Storage)?;
        for packed in prepared.matrices {
            self.packed_vectors
                .put(&mut self.container, packed)
                .map_err(|_| ExternalSourceError::Storage)?;
        }
        let status = prepared.state.status();
        self.external_index.install(
            &mut self.container,
            prepared.state,
            prepared.expected_revision,
        )?;
        Ok(status)
    }
    pub(crate) fn external_search(
        &mut self,
        state: &store::SourceState,
        vector: Option<&[f32]>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ExternalSearchHit>, ExternalSourceError> {
        if limit == 0 || limit > MAX_EXTERNAL_RESULTS || query.len() > MAX_EXTERNAL_MESSAGE_BYTES {
            return Err(ExternalSourceError::Bounds);
        }
        let terms = index::term_hashes(query);
        if query.trim().is_empty() || (vector.is_none() && terms.is_empty()) {
            return Err(ExternalSourceError::Bounds);
        }
        if let Some(vector) = vector {
            index::validate_vector(vector, state.dimensions)?;
        }

        let mut matrices = std::collections::BTreeMap::new();
        let mut hits = Vec::new();
        for chunk in state
            .messages
            .iter()
            .filter(|m| m.descriptor.active)
            .flat_map(|m| &m.chunks)
        {
            let score = if let Some(vector) = vector {
                if !matrices.contains_key(&chunk.matrix) {
                    let packed = self
                        .packed_vectors(PackedVectorId(chunk.matrix))
                        .map_err(|_| ExternalSourceError::RebuildRequired)?;
                    matrices.insert(chunk.matrix, packed);
                }
                let packed = &matrices[&chunk.matrix];
                let stride = state.dimensions as usize * 4;
                let start = chunk.row as usize * stride;
                let bytes = packed
                    .bytes()
                    .get(start..start + stride)
                    .ok_or(ExternalSourceError::RebuildRequired)?;
                let row: Vec<f32> = bytes
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                    .collect();
                index::validate_vector(&row, state.dimensions)?;
                let dot = row
                    .iter()
                    .zip(vector)
                    .map(|(a, b)| f64::from(*a) * f64::from(*b))
                    .sum::<f64>();
                let norm = row
                    .iter()
                    .map(|v| f64::from(*v).powi(2))
                    .sum::<f64>()
                    .sqrt()
                    * vector
                        .iter()
                        .map(|v| f64::from(*v).powi(2))
                        .sum::<f64>()
                        .sqrt();
                dot / norm
            } else {
                if terms.is_empty() {
                    return Err(ExternalSourceError::Bounds);
                }
                let count = terms
                    .iter()
                    .filter(|term| chunk.terms.binary_search(term).is_ok())
                    .count();
                if count == 0 {
                    continue;
                }
                count as f64 / terms.len() as f64
            };
            hits.push(ExternalSearchHit {
                reference: chunk.reference.clone(),
                score,
            });
        }
        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.reference.cmp(&b.reference))
        });
        hits.truncate(limit);
        Ok(hits)
    }
    pub(crate) fn external_profile(
        &self,
        state: &store::SourceState,
    ) -> Result<crate::CompatibilityProfile, ExternalSourceError> {
        self.compatibility_profile(CompatibilityProfileId(state.profile))
            .map_err(|_| ExternalSourceError::RebuildRequired)
    }
}
