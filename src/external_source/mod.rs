//! Framework-neutral external canonical transcript integration.
//! Host text is transient. This lane owns rebuildable index state, not history.
#[path = "../owner_lifecycle/external_publication.rs"]
pub(crate) mod publication;
pub use crate::external_index::model::{
    EXTERNAL_SOURCE_CONTRACT, ExternalChangeBatch, ExternalChangeKind, ExternalChunkRef,
    ExternalConversationChange, ExternalIndexStatus, ExternalMessageDescriptor, ExternalMessageRef,
    ExternalSearchHit, ExternalSnapshotManifest, ExternalSourceError, HydratedExternalChunk,
    HydratedExternalMessage, MAX_EXTERNAL_BATCH, MAX_EXTERNAL_INDEX_BYTES, MAX_EXTERNAL_MANIFEST,
    MAX_EXTERNAL_MESSAGE_BYTES, MAX_EXTERNAL_RESULTS, external_content_hash,
};
use crate::external_index::store;
mod provider;
pub use provider::ExternalConversationSource;
pub(crate) mod index;
mod provenance;
pub use provenance::{ExternalMemoryEvidence, SourceEvidenceRef};
pub(crate) mod processing;
pub use processing::{ExternalProcessingResult, PreparedExternalProcessing};
#[cfg(test)]
pub(crate) mod tests;

use crate::external_vector_reader::ExternalVectorReader;
use store::SourceState;

pub(crate) fn search(
    vectors: &mut ExternalVectorReader<'_>,
    state: &SourceState,
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

    let mut hits = Vec::new();
    for chunk in state
        .messages
        .iter()
        .filter(|m| m.descriptor.active)
        .flat_map(|m| &m.chunks)
    {
        let score = if let Some(vector) = vector {
            let (row, score) = vectors
                .cosine_row(chunk.matrix, chunk.row, state.dimensions, vector)
                .map_err(|_| ExternalSourceError::RebuildRequired)?;
            // Keep the External contract check here, after lazy matrix hydration.
            index::validate_vector(&row, state.dimensions)?;
            score
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

#[cfg(test)]
#[path = "publication_compact_tests.rs"]
mod publication_compact_tests;
