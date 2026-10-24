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
        let profile = crate::owner_lifecycle::vector_publication::accept_profile(
            &mut self.container,
            &mut self.compatibility_profiles,
            candidate,
        )?;

        let batch = crate::vector_publication::prepare_batch(
            &crate::memory_store::MemoryReadView::new(&self.memories, &self.container),
            &self.compatibility_profiles,
            &self.memory_vectors,
            profile.id,
        )?;
        let memory_vectors_embedded = match batch {
            Some(batch) => {
                let vectors = endpoint
                    .embed(EmbeddingMode::Document, &batch.texts)
                    .map_err(MemoryVectorError::from)?;
                {
                    let plan = crate::runtime_vector_step_policy::publication_plan(
                        &self.memories,
                        &self.compatibility_profiles,
                        &self.memory_vectors,
                        batch,
                        vectors,
                    )?;
                    match plan {
                        Some(plan) => {
                            crate::owner_lifecycle::vector_publication::publish_memory_vectors(
                                &mut self.container,
                                &self.compatibility_profiles,
                                &mut self.packed_vectors,
                                &mut self.memory_vectors,
                                plan,
                            )?
                        }
                        None => 0,
                    }
                }?
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
