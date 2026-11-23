use super::*;
use reliquary_archive::{
    ArchiveError, Fragment, FragmentId, FragmentLexicalIndex, FragmentLexicalIndexStats,
    LexicalMatchCounts,
};
use std::sync::{Mutex, PoisonError};

/// The lexical snapshot for each searched profile in this live owner session.
#[derive(Default)]
pub(in crate::session) struct ArchiveHybridCache {
    sources: Mutex<BTreeMap<CompatibilityProfileId, Arc<ArchiveHybridSources>>>,
}

impl ArchiveHybridCache {
    pub(in crate::session) fn discard(&mut self, profile: CompatibilityProfileId) {
        self.sources
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&profile);
    }
}

/// The captured Archive coordinates, lexical evidence, and packed rows of a generation.
pub struct ArchiveHybridView {
    pub generation: VectorGeneration,
    pub dimensions: usize,
    sources: Arc<ArchiveHybridSources>,
    vectors: PackedVectorView,
}

struct ArchiveHybridSources {
    generation: VectorGenerationId,
    fragments: Box<[Fragment]>,
    fragments_with_text: usize,
    lexical: Mutex<LexicalState>,
}

enum LexicalState {
    Captured(Vec<Option<Vec<Arc<String>>>>),
    Indexed(Arc<FragmentLexicalIndex>),
}

impl ArchiveHybridSources {
    fn capture(archive: &Archive, view: &ArchiveVectorView) -> Result<Self, OwnerSessionError> {
        let mut fragments = Vec::with_capacity(view.fragments.len());
        let mut texts = Vec::with_capacity(view.fragments.len());
        let mut fragments_with_text = 0;
        for id in view.fragments.iter() {
            let (fragment, _) = archive
                .fragment_vector_source(FragmentId(id.0), view.generation.source_archive_version)
                .ok_or_else(|| {
                    OwnerSessionError::Record(
                        "active Archive vector has no fragment at its source cut".into(),
                    )
                })?;
            let text = match archive.bounded_fragment_text(fragment) {
                Ok(text) => {
                    fragments_with_text += 1;
                    Some(text)
                }
                Err(ArchiveError::MissingContent | ArchiveError::EvidenceLimitExceeded) => None,
                Err(error) => return Err(error.into()),
            };
            fragments.push(fragment.clone());
            texts.push(text);
        }
        Ok(Self {
            generation: view.generation.id,
            fragments: fragments.into_boxed_slice(),
            fragments_with_text,
            lexical: Mutex::new(LexicalState::Captured(texts)),
        })
    }

    fn index(&self) -> Result<Arc<FragmentLexicalIndex>, OwnerSessionError> {
        let mut state = self.lexical.lock().map_err(|_| {
            OwnerSessionError::Record("Archive lexical index build lock is poisoned".into())
        })?;
        match &*state {
            LexicalState::Indexed(index) => Ok(Arc::clone(index)),
            LexicalState::Captured(texts) => {
                let index = Arc::new(FragmentLexicalIndex::build(texts)?);
                *state = LexicalState::Indexed(Arc::clone(&index));
                Ok(index)
            }
        }
    }
}

impl ArchiveHybridView {
    pub fn packed_row(&self, ordinal: usize) -> Option<&[u8]> {
        self.vectors.row(ordinal)
    }

    pub fn fragments(&self) -> &[Fragment] {
        &self.sources.fragments
    }

    pub fn fragments_with_text(&self) -> usize {
        self.sources.fragments_with_text
    }

    pub fn lexical_counts(
        &self,
        terms: &[String],
    ) -> Result<Vec<LexicalMatchCounts>, OwnerSessionError> {
        if terms.is_empty() {
            return Ok(vec![LexicalMatchCounts::default(); self.fragments().len()]);
        }
        Ok(self.sources.index()?.counts(terms))
    }

    pub fn lexical_index_stats(&self) -> Result<FragmentLexicalIndexStats, OwnerSessionError> {
        Ok(self.sources.index()?.stats())
    }
}

impl RelSession {
    /// Pin a reusable generation snapshot. Its first lexical lookup builds the index
    /// from captured text outside the owning writer lock.
    pub fn archive_hybrid_view(
        &self,
        profile_id: CompatibilityProfileId,
    ) -> Result<Option<ArchiveHybridView>, OwnerSessionError> {
        let Some(view) = self.archive_vector_view(profile_id)? else {
            return Ok(None);
        };
        let mut cached = self.archive_hybrid_cache.sources.lock().map_err(|_| {
            OwnerSessionError::Record("Archive hybrid snapshot cache lock is poisoned".into())
        })?;
        let sources = match cached.get(&profile_id) {
            Some(sources) if sources.generation == view.generation.id => Arc::clone(sources),
            _ => {
                let sources = Arc::new(ArchiveHybridSources::capture(&self.archive, &view)?);
                cached.insert(profile_id, Arc::clone(&sources));
                sources
            }
        };
        Ok(Some(ArchiveHybridView {
            generation: view.generation,
            dimensions: view.dimensions,
            sources,
            vectors: view.vectors,
        }))
    }
}
