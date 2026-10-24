use crate::external_index::store::SourceState;
use crate::external_source::{
    ExternalIndexStatus, ExternalMemoryEvidence, ExternalSearchHit, ExternalSourceError,
};
use crate::{
    CompatibilityProfile, CompatibilityProfileId, Cva, Memory, MemoryError, MemoryId, Phylactery,
};

impl Cva {
    pub(crate) fn external_index_state(
        &self,
        namespace: &str,
    ) -> Result<Option<SourceState>, ExternalSourceError> {
        crate::external_source::read_state(&self.external_index, namespace)
    }

    pub(crate) fn external_install(
        &mut self,
        prepared: crate::external_source::index::PreparedIndex,
    ) -> Result<ExternalIndexStatus, ExternalSourceError> {
        if self
            .external_index
            .sources
            .get(&prepared.state.namespace)
            .map(|state| state.revision)
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
        state: &SourceState,
        vector: Option<&[f32]>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ExternalSearchHit>, ExternalSourceError> {
        crate::external_source::search(
            &self.container,
            &self.packed_vectors,
            state,
            vector,
            query,
            limit,
        )
    }

    pub(crate) fn external_profile(
        &self,
        state: &SourceState,
    ) -> Result<CompatibilityProfile, ExternalSourceError> {
        self.compatibility_profile(CompatibilityProfileId(state.profile))
            .map_err(|_| ExternalSourceError::RebuildRequired)
    }

    pub(crate) fn publish_external_window(
        &mut self,
        window: [u8; 32],
        prepared: &[crate::external_source::processing::PreparedExternalMemory],
    ) -> Result<Vec<Memory>, MemoryError> {
        if let Some(existing) = self.memories.external_window(&mut self.container, window)? {
            return Ok(existing);
        }
        let turn = self.rel_turn_count();
        let mut after = self.freshness.clone();
        if let Some(owner) = self.owner_uuid() {
            for memory in prepared {
                after
                    .derive_birth(
                        owner,
                        crate::memory_model::memory_id(&memory.draft.mutation_id),
                        turn,
                        false,
                    )
                    .map_err(MemoryError::Freshness)?;
            }
        }
        let birth_turn = self.owner_uuid().map(|_| turn);
        let results = crate::external_source::publication::publish(
            &mut self.container,
            &mut self.memories,
            window,
            prepared,
            birth_turn,
        )?;
        self.freshness = after;
        for memory in &results {
            self.dream_freshness.derive_publication_birth(memory.id);
        }
        Ok(results)
    }
}

impl Cva {
    pub fn external_memory_evidence(
        &self,
        id: MemoryId,
    ) -> Result<Option<ExternalMemoryEvidence>, ExternalSourceError> {
        let body = self
            .memories
            .current_body_id(id)
            .map_err(|_| ExternalSourceError::Unavailable)?;
        Ok(self.memories.external_evidence.get(&(id, body)).cloned())
    }
}

impl Phylactery {
    pub fn external_memory_evidence(
        &self,
        id: MemoryId,
    ) -> Result<Option<ExternalMemoryEvidence>, ExternalSourceError> {
        let body = self
            .memories
            .current_body_id(id)
            .map_err(|_| ExternalSourceError::Unavailable)?;
        Ok(self.memories.external_evidence.get(&(id, body)).cloned())
    }
}
