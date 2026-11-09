//! Execute Application-declared stages against authenticated mounted owners.
use crate::{
    GatewayError, Instance, OwnerId, OwnerSessionError, ProcessingState,
    insomnia_worker::{SharedEndpoint, now_ns},
};
use reliquary_application::{
    DreamClassifier, DreamVerificationPolicy, DreamVerifier, SwitchboardEmbeddingEndpoint,
    VectorWorkflowError,
    import_entity_work::accepted_import_entity_work,
    native_import_workflow::{NativeImportDreamCandidates, NativeImportPostprocessingPlan},
    prepare_compatibility_profile,
    processing_plan::{
        ImportStage, ImportStageOutcome, ImportStageRun, StageState, coordinate_import_stages,
    },
    verify_compatibility_profile,
};
use reliquary_owner_session::{InsomniaWorkState, model::MemoryId};
use reliquary_switchboard::{EmbeddingEndpoint, GeneralEndpoint};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const VECTOR_BATCH: usize = 32;

pub(crate) struct ImportProcessingRoutes {
    pub dream: Option<Arc<dyn GeneralEndpoint>>,
    pub entity: Option<Arc<dyn GeneralEndpoint>>,
    pub embedding: Option<Arc<dyn EmbeddingEndpoint>>,
}

pub(crate) fn run_import_stages(
    instance: &Instance,
    owner: OwnerId,
    phy: Option<OwnerId>,
    import_id: &str,
    routes: ImportProcessingRoutes,
    stop: &AtomicBool,
    mut report: impl FnMut(ImportStage, StageState, usize),
) -> Result<ProcessingState, GatewayError> {
    let stopped = || stop.load(Ordering::Acquire);
    if stopped() {
        return Ok(ProcessingState::Stopped);
    }
    let recovering_seal = instance.with_writable_reliquary(owner, |session| {
        session.native_import_settlement(import_id).is_some()
    })?;
    let Some(embedding) = routes.embedding else {
        report(ImportStage::Vectors, StageState::NotConfigured, 0);
        return Ok(ProcessingState::Incomplete);
    };
    if recovering_seal {
        return verify_import_seal(instance, owner, phy, import_id, embedding, report);
    }
    let mut plan = NativeImportPostprocessingPlan::default();
    let dream = routes.dream.map(SharedEndpoint);
    let entity = routes.entity.map(SharedEndpoint);
    let embedding = SwitchboardEmbeddingEndpoint::new(embedding);
    let mut routed = BTreeSet::<MemoryId>::new();
    let mut profile = None;
    let result = coordinate_import_stages(
        stopped,
        |stage| {
            let result = match stage {
                ImportStage::Entity => (|| {
                    let episodes = instance.with_writable_reliquary(owner, |session| {
                        let mut episodes = Vec::new();
                        session.scoped_insomnia_status(Some(import_id), now_ns(), |work| {
                            if work.state == InsomniaWorkState::Complete {
                                episodes.push(work.id);
                            }
                        })?;
                        Ok::<_, reliquary_owner_session::OwnerSessionError>(episodes)
                    })??;
                    let mut count = 0;
                    for episode in episodes {
                        if stopped() {
                            break;
                        }
                        let work = instance.with_rel_writer(owner, |writer| {
                            let checkpoint =
                                writer.completed_import_extraction(import_id, episode)?;
                            accepted_import_entity_work(
                                writer.read_session(),
                                checkpoint,
                                phy.map(OwnerId::as_bytes),
                            )
                        })??;
                        routed.extend(work.routed_memories);
                        for mention in work.mentions {
                            if stopped() {
                                break;
                            }
                            let local = mention.owner_id == owner.as_bytes();
                            let endpoint =
                                entity.as_ref().ok_or(GatewayError::RetrievalUnavailable(
                                    "Entity processing requires a configured route",
                                ))?;
                            let result = if local {
                                instance.resolve_native_import_entity_mention(
                                    import_id,
                                    mention,
                                    endpoint.clone(),
                                    now_ns(),
                                )?
                            } else {
                                instance.resolve_and_publish_entity_mention(
                                    mention,
                                    endpoint.clone(),
                                    now_ns(),
                                )?
                            };
                            if local {
                                plan.entity_mentions.push(result.key);
                            }
                            count += 1;
                        }
                    }
                    Ok(count)
                })(),
                ImportStage::Relationship => {
                    // Relationships require a typed source-authorized proposal.
                    return Ok(ImportStageOutcome::NotConfigured);
                }
                ImportStage::Vectors => (|| {
                    let id = instance
                        .publish_native_import_embedding_profile(owner, import_id, &embedding)?;
                    let missing =
                        instance.native_import_missing_memory_vectors(owner, import_id, id)?;
                    for ids in missing.chunks(VECTOR_BATCH) {
                        if stopped() {
                            break;
                        }
                        instance.publish_native_import_memory_vectors(
                            owner, import_id, id, ids, &embedding,
                        )?;
                    }
                    profile = Some(id);
                    plan.memory_vector_profiles.push(id);
                    if let Some(phy) = phy.filter(|_| !routed.is_empty() && !stopped()) {
                        instance.authorize_routed_owners(owner, phy)?;
                        let accepted = instance
                            .with_writable_reliquary(owner, |rel| {
                                rel.active_compatibility_profile(id).cloned()
                            })?
                            .ok_or(GatewayError::RetrievalUnavailable(
                                "REL embedding profile changed",
                            ))?;
                        let phy_profile = instance.with_entity_phy_writer(phy, |writer| {
                            writer.publish_compatibility_profile(accepted)
                        })??;
                        let ids = routed.iter().copied().collect::<Vec<_>>();
                        for ids in ids.chunks(VECTOR_BATCH) {
                            if stopped() {
                                break;
                            }
                            let prepared = instance.prepare_owner_memory_vectors(
                                phy,
                                phy_profile,
                                ids,
                                &embedding,
                            )?;
                            instance.publish_owner_memory_vectors(&prepared)?;
                        }
                    }
                    Ok(missing.len() + routed.len())
                })(),
                ImportStage::Dream => (|| {
                    let profile = profile.ok_or_else(|| {
                        GatewayError::HostAdapter(
                            "Dream requires a prepared embedding profile".into(),
                        )
                    })?;
                    let memories = instance.native_import_memory_ids(owner, import_id)?;
                    if memories.is_empty() {
                        return Ok(Some(0));
                    }
                    let endpoint = dream.as_ref().ok_or(GatewayError::RetrievalUnavailable(
                        "Dream processing requires a configured route",
                    ))?;
                    let classifier = DreamClassifier::new(endpoint.clone());
                    let verifier = DreamVerifier::new(endpoint.clone());
                    for &source in &memories {
                        if stopped() {
                            break;
                        }
                        let complete = instance.process_native_import_dream(
                            owner,
                            import_id,
                            source,
                            NativeImportDreamCandidates::Ranked {
                                profile,
                                config: Default::default(),
                            },
                            &classifier,
                            Some(&verifier),
                            DreamVerificationPolicy::default(),
                            now_ns(),
                        )?;
                        if !complete {
                            return Ok(None);
                        }
                    }
                    Ok(Some(memories.len()))
                })()
                .and_then(|result| result.ok_or(GatewayError::HostStatusUnknown)),
                ImportStage::HistoricalFreshness => instance
                    .finish_native_import_postprocessing(owner, import_id, &plan, now_ns())
                    .map(|_| 1),
            };
            match result {
                Ok(count) => Ok(ImportStageOutcome::Complete(count)),
                Err(GatewayError::HostStatusUnknown) => Ok(ImportStageOutcome::Held),
                Err(error) => Err(error),
            }
        },
        report,
    )?;
    Ok(match result {
        ImportStageRun::Ready => ProcessingState::Ready,
        ImportStageRun::Incomplete => ProcessingState::Incomplete,
        ImportStageRun::Held => ProcessingState::Held,
        ImportStageRun::Stopped => ProcessingState::Stopped,
    })
}

