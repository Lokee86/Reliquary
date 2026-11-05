use crate::{HydratedExternalMessage, verify_external_hydration};
use reliquary_owner_session::records::external_index::model::{
    ExternalChunkRef, ExternalMessageDescriptor, ExternalSourceError, MAX_EXTERNAL_MANIFEST,
    MAX_EXTERNAL_RESULTS, identifier,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const MAX_QUERY_BYTES: usize = 4096;
const MAX_TERMS_PER_MESSAGE: usize = 256;

/// Disposable search hints. Each hit must be hydrated through the host before
/// exposing its source content or accepting it as semantic evidence.
pub struct ExternalLexicalIndex {
    namespace: String,
    entries: BTreeMap<ExternalChunkRef, Vec<[u8; 32]>>,
}

impl ExternalLexicalIndex {
    pub fn new(namespace: &str) -> Result<Self, ExternalSourceError> {
        identifier(namespace)?;
        Ok(Self {
            namespace: namespace.to_owned(),
            entries: BTreeMap::new(),
        })
    }

    pub fn add_batch(
        &mut self,
        expected: &[ExternalMessageDescriptor],
        hydrated: &[HydratedExternalMessage],
    ) -> Result<(), ExternalSourceError> {
        verify_external_hydration(&self.namespace, expected, hydrated)?;
        if self.entries.len().saturating_add(hydrated.len()) > MAX_EXTERNAL_MANIFEST {
            return Err(ExternalSourceError::Bounds);
        }
        for message in hydrated {
            if !message.content.is_empty() {
                let reference = ExternalChunkRef {
                    message: message.descriptor.reference.clone(),
                    start: 0,
                    end: message.content.len() as u64,
                };
                self.entries
                    .insert(reference, term_hashes(&message.content));
            }
        }
        Ok(())
    }

    pub fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ExternalChunkRef>, ExternalSourceError> {
        if query.len() > MAX_QUERY_BYTES || limit == 0 || limit > MAX_EXTERNAL_RESULTS {
            return Err(ExternalSourceError::Bounds);
        }
        let terms = term_hashes(query);
        let mut hits: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(reference, hashes)| {
                let matches = terms
                    .iter()
                    .filter(|hash| hashes.binary_search(hash).is_ok())
                    .count();
                (matches > 0).then_some((reference, matches))
            })
            .collect();
        hits.sort_by(|(a, a_count), (b, b_count)| b_count.cmp(a_count).then_with(|| a.cmp(b)));
        Ok(hits
            .into_iter()
            .take(limit)
            .map(|(reference, _)| reference.clone())
            .collect())
    }
}

fn term_hashes(text: &str) -> Vec<[u8; 32]> {
    let mut hashes: Vec<_> = reliquary_retrieval::lexical_terms(text)
        .into_iter()
        .take(MAX_TERMS_PER_MESSAGE)
        .map(|term| Sha256::digest(term.as_bytes()).into())
        .collect();
    hashes.sort_unstable();
    hashes.dedup();
    hashes
}
