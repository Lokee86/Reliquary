//! Import-owned historical event orchestration over narrow immutable reads.
use crate::freshness::{
    historical_dream::HistoricalFirstCycleProof,
    historical_semantic::HistoricalSemanticSourceVector, semantic_policy::SemanticVectorProfile,
};
use crate::history::ImportEpochScope;
use crate::imports::{historical_event_plan as plan, historical_manifest_model::Manifest};
use crate::owner_lifecycle::{
    HistoryReadAccess, OwnerError,
    semantic_source::{HistoricalVectorReadAccess, SemanticSourceReadiness},
};
use crate::{Archive, MemoryId, memory_store::MemoryStore};
use sha2::Digest;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) use crate::imports::historical_event_plan::NativeHistoricalEvents;

pub(crate) struct HistoricalEventsOutcome {
    pub(crate) result: Result<NativeHistoricalEvents, OwnerError>,
    #[cfg(feature = "freshness-performance-trace")]
    pub(crate) traces: Vec<crate::history::dream_evidence::DreamEvidenceTrace>,
}

/// Concrete read capabilities gathered by the facade after its ordered
/// identity and epoch checks.  It contains no facade, writer, or raw container.
pub(crate) struct HistoricalEventReadAccess<'a> {
    pub(crate) owner_uuid: [u8; 16],
    pub(crate) owner_id: &'a str,
    pub(crate) epoch_scope: Option<ImportEpochScope>,
    pub(crate) history: &'a HistoryReadAccess<'a>,
    pub(crate) archive: &'a Archive,
    pub(crate) memories: &'a MemoryStore,
    pub(crate) vectors: &'a HistoricalVectorReadAccess<'a>,
    pub(crate) dream: &'a crate::history::dream_evidence::DreamEvidenceReadAccess<'a>,
}

fn event_memory_facts(manifest: &Manifest) -> Vec<plan::EventMemoryFacts> {
    manifest
        .memory_inputs
        .iter()
        .map(|(&id, input)| plan::EventMemoryFacts {
            id,
            cohort_key: input.cohort_key,
            source_episode: None,
            availability: input.availability,
            credit_eligible: input.credit_eligible,
            necessary_sources: input.necessary_sources.clone(),
        })
        .collect()
}

pub(crate) fn build(
    access: &HistoricalEventReadAccess<'_>,
    manifest: &Manifest,
) -> HistoricalEventsOutcome {
    #[cfg(feature = "freshness-performance-trace")]
    let mut traces = Vec::new();
    let result = build_inner(
        access,
        manifest,
        #[cfg(feature = "freshness-performance-trace")]
        &mut traces,
    );
    HistoricalEventsOutcome {
        result,
        #[cfg(feature = "freshness-performance-trace")]
        traces,
    }
}

