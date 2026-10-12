use super::{
    ReliquaryRuntimeHostError, RuntimeMemoryProfiles, Shared, VECTOR_RETRY_POLL, notify_work,
    operation, stopped, wait_for_work, wait_for_work_timeout,
};
use crate::compatibility_profile_probe::profile_from_endpoint;
use crate::{
    CompatibilityProfile, EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode,
    VectorNormalization,
};
use std::sync::Arc;

#[derive(Clone)]
struct SharedEmbeddingEndpoint(Arc<dyn EmbeddingEndpoint + Send + Sync>);

impl EmbeddingEndpoint for SharedEmbeddingEndpoint {
    fn dimensions(&self) -> u32 {
        self.0.dimensions()
    }

    fn normalization(&self) -> VectorNormalization {
        self.0.normalization()
    }

    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.0.embed(mode, inputs)
    }
}

pub(super) fn worker_loop(shared: Arc<Shared>) -> Result<(), ReliquaryRuntimeHostError> {
    let mut seen_epoch = 0_u64;
    let mut active_endpoint: Option<Arc<dyn EmbeddingEndpoint + Send + Sync>> = None;
    let mut project_profile: Option<CompatibilityProfile> = None;
    let mut user_profile: Option<CompatibilityProfile> = None;
    loop {
        if stopped(&shared.signal)? {
            return Ok(());
        }
        let endpoint = shared
            .routes
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .embedding();
        let Some(endpoint) = endpoint else {
            active_endpoint = None;
            project_profile = None;
            user_profile = None;
            *shared
                .memory_profiles
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)? =
                RuntimeMemoryProfiles::default();
            seen_epoch = wait_for_work(&shared.signal, seen_epoch)?;
            continue;
        };
        let changed = active_endpoint
            .as_ref()
            .is_none_or(|current| !Arc::ptr_eq(current, &endpoint));
        let candidate = if changed || user_profile.is_none() {
            match profile_from_endpoint(&SharedEmbeddingEndpoint(Arc::clone(&endpoint))) {
                Ok(candidate) => Some(candidate),
                Err(_) => {
                    seen_epoch =
                        wait_for_work_timeout(&shared.signal, seen_epoch, VECTOR_RETRY_POLL)?;
                    continue;
                }
            }
        } else {
            None
        };
        if changed {
            let candidate = candidate.as_ref().expect("changed endpoint has profile");
            project_profile = Some({
                let mut runtime = shared
                    .runtime
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                runtime
                    .cva
                    .accept_runtime_compatibility_profile(candidate.clone())
                    .map_err(operation)?
            });
            active_endpoint = Some(Arc::clone(&endpoint));
        }
        let has_phylactery = shared.phylactery_active()
            && shared
                .phylactery
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .is_some();
        if has_phylactery && user_profile.is_none() {
            let candidate = candidate.as_ref().ok_or_else(|| {
                ReliquaryRuntimeHostError::Operation("embedding profile is unavailable".into())
            })?;
            user_profile = {
                let mut phylactery = shared
                    .phylactery
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                phylactery
                    .as_mut()
                    .map(|phy| phy.accept_runtime_compatibility_profile(candidate.clone()))
                    .transpose()
                    .map_err(operation)?
            };
        } else if !has_phylactery {
            user_profile = None;
        }
        let next_profiles = RuntimeMemoryProfiles {
            project: project_profile.as_ref().map(|profile| profile.id),
            user: user_profile.as_ref().map(|profile| profile.id),
        };
        let mut published_profiles = shared
            .memory_profiles
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if published_profiles.project != next_profiles.project
            || published_profiles.user != next_profiles.user
        {
            *published_profiles = next_profiles;
            drop(published_profiles);
            notify_work(&shared.signal)?;
        }

        let mut did_work = false;
        if let Some(profile) = &project_profile {
            let batch = {
                let mut runtime = shared
                    .runtime
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                runtime
                    .cva
                    .prepare_runtime_memory_vector_batch(profile.id)
                    .map_err(operation)?
            };
            if let Some(batch) = batch {
                let vectors = match endpoint.embed(EmbeddingMode::Document, &batch.texts) {
                    Ok(vectors) => vectors,
                    Err(_) => {
                        seen_epoch =
                            wait_for_work_timeout(&shared.signal, seen_epoch, VECTOR_RETRY_POLL)?;
                        continue;
                    }
                };
                let mut runtime = shared
                    .runtime
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                runtime
                    .cva
                    .commit_runtime_memory_vector_batch(batch, vectors)
                    .map_err(operation)?;
                did_work = true;
            }
        }

        if let Some(profile) = &project_profile {
            let (batch, progressed) = {
                let mut runtime = shared
                    .runtime
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                let before = std::fs::metadata(runtime.cva.container.path())
                    .map_err(operation)?
                    .len();
                let batch = runtime
                    .cva
                    .prepare_semantic_archive_batch(profile.id)
                    .map_err(operation)?;
                let progressed = std::fs::metadata(runtime.cva.container.path())
                    .map_err(operation)?
                    .len()
                    != before;
                (batch, progressed)
            };
            did_work |= progressed;
            if let Some(batch) = batch {
                let vectors = match endpoint.embed(EmbeddingMode::Document, &batch.texts) {
                    Ok(vectors) => vectors,
                    Err(_) => {
                        seen_epoch =
                            wait_for_work_timeout(&shared.signal, seen_epoch, VECTOR_RETRY_POLL)?;
                        continue;
                    }
                };
                let mut runtime = shared
                    .runtime
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                runtime
                    .cva
                    .commit_semantic_archive_batch(batch, vectors)
                    .map_err(operation)?;
                did_work = true;
            }
        }

        if shared.phylactery_active()
            && let Some(profile) = &user_profile
        {
            let batch = {
                let mut phylactery = shared
                    .phylactery
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                match phylactery.as_mut() {
                    Some(phy) => phy
                        .prepare_runtime_memory_vector_batch(profile.id)
                        .map_err(operation)?,
                    None => None,
                }
            };
            if let Some(batch) = batch {
                let vectors = match endpoint.embed(EmbeddingMode::Document, &batch.texts) {
                    Ok(vectors) => vectors,
                    Err(_) => {
                        seen_epoch =
                            wait_for_work_timeout(&shared.signal, seen_epoch, VECTOR_RETRY_POLL)?;
                        continue;
                    }
                };
                if shared.phylactery_active() {
                    let mut phylactery = shared
                        .phylactery
                        .lock()
                        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                    if let Some(phy) = phylactery.as_mut() {
                        phy.commit_runtime_memory_vector_batch(batch, vectors)
                            .map_err(operation)?;
                        did_work = true;
                    }
                }
            }
        }

        if did_work {
            notify_work(&shared.signal)?;
        } else {
            seen_epoch = wait_for_work(&shared.signal, seen_epoch)?;
        }
    }
}
