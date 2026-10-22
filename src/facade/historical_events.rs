//! Owner read adapter for Import's canonical historical event preparation.
use super::manifest::Manifest;
use crate::freshness::{
    historical_dream::HistoricalFirstCycleProof,
    historical_semantic::HistoricalSemanticSourceVector, semantic_policy::SemanticVectorProfile,
};
use crate::imports::historical_event_plan as event_plan;
use crate::{Cva, CvaError};
use sha2::Digest;
use std::collections::{BTreeMap, BTreeSet};

pub(super) use crate::imports::historical_event_plan::NativeHistoricalEvents;

fn event_memory_facts(manifest: &Manifest) -> Vec<event_plan::EventMemoryFacts> {
    manifest
        .memory_inputs
        .iter()
        .map(|(&id, input)| event_plan::EventMemoryFacts {
            id,
            cohort_key: input.cohort_key,
            source_episode: None,
            availability: input.availability,
            credit_eligible: input.credit_eligible,
            necessary_sources: input.necessary_sources.clone(),
        })
        .collect()
}

pub(super) fn build(
    cva: &mut Cva,
    manifest: &Manifest,
) -> Result<NativeHistoricalEvents, CvaError> {
    use crate::cva_semantic_source::SemanticSourceReadiness;

    let owner = cva
        .owner_uuid()
        .ok_or_else(|| CvaError::Freshness("missing historical event owner".into()))?;
    let owner_id = cva
        .owner_id()
        .ok_or_else(|| CvaError::Freshness("missing historical event owner identity".into()))?;
    // Historical import planning is constrained to the physical import epoch.
    // Keep the old all-history behavior for ordinary (non-import) manifests.
    let epoch_scope = if cva.open_import_epoch().is_some() {
        Some(cva.import_epoch_scope()?)
    } else {
        None
    };
    let generation = cva
        .vector_generations
        .generations()
        .iter()
        .max_by_key(|g| g.vector_version)
        .copied();

    // Collect owner facts for Import planning. Reads stay here; cohort identity
    // and event selection belong to the Import module.
    let mut memories = event_memory_facts(manifest);
    let mut publication_cohorts = BTreeMap::new();
    if epoch_scope.is_some() {
        for memory in &mut memories {
            let value = cva.memory(memory.id)?;
            memory.source_episode = value.source_episode_id.map(|id| id.0);
            publication_cohorts.insert(memory.id, cva.freshness_publication_cohort(memory.id)?);
        }
    }
    let policy_cohorts =
        event_plan::merge_policy_cohorts(&memories, &publication_cohorts, epoch_scope.is_some());

    let profile = generation
        .map(|g| {
            cva.compatibility_profile(g.compatibility_profile_id)
                .map(|p| SemanticVectorProfile {
                    profile_id: p.id.0,
                    dimensions: p.dimensions,
                    generation: g.vector_version,
                })
                .map_err(|e| CvaError::Freshness(e.to_string()))
        })
        .transpose()?
        .unwrap_or(SemanticVectorProfile {
            profile_id: [0; 32],
            dimensions: 0,
            generation: 0,
        });

    let mut sources = Vec::new();
    let mut targets = Vec::new();
    for fragment in cva.fragments() {
        let Some(fragment_position) = cva.semantic_fragment_position(fragment.id)? else {
            continue;
        };
        let base_cut = epoch_scope.as_ref().map(|scope| scope.base_cut);
        if !event_plan::fragment_is_after_base(fragment_position, base_cut) {
            continue;
        }
        let raw_source_keys: BTreeSet<_> = cva
            .semantic_fragment_sources(fragment.id)?
            .into_iter()
            .collect();
        let source_keys = event_plan::scoped_fragment_sources(
            raw_source_keys,
            epoch_scope.as_ref().map(|scope| &scope.source_nodes),
            fragment_position,
            base_cut,
        )?;
        let Some(source_keys) = source_keys else {
            continue;
        };

        let source = generation
            .map(|g| cva.prepare_semantic_fragment(fragment.id, g.id))
            .transpose()?;
        let credit_excluded =
            event_plan::credit_excluded_targets(&source_keys, &memories, &policy_cohorts);
        let (evidence_available, evidence_digest, vectors) = match source {
            Some(SemanticSourceReadiness::Ready(source)) => (
                true,
                source.source_digest,
                source
                    .vectors
                    .into_iter()
                    .map(|v| HistoricalSemanticSourceVector {
                        fragment_id: v.fragment_id.0,
                        components: v.components,
                    })
                    .collect(),
            ),
            _ => (false, [0; 32], Vec::new()),
        };
        sources.push(event_plan::semantic_source(
            event_plan::EventFragmentFacts {
                id: fragment.id.0,
                source_keys,
                evidence_available,
                evidence_digest,
                vectors,
            },
            &owner_id,
            profile,
            credit_excluded,
        ));
    }

    if let Some(generation) = generation {
        for (&id, input) in &manifest.memory_inputs {
            let Some(location) =
                cva.memory_vector_location(generation.compatibility_profile_id, input.body_id)
            else {
                continue;
            };
            let set = cva
                .memory_vectors(location.set_id)
                .map_err(|e| CvaError::Freshness(e.to_string()))?;
            if set.compatibility_profile_id != generation.compatibility_profile_id
                || set.memory_body_ids.get(location.row as usize) != Some(&input.body_id)
            {
                return Err(CvaError::Freshness(
                    "historical event target binding conflict".into(),
                ));
            }
            let packed = cva.packed_vectors(set.packed_vector_id)?;
            let row = packed
                .row(location.row as usize)
                .ok_or_else(|| CvaError::Freshness("missing historical event target row".into()))?;
            let compatible = packed.schema().dimensions == profile.dimensions
                && packed.schema().scalar == crate::ScalarType::F32;
            let vector = if compatible {
                row.chunks_exact(4)
                    .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                    .collect()
            } else {
                vec![]
            };
            targets.push(event_plan::semantic_target(
                event_plan::EventTargetFacts {
                    id,
                    body_id: input.body_id.0,
                    availability: input.availability,
                    credit_eligible: input.credit_eligible,
                    binding_id: set.id.0,
                    row: location.row,
                    row_digest: sha2::Sha256::digest(row).into(),
                    vector,
                },
                &owner_id,
                profile,
            )?);
        }
    }

    let semantic =
        event_plan::semantic_plan(owner, &owner_id, &manifest.timeline, sources, targets)?;

    let completed = cva.historical_completed_initial_dream_passes()?;
    // The Dream planner needs descriptors for every possible root endpoint, while
    // only epoch-born memories may be event sources. Unsupported proofs suppress
    // source events for preexisting recipients without removing them as roots.
    let dream_ids: BTreeSet<_> = manifest.memory_inputs.keys().copied().collect();
    let dream_sources: BTreeSet<_> = epoch_scope
        .as_ref()
        .map_or_else(|| dream_ids.clone(), |scope| scope.cohort.clone());
    event_plan::validate_dream_sources(&dream_sources, &dream_ids)?;
    let dream_memories = event_plan::dream_memories(&memories, &policy_cohorts);
    let mut proofs = BTreeMap::<_, HistoricalFirstCycleProof>::new();
    let mut representatives = BTreeMap::new();
    for &id in &dream_ids {
        if !dream_sources.contains(&id) {
            proofs.insert(id, event_plan::unsupported_first_cycle());
            continue;
        }
        let Some(pass) = completed.get(&id) else {
            // A transformed/legacy settled marker cannot prove direct publication,
            // empty roots or a final-graph approximation.
            proofs.insert(id, event_plan::unsupported_first_cycle());
            continue;
        };
        let (roots, duplicate_origin) = cva.initial_dream_committed_evidence(pass)?;
        event_plan::validate_first_cycle_roots(&pass.roots, &roots)?;
        event_plan::record_representatives(
            &mut representatives,
            cva.historical_dream_representatives(id, &roots)?,
        )?;
        proofs.insert(
            id,
            event_plan::first_cycle_proof(
                owner,
                id,
                &pass.pass_id,
                pass.first_revision,
                pass.graph_version_before,
                pass.accepted_settlement_turn,
                &roots,
                duplicate_origin,
            )?,
        );
    }
    let dream = event_plan::dream_plan(
        owner,
        manifest.import_id,
        &dream_memories,
        &proofs,
        &representatives,
    )?;
    event_plan::native_events(semantic, dream)
}
