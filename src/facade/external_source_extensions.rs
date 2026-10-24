use crate::external_source::{ExternalMemoryEvidence, ExternalSourceError};
use crate::{Cva, MemoryId, Phylactery};

impl Cva {
    pub fn external_memory_evidence(
        &self,
        id: MemoryId,
    ) -> Result<Option<ExternalMemoryEvidence>, ExternalSourceError> {
        self.runtime_external_memory_evidence(id)
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
