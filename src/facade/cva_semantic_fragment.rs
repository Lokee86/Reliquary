//! Entry points for owner-backed semantic fragment facts.

use crate::cva_semantic_source::{AcceptedSemanticSource, SemanticSourceReadiness};
use crate::{Cva, CvaError, FragmentId, Memory, VectorGenerationId};

impl Cva {
    pub(crate) fn semantic_fragment_sources(
        &self,
        id: FragmentId,
    ) -> Result<Vec<(String, String)>, CvaError> {
        self.runtime_semantic_fragment_sources(id)
    }

    pub(crate) fn semantic_fragment_position(
        &self,
        id: FragmentId,
    ) -> Result<Option<u64>, CvaError> {
        self.runtime_semantic_fragment_position(id)
    }

    pub(crate) fn semantic_source_excludes_memory(
        &self,
        source: &AcceptedSemanticSource,
        memory: &Memory,
    ) -> Result<bool, CvaError> {
        self.runtime_semantic_source_excludes_memory(source, memory)
    }

    pub(crate) fn prepare_semantic_fragment(
        &self,
        id: FragmentId,
        generation_id: VectorGenerationId,
    ) -> Result<SemanticSourceReadiness, CvaError> {
        self.runtime_prepare_semantic_fragment(id, generation_id)
    }
}
