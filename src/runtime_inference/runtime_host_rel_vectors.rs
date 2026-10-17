use super::{
    RelShared, ReliquaryRuntimeHostError, VECTOR_RETRY_POLL, notify_work, operation, stopped,
    wait_for_work, wait_for_work_timeout,
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

fn current_lease(
    shared: &RelShared,
) -> Result<Option<super::LeaseGuard>, ReliquaryRuntimeHostError> {
    shared.admit()
}

pub(super) fn worker_loop(shared: Arc<RelShared>) -> Result<(), ReliquaryRuntimeHostError> {
    let mut seen_epoch = 0_u64;
    let mut active_endpoint: Option<Arc<dyn EmbeddingEndpoint + Send + Sync>> = None;
    let mut active_profile: Option<CompatibilityProfile> = None;
    loop {
        if stopped(&shared.signal)? {
            return Ok(());
        }
        if !shared.background_open()? {
            seen_epoch =
                super::wait_for_work_timeout(&shared.signal, seen_epoch, super::IDLE_POLL)?;
            continue;
        }
        let endpoint = shared
            .routes
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .embedding();
        let Some(endpoint) = endpoint else {
            active_endpoint = None;
            active_profile = None;
            *shared
                .memory_profile
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)? = None;
            seen_epoch = wait_for_work(&shared.signal, seen_epoch)?;
            continue;
        };

        let changed = active_endpoint
            .as_ref()
            .is_none_or(|current| !Arc::ptr_eq(current, &endpoint));
        let candidate = if changed || active_profile.is_none() {
            match profile_from_endpoint(&SharedEmbeddingEndpoint(Arc::clone(&endpoint))) {
                Ok(profile) => Some(profile),
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
            let Some(_lease) = current_lease(&shared)? else {
                return Ok(());
            };
            active_profile = Some(shared.mutate(
                super::super::mutation_events::RelMutationFamily::Retrieval,
                |runtime| {
                    let count = runtime.cva.compatibility_profiles().len();
                    let value = runtime
                        .cva
                        .accept_runtime_compatibility_profile(candidate.clone())
                        .map_err(operation)?;
                    let changed = runtime.cva.compatibility_profiles().len() != count;
                    Ok((
                        value,
                        changed,
                        vec![super::super::events::RelInvalidation::RetrievalProfile(
                            candidate.id,
                        )],
                    ))
                },
            )?);
            active_endpoint = Some(Arc::clone(&endpoint));
            *shared
                .memory_profile
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)? =
                active_profile.as_ref().map(|profile| profile.id);
            drop(_lease);
            notify_work(&shared.signal)?;
        }

        let Some(profile) = active_profile.as_ref() else {
            seen_epoch = wait_for_work_timeout(&shared.signal, seen_epoch, VECTOR_RETRY_POLL)?;
            continue;
        };
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
            let Some(_lease) = current_lease(&shared)? else {
                return Ok(());
            };
            shared.mutate(
                super::super::mutation_events::RelMutationFamily::Retrieval,
                |runtime| {
                    let count = runtime
                        .cva
                        .commit_runtime_memory_vector_batch(batch, vectors)
                        .map_err(operation)?;
                    Ok((
                        (),
                        count > 0,
                        vec![super::super::events::RelInvalidation::RetrievalProfile(
                            profile.id,
                        )],
                    ))
                },
            )?;
            drop(_lease);
            notify_work(&shared.signal)?;
        } else {
            let Some(lease) = current_lease(&shared)? else {
                return Ok(());
            };
            let (archive_batch, progressed) = shared.mutate(
                super::super::mutation_events::RelMutationFamily::Semantic,
                |runtime| {
                    let before = std::fs::metadata(runtime.cva.container.path())
                        .map_err(operation)?
                        .len();
                    let batch = runtime
                        .cva
                        .prepare_semantic_archive_batch(profile.id)
                        .map_err(operation)?;
                    let changed = std::fs::metadata(runtime.cva.container.path())
                        .map_err(operation)?
                        .len()
                        != before;
                    Ok(((batch, changed), changed, Vec::new()))
                },
            )?;
            drop(lease);
            if let Some(batch) = archive_batch {
                let vectors = match endpoint.embed(EmbeddingMode::Document, &batch.texts) {
                    Ok(vectors) => vectors,
                    Err(_) => {
                        seen_epoch =
                            wait_for_work_timeout(&shared.signal, seen_epoch, VECTOR_RETRY_POLL)?;
                        continue;
                    }
                };
                let Some(lease) = current_lease(&shared)? else {
                    return Ok(());
                };
                shared.mutate(
                    super::super::mutation_events::RelMutationFamily::Semantic,
                    |runtime| {
                        runtime
                            .cva
                            .commit_semantic_archive_batch(batch, vectors)
                            .map_err(operation)?;
                        Ok((
                            (),
                            true,
                            vec![super::super::events::RelInvalidation::RetrievalProfile(
                                profile.id,
                            )],
                        ))
                    },
                )?;
                drop(lease);
                notify_work(&shared.signal)?;
            } else if progressed {
                notify_work(&shared.signal)?;
            } else {
                seen_epoch = wait_for_work(&shared.signal, seen_epoch)?;
            }
        }
    }
}
