use super::{WorkerShared, operation};
use crate::compatibility_profile_probe::profile_from_endpoint;
use crate::{EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, VectorNormalization};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
struct SharedEmbedding(Arc<dyn EmbeddingEndpoint + Send + Sync>);
impl EmbeddingEndpoint for SharedEmbedding {
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

pub(super) fn worker_loop(
    shared: Arc<WorkerShared>,
) -> Result<(), super::ReliquaryRuntimeHostError> {
    let mut seen = 0;
    let mut endpoint_active: Option<Arc<dyn EmbeddingEndpoint + Send + Sync>> = None;
    let mut profile: Option<crate::CompatibilityProfile> = None;
    loop {
        if shared.stopped()? {
            return Ok(());
        }
        if !shared.admission_open()? {
            shared.wait(&mut seen, super::IDLE_POLL)?;
            continue;
        }
        let endpoint = shared
            .routes
            .read()
            .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?
            .embedding();
        let Some(endpoint) = endpoint else {
            endpoint_active = None;
            profile = None;
            *shared
                .profile
                .lock()
                .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)? = None;
            shared.wait(&mut seen, super::IDLE_POLL)?;
            continue;
        };
        let changed = endpoint_active
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, &endpoint));
        let candidate = if changed || profile.is_none() {
            match profile_from_endpoint(&SharedEmbedding(Arc::clone(&endpoint))) {
                Ok(p) => Some(p),
                Err(_) => {
                    shared.wait(&mut seen, Duration::from_secs(60))?;
                    continue;
                }
            }
        } else {
            None
        };
        if changed {
            let Some(_lease) = shared.lease()? else {
                return Ok(());
            };
            if shared.stopped()? {
                return Ok(());
            }
            profile = Some(
                shared
                    .owner
                    .lock()
                    .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?
                    .accept_runtime_compatibility_profile(
                        candidate.as_ref().expect("profile candidate").clone(),
                    )
                    .map_err(operation)?,
            );
            endpoint_active = Some(Arc::clone(&endpoint));
        }
        if profile.is_none() {
            profile = shared
                .profile
                .lock()
                .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?
                .clone();
        }
        if let Some(p) = profile.as_ref() {
            let mut published = shared
                .profile
                .lock()
                .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
            let changed = published.as_ref().map(|v| v.id) != Some(p.id);
            if changed {
                *published = Some(p.clone());
            }
            drop(published);
            if changed {
                shared.wake()?;
            }
            let batch = shared
                .owner
                .lock()
                .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?
                .prepare_runtime_memory_vector_batch(p.id)
                .map_err(operation)?;
            if let Some(batch) = batch {
                let vectors = match endpoint.embed(EmbeddingMode::Document, &batch.texts) {
                    Ok(value) => value,
                    Err(_) => {
                        shared.wait(&mut seen, Duration::from_secs(60))?;
                        continue;
                    }
                };
                if shared.stopped()? {
                    return Ok(());
                }
                let Some(_lease) = shared.lease()? else {
                    return Ok(());
                };
                if shared.stopped()? {
                    return Ok(());
                }
                shared
                    .owner
                    .lock()
                    .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?
                    .commit_runtime_memory_vector_batch(batch, vectors)
                    .map_err(operation)?;
                shared.wake()?;
                continue;
            }
        }
        shared.wait(&mut seen, super::IDLE_POLL)?;
    }
}
