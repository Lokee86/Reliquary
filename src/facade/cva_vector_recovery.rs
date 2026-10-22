use crate::compatibility_profile_probe::profile_from_endpoint;
use crate::{
    CompatibilityProfileId, Cva, CvaError, EmbeddingEndpoint, EmbeddingMode, MemoryVectorError,
};

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
        let candidate = profile_from_endpoint(endpoint)?;
        let profile = self.accept_runtime_compatibility_profile(candidate)?;

        let memory_vectors_embedded = match self.prepare_runtime_memory_vector_batch(profile.id)? {
            Some(batch) => {
                let vectors = endpoint
                    .embed(EmbeddingMode::Document, &batch.texts)
                    .map_err(MemoryVectorError::from)?;
                self.commit_runtime_memory_vector_batch(batch, vectors)?
            }
            None => 0,
        };

        let archive_generation_rebuilt = if self.fragments().is_empty() {
            false
        } else {
            let archive_version = self.archive_version();
            let rebuild = self
                .current_vector_generation(profile.id)
                .is_none_or(|generation| generation.source_archive_version != archive_version);
            if rebuild {
                self.build_archive_vector_generation_verified(profile.clone(), endpoint)?;
            }
            rebuild
        };

        self.sync()?;
        Ok(DerivedVectorRecovery {
            compatibility_profile_id: profile.id,
            memory_vectors_embedded,
            archive_generation_rebuilt,
        })
    }
}
