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
                    let count = runtime
                        .cva
                        .runtime_active_memory_compatibility_profiles()
                        .len();
                    let value = crate::owner_lifecycle::vector_publication::accept_profile(
                        &mut runtime.cva.owner.container,
                        &mut runtime.cva.owner.compatibility_profiles,
                        candidate.clone(),
                    )
                    .map_err(operation)?;
                    let changed = runtime
                        .cva
                        .runtime_active_memory_compatibility_profiles()
                        .len()
                        != count;
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
            crate::vector_publication::prepare_batch(
                &crate::memory_store::MemoryReadView::new(
                    &runtime.cva.memories,
                    &runtime.cva.container,
                ),
                &runtime.cva.compatibility_profiles,
                &runtime.cva.memory_vectors,
                profile.id,
            )
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
                    let plan = crate::runtime_vector_step_policy::publication_plan(
                        &runtime.cva.memories,
                        &runtime.cva.compatibility_profiles,
                        &runtime.cva.memory_vectors,
                        batch,
                        vectors,
                    )
                    .map_err(operation)?;
                    let count = match plan {
                        Some(plan) => {
                            crate::owner_lifecycle::vector_publication::publish_memory_vectors(
                                &mut runtime.cva.owner.container,
                                &runtime.cva.owner.compatibility_profiles,
                                &mut runtime.cva.owner.packed_vectors,
                                &mut runtime.cva.owner.memory_vectors,
                                plan,
                            )
                            .map_err(operation)?
                        }
                        None => 0,
                    };
                    Ok((
                        (),
                        count > 0,
                        vec![super::super::events::RelInvalidation::RetrievalProfile(
                            profile.id,
                        )],
                    ))
                },
            )?;
            let active = shared
                .runtime
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .cva
                .active_memory_compatibility_profile(profile.id)
                .map(|profile| profile.id);
            *shared
                .memory_profile
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)? = active;
            drop(_lease);
            notify_work(&shared.signal)?;
        } else {
            let active = shared
                .runtime
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .cva
                .active_memory_compatibility_profile(profile.id)
                .map(|profile| profile.id);
            *shared
                .memory_profile
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)? = active;
            let Some(lease) = current_lease(&shared)? else {
                return Ok(());
            };
            let (archive_batch, progressed) = shared.mutate(
                super::super::mutation_events::RelMutationFamily::Semantic,
                |runtime| {
                    let before = runtime.cva.owner.backing_len().map_err(operation)?;
                    if let Some(generation) =
                        runtime.cva.runtime_current_vector_generation(profile.id)
                    {
                        runtime
                            .cva
                            .process_semantic_freshness(generation.id, 64)
                            .map_err(operation)?;
                    }
                    let cva = &runtime.cva;
                    let read =
                        crate::owner_lifecycle::semantic_archive_read::SemanticArchiveRead::new(
                            crate::owner_lifecycle::semantic_source::SemanticSourceReadParts {
                                container: &cva.container,
                                archive: &cva.archive,
                                freshness: &cva.freshness,
                                generations: &cva.vector_generations,
                                compatibility_profiles: &cva.compatibility_profiles,
                                archive_vectors: &cva.archive_vectors,
                                packed_vectors: &cva.packed_vectors,
                            },
                        );
                    let batch = crate::semantic_archive_batch::prepare_batch(
                        &read,
                        profile.id,
                        cva.current_vector_generation(profile.id),
                    )
                    .map_err(operation)?;
                    let changed = runtime.cva.owner.backing_len().map_err(operation)? != before;
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
                        let current=runtime.cva.runtime_current_vector_generation(batch.profile.id);
                        if let Some(plan)=crate::semantic_archive_batch::pack_batch(batch,vectors,current).map_err(operation)?{
                            let cva=&mut runtime.cva;
                            let generation=crate::owner_lifecycle::vector_publication::publish_semantic_archive(&mut cva.owner.container,&cva.owner.archive,&cva.owner.compatibility_profiles,&mut cva.owner.packed_vectors,&mut cva.owner.archive_vectors,&mut cva.owner.vector_generations,plan).map_err(operation)?;
                            if cva.owner_uuid().is_some(){cva.process_semantic_freshness(generation.id,64).map_err(|e|operation(crate::VectorGenerationError::Freshness(e.to_string())))?;}
                        }
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
