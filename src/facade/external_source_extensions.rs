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
        self.native_external_memory_evidence(id)
    }
}