pub(crate) fn verify_import_seal(
    instance: &Instance,
    owner: OwnerId,
    phy: Option<OwnerId>,
    import_id: &str,
    embedding: Arc<dyn EmbeddingEndpoint>,
    mut report: impl FnMut(ImportStage, StageState, usize),
) -> Result<ProcessingState, GatewayError> {
    let bounded = embedding.supports_bounded_embedding();
    let endpoint = SwitchboardEmbeddingEndpoint::new(embedding);
    let accepted = instance.with_writable_reliquary(owner, |session| {
        session.native_import_processing_profile(import_id)
    })??;
    let probe_failed =
        || GatewayError::RetrievalUnavailable("configured embedding profile could not be verified");
    let profile = if let Some(accepted) = accepted {
        verify_compatibility_profile(&accepted, &endpoint).map_err(|error| match error {
            VectorWorkflowError::IncompatibleEndpoint if bounded => {
                OwnerSessionError::ManagementConflict.into()
            }
            _ => probe_failed(),
        })?;
        accepted.id
    } else {
        // Seals written before the processing binding still require an exact active profile.
        prepare_compatibility_profile(&endpoint)
            .map_err(|_| probe_failed())?
            .id
    };
    if !instance
        .native_import_missing_memory_vectors(owner, import_id, profile)?
        .is_empty()
    {
        return Err(GatewayError::RetrievalUnavailable(
            "sealed REL vector coverage is incomplete",
        ));
    }
    if let Some(phy) = phy {
        let routed = instance.native_import_routed_memory_ids(owner, import_id, phy)?;
        for ids in routed.chunks(VECTOR_BATCH) {
            let has_coverage = instance.with_entity_phy_writer(phy, |writer| {
                writer
                    .memory_vector_input(profile, ids)
                    .map(|input| input.existing().iter().all(Option::is_some))
            })??;
            if !has_coverage {
                return Err(GatewayError::RetrievalUnavailable(
                    "sealed PHY vector coverage is incomplete",
                ));
            }
        }
    }
    let complete = instance.with_rel_writer(owner, |writer| {
        Ok::<_, OwnerSessionError>(writer.native_import_memory_ids(import_id)?.into_iter().all(
            |id| {
                let (pass, _) = crate::RelWriter::native_import_dream_identity(import_id, id);
                writer.initial_dream_settled(&pass)
            },
        ))
    })??;
    if !complete {
        return Err(GatewayError::RetrievalUnavailable(
            "sealed import Dream coverage is incomplete",
        ));
    }
    let plan = NativeImportPostprocessingPlan {
        memory_vector_profiles: vec![profile],
        ..NativeImportPostprocessingPlan::default()
    };
    instance.finish_native_import_postprocessing(owner, import_id, &plan, now_ns())?;
    for stage in [
        ImportStage::Entity,
        ImportStage::Vectors,
        ImportStage::Dream,
        ImportStage::HistoricalFreshness,
    ] {
        report(stage, StageState::Complete, 0);
    }
    report(ImportStage::Relationship, StageState::NotConfigured, 0);
    Ok(ProcessingState::Ready)
}
