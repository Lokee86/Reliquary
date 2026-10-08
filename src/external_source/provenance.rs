use super::model::*;
use serde::{Deserialize, Serialize};

/// Identifier-only evidence; no Episode, Archive node or source quote is stored.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalMemoryEvidence {
    pub source_owner_id: String,
    pub principal_id: String,
    pub processing_window: [u8; 32],
    pub authority: ExternalChunkRef,
    pub content: Option<ExternalChunkRef>,
    pub grounding: Option<ExternalChunkRef>,
}
impl ExternalMemoryEvidence {
    pub(crate) fn validate(&self) -> Result<(), ExternalSourceError> {
        identifier(&self.source_owner_id)?;
        identifier(&self.principal_id)?;
        let namespace = &self.authority.message.source_namespace;
        for chunk in std::iter::once(&self.authority)
            .chain(self.content.iter())
            .chain(self.grounding.iter())
        {
            chunk.message.validate()?;
            if chunk.message.source_namespace != *namespace
                || chunk.start >= chunk.end
                || chunk.end > MAX_EXTERNAL_MESSAGE_BYTES as u64
            {
                return Err(ExternalSourceError::InvalidContract);
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceEvidenceRef {
    Reliquary(crate::MemorySourceRef),
    External(ExternalMemoryEvidence),
}
impl crate::Cva {
    pub fn external_memory_evidence(
        &self,
        id: crate::MemoryId,
    ) -> Result<Option<ExternalMemoryEvidence>, ExternalSourceError> {
        let body = self
            .memories
            .current_body_id(id)
            .map_err(|_| ExternalSourceError::Unavailable)?;
        Ok(self.memories.external_evidence.get(&(id, body)).cloned())
    }
}
impl crate::Phylactery {
    pub fn external_memory_evidence(
        &self,
        id: crate::MemoryId,
    ) -> Result<Option<ExternalMemoryEvidence>, ExternalSourceError> {
        let body = self
            .memories
            .current_body_id(id)
            .map_err(|_| ExternalSourceError::Unavailable)?;
        Ok(self.memories.external_evidence.get(&(id, body)).cloned())
    }
}
