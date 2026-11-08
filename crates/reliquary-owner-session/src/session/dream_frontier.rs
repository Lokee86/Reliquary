//! Bind the initial candidate frontier before any model calls start.
use super::*;
use crate::DreamMemoryCut;
fn validate(
    common: &CommonState,
    source: DreamMemoryCut,
    candidates: &[DreamMemoryCut],
) -> Result<(), OwnerSessionError> {
    if candidates.len() > 16 {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    super::dream_no_link::validate_memory(common, source)?;
    for c in candidates {
        super::dream_no_link::validate_memory(common, *c)?;
        if common.dream_pairs.contains(source.id, c.id) {
            return Err(OwnerSessionError::StaleDreamCut);
        }
    }
    Ok(())
}
impl RelWriter {
    pub fn begin_initial_dream_frontier(
        &mut self,
        pass: String,
        source: DreamMemoryCut,
        candidates: Vec<DreamMemoryCut>,
        turn: u64,
    ) -> Result<DreamBeginAdmission, OwnerSessionError> {
        self.ensure_write_ready()?;
        self.begin_initial_dream_frontier_after_fence(pass, source, candidates, turn)
    }

    pub fn begin_native_import_dream_frontier(
        &mut self,
        import_id: &str,
        pass: String,
        source: DreamMemoryCut,
        candidates: Vec<DreamMemoryCut>,
        turn: u64,
    ) -> Result<DreamBeginAdmission, OwnerSessionError> {
        self.validate_native_import_memory(import_id, source.id)?;
        self.validate_native_import_postprocessing(import_id)?;
        if turn != self.session.archive.rel_turn_count()
            || pass != Self::native_import_dream_identity(import_id, source.id).0
            || self.session.common.dreams.has_other_pending_frontier(
                &super::native_import::native_import_dream_prefix(import_id),
                &pass,
            )
        {
            return Err(OwnerSessionError::StaleDreamCut);
        }
        if self.initial_dream_frontier(&pass).is_none()
            && candidates
                .iter()
                .any(|candidate| !self.native_import_dream_candidate_ready(import_id, candidate.id))
        {
            return Err(OwnerSessionError::StaleDreamCut);
        }
        self.begin_initial_dream_frontier_after_fence(pass, source, candidates, turn)
    }

    fn begin_initial_dream_frontier_after_fence(
        &mut self,
        pass: String,
        source: DreamMemoryCut,
        candidates: Vec<DreamMemoryCut>,
        turn: u64,
    ) -> Result<DreamBeginAdmission, OwnerSessionError> {
        if self.container.writer_lease_proof().is_none()
            || self.container.writer_lease_proof() != self.session.writer_lease_proof
        {
            return Err(OwnerSessionError::StaleDreamCut);
        }
        if self.session.common.dreams.initial_frontier(&pass).is_none() {
            validate(&self.session.common, source, &candidates)?;
        }
        let admission = super::dream_empty::initial_admission(
            &self.container,
            &self.session.common,
            source,
            pass,
            turn,
        )?;
        let result = self.session.common.dreams.append_frontier(
            &mut self.container,
            admission,
            source,
            candidates,
            self.session.common.graph.graph_version(),
            self.session.common.memories.memory_version(),
        );
        match result {
            Ok(value) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(value)
            }
            Err(e) => {
                self.session.write_poisoned = true;
                Err(OwnerSessionError::Record(e))
            }
        }
    }
}
impl PhyWriter {
    pub fn begin_initial_dream_frontier(
        &mut self,
        pass: String,
        source: DreamMemoryCut,
        candidates: Vec<DreamMemoryCut>,
        turn: u64,
    ) -> Result<DreamBeginAdmission, OwnerSessionError> {
        if self.poisoned
            || self.container.writer_lease_proof().is_none()
            || self.container.writer_lease_proof() != self.writer_lease_proof
        {
            return Err(OwnerSessionError::StaleDreamCut);
        }
        if self.common.dreams.initial_frontier(&pass).is_none() {
            validate(&self.common, source, &candidates)?;
        }
        let admission = super::dream_empty::initial_admission(
            &self.container,
            &self.common,
            source,
            pass,
            turn,
        )?;
        let result = self.common.dreams.append_frontier(
            &mut self.container,
            admission,
            source,
            candidates,
            self.common.graph.graph_version(),
            self.common.memories.memory_version(),
        );
        match result {
            Ok(value) => {
                self.writer_lease_proof = self.container.writer_lease_proof();
                Ok(value)
            }
            Err(e) => {
                self.poisoned = true;
                Err(OwnerSessionError::Record(e))
            }
        }
    }
}

impl RelWriter {
    pub fn initial_dream_frontier(&self, pass: &str) -> Option<crate::InitialDreamFrontier> {
        self.session.common.dreams.initial_frontier(pass)
    }
    pub fn dream_graph_version(&self) -> u64 {
        self.session.common.graph.graph_version()
    }

    /// True when the pinned frontier has a terminal receipt for every pair.
    pub fn initial_dream_frontier_evaluated(&self, pass: &str) -> bool {
        self.initial_dream_frontier(pass).is_some_and(|frontier| {
            frontier.candidates.iter().all(|candidate| {
                self.session
                    .common
                    .dream_pairs
                    .contains(frontier.source.id, candidate.id)
            })
        })
    }

    pub fn dream_batch_published(&self, publication_id: [u8; 32]) -> bool {
        self.session
            .common
            .dream_pairs
            .batch(publication_id)
            .is_some()
    }

    pub fn initial_dream_settled(&self, pass: &str) -> bool {
        self.session
            .common
            .dreams
            .initial_settlement(pass)
            .is_some()
    }
}
impl PhyWriter {
    pub fn dream_batch_published(&self, publication_id: [u8; 32]) -> bool {
        self.common.dream_pairs.batch(publication_id).is_some()
    }
    pub fn dream_graph_version(&self) -> u64 {
        self.common.graph.graph_version()
    }
}