fn build_inner(
    access: &HistoricalEventReadAccess<'_>,
    manifest: &Manifest,
    #[cfg(feature = "freshness-performance-trace")] traces: &mut Vec<
        crate::history::dream_evidence::DreamEvidenceTrace,
    >,
) -> Result<NativeHistoricalEvents, OwnerError> {
    let generation = access.vectors.generation();
    let mut memories = event_memory_facts(manifest);
    let mut publication_cohorts = BTreeMap::new();
    if access.epoch_scope.is_some() {
        for memory in &mut memories {
            let value = access.history.current_memory(access.memories, memory.id)?;
            memory.source_episode = value.source_episode_id.map(|id| id.0);
            publication_cohorts.insert(memory.id, access.dream.publication_cohort(memory.id)?);
        }
    }
    let policy_cohorts = plan::merge_policy_cohorts(
        &memories,
        &publication_cohorts,
        access.epoch_scope.is_some(),
    );

    let profile = generation
        .map(|generation| {
            let profile = access
                .vectors
                .profile(generation.compatibility_profile_id)?;
            Ok::<SemanticVectorProfile, OwnerError>(SemanticVectorProfile {
                profile_id: profile.id.0,
                dimensions: profile.dimensions,
                generation: generation.vector_version,
            })
        })
        .transpose()?
        .unwrap_or(SemanticVectorProfile {
            profile_id: [0; 32],
            dimensions: 0,
            generation: 0,
        });

    let mut sources = Vec::new();
    let mut targets = Vec::new();
    for fragment in access.vectors.fragments() {
        let Some(position) = access.vectors.fragment_position(fragment.id)? else {
            continue;
        };
        let base_cut = access.epoch_scope.as_ref().map(|scope| scope.base_cut);
        if !plan::fragment_is_after_base(position, base_cut) {
            continue;
        }
        let raw: BTreeSet<_> = access
            .vectors
            .fragment_sources(fragment.id)?
            .into_iter()
            .collect();
        let source_keys = plan::scoped_fragment_sources(
            raw,
            access.epoch_scope.as_ref().map(|scope| &scope.source_nodes),
            position,
            base_cut,
        )?;
        let Some(source_keys) = source_keys else {
            continue;
        };
        let source = generation
            .map(|generation| access.vectors.fragment(fragment.id, generation.id))
            .transpose()?;
        let excluded = plan::credit_excluded_targets(&source_keys, &memories, &policy_cohorts);
        let (available, digest, vectors) = match source {
            Some(SemanticSourceReadiness::Ready(source)) => (
                true,
                source.source_digest,
                source
                    .vectors
                    .into_iter()
                    .map(|vector| HistoricalSemanticSourceVector {
                        fragment_id: vector.fragment_id.0,
                        components: vector.components,
                    })
                    .collect(),
            ),
            _ => (false, [0; 32], Vec::new()),
        };
        sources.push(plan::semantic_source(
            plan::EventFragmentFacts {
                id: fragment.id.0,
                source_keys,
                evidence_available: available,
                evidence_digest: digest,
                vectors,
            },
            access.owner_id,
            profile,
            excluded,
        ));
    }

    if let Some(generation) = generation {
        for (&id, input) in &manifest.memory_inputs {
            let Some(location) = access
                .vectors
                .target_location(generation.compatibility_profile_id, input.body_id)
            else {
                continue;
            };
            let set = access
                .vectors
                .target_set(location.set_id)
                .map_err(|error| OwnerError::Freshness(error.to_string()))?;
            if set.compatibility_profile_id != generation.compatibility_profile_id
                || set.memory_body_ids.get(location.row as usize) != Some(&input.body_id)
            {
                return Err(OwnerError::Freshness(
                    "historical event target binding conflict".into(),
                ));
            }
            let packed = access.vectors.packed(set.packed_vector_id)?;
            let row = packed.row(location.row as usize).ok_or_else(|| {
                OwnerError::Freshness("missing historical event target row".into())
            })?;
            let vector = if packed.schema().dimensions == profile.dimensions
                && packed.schema().scalar == lodestone_packed::ScalarType::F32
            {
                row.chunks_exact(4)
                    .map(|bytes| f32::from_le_bytes(bytes.try_into().expect("four bytes")))
                    .collect()
            } else {
                vec![]
            };
            targets.push(plan::semantic_target(
                plan::EventTargetFacts {
                    id,
                    body_id: input.body_id.0,
                    availability: input.availability,
                    credit_eligible: input.credit_eligible,
                    binding_id: set.id.0,
                    row: location.row,
                    row_digest: sha2::Sha256::digest(row).into(),
                    vector,
                },
                access.owner_id,
                profile,
            )?);
        }
    }

    let semantic = plan::semantic_plan(
        access.owner_uuid,
        access.owner_id,
        &manifest.timeline,
        sources,
        targets,
    )?;
    let completed = access
        .dream
        .retained_completed(access.archive, access.memories)?;
    let dream_ids: BTreeSet<_> = manifest.memory_inputs.keys().copied().collect();
    let dream_sources: BTreeSet<_> = access
        .epoch_scope
        .as_ref()
        .map_or_else(|| dream_ids.clone(), |scope| scope.cohort.clone());
    plan::validate_dream_sources(&dream_sources, &dream_ids)?;
    let dream_memories = plan::dream_memories(&memories, &policy_cohorts);
    let mut proofs = BTreeMap::<MemoryId, HistoricalFirstCycleProof>::new();
    let mut representatives = BTreeMap::new();
    for &id in &dream_ids {
        if !dream_sources.contains(&id) {
            proofs.insert(id, plan::unsupported_first_cycle());
            continue;
        }
        let Some(pass) = completed.get(&id) else {
            proofs.insert(id, plan::unsupported_first_cycle());
            continue;
        };
        let evidence_outcome = access.dream.committed_evidence(pass);
        #[cfg(feature = "freshness-performance-trace")]
        if evidence_outcome.result.is_ok() {
            traces.push(evidence_outcome.trace);
        }
        let evidence = evidence_outcome.result?;
        plan::validate_first_cycle_roots(&pass.roots, &evidence.roots)?;
        plan::record_representatives(
            &mut representatives,
            access
                .dream
                .representatives(access.archive, access.memories, id, &evidence.roots)?,
        )?;
        proofs.insert(
            id,
            plan::first_cycle_proof(
                access.owner_uuid,
                id,
                &pass.pass_id,
                pass.first_revision,
                pass.graph_version_before,
                pass.accepted_settlement_turn,
                &evidence.roots,
                evidence.duplicate_origin,
            )?,
        );
    }
    let dream = plan::dream_plan(
        access.owner_uuid,
        manifest.import_id,
        &dream_memories,
        &proofs,
        &representatives,
    )?;
    plan::native_events(semantic, dream)
}
