use crate::external_index::model::*;

pub use crate::memory_model::ExternalMemoryEvidence;

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

/// Identifier-only evidence; no Episode, Archive node or source quote is stored.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceEvidenceRef {
    Reliquary(crate::MemorySourceRef),
    External(ExternalMemoryEvidence),
}
