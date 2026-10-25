use crate::{CompatibilityProfileId, Cva, CvaError, EmbeddingEndpoint};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedVectorRecovery {
    pub compatibility_profile_id: CompatibilityProfileId,
    pub memory_vectors_embedded: usize,
    pub archive_generation_rebuilt: bool,
}

impl Cva {
    pub fn rebuild_derived_vectors(
        &mut self,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<DerivedVectorRecovery, CvaError> {
        let (compatibility_profile_id, memory_vectors_embedded, archive_generation_rebuilt) =
            self.runtime_rebuild_derived_vectors(endpoint)?;
        Ok(DerivedVectorRecovery {
            compatibility_profile_id,
            memory_vectors_embedded,
            archive_generation_rebuilt,
        })
    }
}
