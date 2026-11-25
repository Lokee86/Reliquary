use super::*;
use crate::session::archive_lexical::{ArchiveLexicalIndexPublication, StoredArchiveLexicalIndex};
use reliquary_archive::{
    ArchiveError, Fragment, FragmentId, FragmentLexicalIndex, FragmentLexicalIndexStats,
    LexicalMatchCounts,
};
use sha2::{Digest, Sha256};
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
    turns: Option<Box<[Option<Vec<reliquary_archive::ResolvedTurn>>]>>,
}

struct ArchiveHybridSources {
    generation: VectorGenerationId,
    source_removal_version: u64,
    eligible: Box<[bool]>,
    fragments: Box<[Fragment]>,
    fragments_with_text: usize,
    source_digest: [u8; 32],
    lexical: Mutex<LexicalState>,
}

enum LexicalState {
    Captured(Vec<Option<Vec<Arc<String>>>>),
    Indexed(Arc<FragmentLexicalIndex>),
}

impl ArchiveHybridSources {
    fn capture(
        archive: &Archive,
        view: &ArchiveVectorView,
        stored: Option<&StoredArchiveLexicalIndex>,
    ) -> Result<Self, OwnerSessionError> {
        let mut fragments = Vec::with_capacity(view.fragments.len());
        let mut texts = Vec::with_capacity(view.fragments.len());
        let mut eligible = Vec::with_capacity(view.fragments.len());
        let mut fragments_with_text = 0;
        let mut source = Sha256::new();
        source.update(b"REL Archive lexical source v1\0");
        for (ordinal, id) in view.fragments.iter().enumerate() {
            let (fragment, _) = archive
                .fragment_vector_source(FragmentId(id.0), view.generation.source_archive_version)
                .ok_or_else(|| {
                    OwnerSessionError::Record(
                        "active Archive vector has no fragment at its source cut".into(),
                    )
                })?;
            source.update(id.0);
            let source_eligible = view.source_eligible(ordinal);
            eligible.push(source_eligible);
            if !source_eligible {
                source.update(u64::MAX.to_le_bytes());
                fragments.push(fragment.clone());
                texts.push(None);
                continue;
            }
            let text = match archive.bounded_fragment_index_text(fragment) {
                Ok(turns) => {
                    fragments_with_text += 1;
                    source.update((turns.len() as u64).to_le_bytes());
                    Some(
                        turns
                            .into_iter()
                            .map(|(content, text)| {
                                source.update(content.0);
                                text
                            })
                            .collect(),
                    )
                }
                Err(ArchiveError::MissingContent | ArchiveError::EvidenceLimitExceeded) => {
                    source.update(u64::MAX.to_le_bytes());
                    None
                }
                Err(error) => return Err(error.into()),
            };
            fragments.push(fragment.clone());
            texts.push(text);
        }
        let generation = view.generation.id;
        let source_digest = source.finalize().into();
        let lexical = match stored {
            Some(index) if index.matches(generation, source_digest, fragments_with_text) => {
                LexicalState::Indexed(Arc::clone(&index.index))
            }
            _ => LexicalState::Captured(texts),
        };
        Ok(Self {
            generation,
            source_removal_version: view.source_removal_version(),
            eligible: eligible.into_boxed_slice(),
            fragments: fragments.into_boxed_slice(),
            fragments_with_text,
            source_digest,
            lexical: Mutex::new(lexical),
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

    fn matches(&self, index: &StoredArchiveLexicalIndex) -> bool {
        index.matches(
            self.generation,
            self.source_digest,
            self.fragments_with_text,
        )
    }
}

impl ArchiveHybridView {
    pub fn source_removal_version(&self) -> u64 {
        self.sources.source_removal_version
    }

    pub fn source_eligible(&self, ordinal: usize) -> bool {
        self.sources.eligible.get(ordinal).copied() == Some(true)
    }

    pub fn packed_row(&self, ordinal: usize) -> Option<&[u8]> {
        if !self.source_eligible(ordinal) {
            return None;
        }
        self.vectors.row(ordinal)
    }

    pub fn fragments(&self) -> &[Fragment] {
        &self.sources.fragments
    }

    /// Exact readable turn path captured with this generation.
    pub fn fragment_turns(&self, ordinal: usize) -> Option<&[reliquary_archive::ResolvedTurn]> {
        self.turns.as_ref()?.get(ordinal)?.as_deref()
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
    /// Capture exact readable paths only for a temporal request.
    pub fn archive_temporal_hybrid_view(
        &self,
        profile_id: CompatibilityProfileId,
    ) -> Result<Option<ArchiveHybridView>, OwnerSessionError> {
        let Some(mut view) = self.archive_hybrid_view(profile_id)? else {
            return Ok(None);
        };
        let mut paths = Vec::with_capacity(view.fragments().len());
        for (ordinal, fragment) in view.fragments().iter().enumerate() {
            if !view.source_eligible(ordinal) {
                paths.push(None);
                continue;
            }
            match self.archive.bounded_fragment_turns(fragment) {
                Ok(turns) => paths.push(Some(turns)),
                Err(ArchiveError::MissingContent | ArchiveError::EvidenceLimitExceeded) => {
                    paths.push(None)
                }
                Err(error) => return Err(error.into()),
            }
        }
        view.turns = Some(paths.into_boxed_slice());
        Ok(Some(view))
    }

    /// Pin a reusable generation snapshot with its validated persisted index or source text.
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
            Some(sources)
                if sources.generation == view.generation.id
                    && sources.source_removal_version == view.source_removal_version() =>
            {
                Arc::clone(sources)
            }
            _ => {
                let sources = Arc::new(ArchiveHybridSources::capture(
                    &self.archive,
                    &view,
                    self.archive_lexical_indexes.get(&profile_id),
                )?);
                cached.insert(profile_id, Arc::clone(&sources));
                sources
            }
        };
        Ok(Some(ArchiveHybridView {
            generation: view.generation,
            dimensions: view.dimensions,
            sources,
            vectors: view.vectors,
            turns: None,
        }))
    }
}

/// Owner-qualified source capture for building a lexical record outside the writer lock.
pub struct ArchiveLexicalIndexInput {
    owner: [u8; 16],
    file_identity: (u64, u64),
    generation: VectorGeneration,
    sources: Arc<ArchiveHybridSources>,
    already_published: bool,
}

pub struct PreparedArchiveLexicalIndex {
    input: ArchiveLexicalIndexInput,
    record: Option<(StoredArchiveLexicalIndex, Vec<u8>)>,
}

impl ArchiveLexicalIndexInput {
    pub fn prepare(self) -> Result<PreparedArchiveLexicalIndex, OwnerSessionError> {
        let record = if self.already_published {
            None
        } else {
            Some(StoredArchiveLexicalIndex::encode(
                self.owner,
                self.generation.id,
                self.sources.source_digest,
                self.sources.fragments_with_text,
                self.sources.index()?,
            ))
        };
        Ok(PreparedArchiveLexicalIndex {
            input: self,
            record,
        })
    }
}

impl RelWriter {
    pub fn archive_lexical_index_input(
        &self,
        profile: CompatibilityProfileId,
    ) -> Result<Option<ArchiveLexicalIndexInput>, OwnerSessionError> {
        self.ensure_write_ready()?;
        let Some(view) = self.session.archive_hybrid_view(profile)? else {
            return Ok(None);
        };
        let already_published = self
            .session
            .archive_lexical_indexes
            .get(&profile)
            .is_some_and(|index| view.sources.matches(index));
        Ok(Some(ArchiveLexicalIndexInput {
            owner: self.owner_id(),
            file_identity: self.session.container_file_identity,
            generation: view.generation,
            sources: view.sources,
            already_published,
        }))
    }

    pub fn publish_archive_lexical_index(
        &mut self,
        prepared: PreparedArchiveLexicalIndex,
    ) -> Result<ArchiveLexicalIndexPublication, OwnerSessionError> {
        self.ensure_write_ready()?;
        let PreparedArchiveLexicalIndex { input, record } = prepared;
        let profile_id = input.generation.compatibility_profile_id;
        if self.session.writer_lease_proof != self.container.writer_lease_proof()
            || self.owner_id() != input.owner
            || self.session.container_file_identity != input.file_identity
            || self
                .session
                .active_compatibility_profile(profile_id)
                .is_none()
            || self.session.generations.current_generation(profile_id) != Some(input.generation)
            || self.session.archive_source_removal_version() != input.sources.source_removal_version
        {
            return Err(OwnerSessionError::ManagementStale);
        }
        let mut report = ArchiveLexicalIndexPublication {
            profile_id,
            generation_id: input.generation.id.0,
            added_bytes: 0,
        };
        if self
            .session
            .archive_lexical_indexes
            .get(&profile_id)
            .is_some_and(|index| input.sources.matches(index))
        {
            return Ok(report);
        }
        let (index, payload) = record.ok_or(OwnerSessionError::ManagementStale)?;
        // The frame carries its own seal. Publish the live entry only after sync.
        let append = self.container.append(&payload).and_then(|frame| {
            self.container.sync()?;
            Ok(frame)
        });
        let frame = match append {
            Ok(frame) => frame,
            Err(error) => {
                self.session.write_poisoned = true;
                return Err(error.into());
            }
        };
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        self.session
            .archive_lexical_indexes
            .insert(profile_id, index);
        report.added_bytes = frame.length + 8;
        Ok(report)
    }
}
