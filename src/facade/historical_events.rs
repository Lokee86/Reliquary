//! Native owner evidence to canonical semantic and Dream event plans.
//! This module cannot publish or accept caller-authored roots/exclusions.
use super::{encoding::Value, fail, input::HistoricalMemoryInput, manifest::Manifest};
use crate::freshness::{
    historical_dream::{
        HistoricalDreamMemory, HistoricalDreamPlan, HistoricalFirstCycleProof,
        plan_historical_dream_events,
    },
    historical_planner::MemoryAvailability,
    historical_propagation::HistoricalPropagationEvent,
    historical_semantic::{
        HistoricalSemanticPlan, HistoricalSemanticSource, HistoricalSemanticSourceVector,
        HistoricalSemanticTarget,
    },
    semantic_policy::{SemanticPolicy, SemanticVectorProfile},
};
use crate::{Cva, CvaError};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeHistoricalEvents {
    pub semantic: HistoricalSemanticPlan,
    pub dream: HistoricalDreamPlan,
    pub ordered: Vec<([u8; 32], HistoricalPropagationEvent)>,
}
fn availability(input: &HistoricalMemoryInput) -> MemoryAvailability {
    match input.availability {
        Some(position) if input.credit_eligible => MemoryAvailability::Supported { position },
        Some(_) => MemoryAvailability::CreditExcluded,
        None => MemoryAvailability::Unsupported,
    }
}
pub(super) fn build(
    cva: &mut Cva,
    manifest: &Manifest,
) -> Result<NativeHistoricalEvents, CvaError> {
    use crate::cva_semantic_source::SemanticSourceReadiness;
    let owner = cva
        .owner_uuid()
        .ok_or_else(|| fail("missing historical event owner"))?;
    let owner_id = cva
        .owner_id()
        .ok_or_else(|| fail("missing historical event owner identity"))?;
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
    let mut sources = Vec::new();
    let mut targets = Vec::new();
    // Proof identity is per base record; policy identity is the canonical
    // generating Episode/publication cohort shared with live exclusions.
    let mut policy_cohorts: BTreeMap<_, _> = manifest
        .memory_inputs
        .iter()
        .map(|(id, input)| (*id, input.cohort_key))
        .collect();
    if epoch_scope.is_some() {
        let mut parents: BTreeMap<_, _> = policy_cohorts.keys().map(|id| (*id, *id)).collect();
        let mut episodes = BTreeMap::new();
        fn root(
            parents: &BTreeMap<crate::MemoryId, crate::MemoryId>,
            mut id: crate::MemoryId,
        ) -> crate::MemoryId {
            while parents[&id] != id {
                id = parents[&id];
            }
            id
        }
        for id in manifest.memory_inputs.keys() {
            let memory = cva.memory(*id)?;
            let mut peers = cva.freshness_publication_cohort(*id)?;
            if let Some(episode) = memory.source_episode_id {
                if let Some(peer) = episodes.insert(episode.0, *id) {
                    peers.insert(peer);
                }
            }
            for peer in peers
                .into_iter()
                .filter(|peer| policy_cohorts.contains_key(peer))
            {
                let a = root(&parents, *id);
                let b = root(&parents, peer);
                parents.insert(a.max(b), a.min(b));
            }
        }
        for (id, key) in &mut policy_cohorts {
            *key = root(&parents, *id).0;
        }
    }
    let profile = generation
        .map(|g| {
            cva.compatibility_profile(g.compatibility_profile_id)
                .map(|p| SemanticVectorProfile {
                    profile_id: p.id.0,
                    dimensions: p.dimensions,
                    generation: g.vector_version,
                })
                .map_err(|e| fail(&e.to_string()))
        })
        .transpose()?
        .unwrap_or(SemanticVectorProfile {
            profile_id: [0; 32],
            dimensions: 0,
            generation: 0,
        });
    for fragment in cva.fragments() {
        let Some(fragment_position) = cva.semantic_fragment_position(fragment.id)? else {
            continue;
        };
        if epoch_scope
            .as_ref()
            .is_some_and(|scope| fragment_position <= scope.base_cut)
        {
            continue;
        }
        let raw_source_keys: BTreeSet<_> = cva
            .semantic_fragment_sources(fragment.id)?
            .into_iter()
            .collect();
        if let Some(scope) = &epoch_scope {
            let overlaps = raw_source_keys
                .iter()
                .any(|key| scope.source_nodes.contains(key));
            let fully_scoped = !raw_source_keys.is_empty()
                && raw_source_keys
                    .iter()
                    .all(|key| scope.source_nodes.contains(key));
            if overlaps && !fully_scoped {
                return Err(fail(
                    "semantic fragment mixes import and external source nodes",
                ));
            }
            if !fully_scoped {
                continue;
            }
        }
        let source_keys: BTreeSet<_> = raw_source_keys
            .into_iter()
            .map(
                |(conversation_id, node_id)| crate::freshness::historical_planner::SourceKey {
                    conversation_id,
                    node_id,
                },
            )
            .collect();
        let source = generation
            .map(|g| cva.prepare_semantic_fragment(fragment.id, g.id))
            .transpose()?;
        let source_cohorts: BTreeSet<_> = manifest
            .memory_inputs
            .iter()
            .filter(|(_, input)| {
                input
                    .necessary_sources
                    .iter()
                    .any(|key| source_keys.contains(key))
            })
            .map(|(id, _)| policy_cohorts[id])
            .collect();
        let credit_excluded = manifest
            .memory_inputs
            .iter()
            .filter(|(id, _)| source_cohorts.contains(&policy_cohorts[id]))
            .map(|(&id, _)| id)
            .collect();
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
        sources.push(HistoricalSemanticSource {
            fragment_id: fragment.id.0,
            source_keys,
            owner_id: owner_id.clone(),
            profile,
            evidence_available,
            evidence_digest,
            vectors,
            credit_excluded,
        });
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
                .map_err(|e| fail(&e.to_string()))?;
            if set.compatibility_profile_id != generation.compatibility_profile_id
                || set.memory_body_ids.get(location.row as usize) != Some(&input.body_id)
            {
                return Err(fail("historical event target binding conflict"));
            }
            let packed = cva.packed_vectors(set.packed_vector_id)?;
            let row = packed
                .row(location.row as usize)
                .ok_or_else(|| fail("missing historical event target row"))?;
            let compatible = packed.schema().dimensions == profile.dimensions
                && packed.schema().scalar == crate::ScalarType::F32;
            let vector = if compatible {
                row.chunks_exact(4)
                    .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                    .collect()
            } else {
                vec![]
            };
            let digest = Value::map([
                ("memory_id", Value::bytes(id.0)),
                ("body_id", Value::bytes(input.body_id.0)),
                ("binding_id", Value::bytes(set.id.0)),
                ("profile_id", Value::bytes(profile.profile_id)),
                ("row", Value::Unsigned(location.row)),
                ("row_digest", Value::bytes(Sha256::digest(row))),
            ])
            .digest("reliquary:historical-target-binding:v1")?;
            targets.push(HistoricalSemanticTarget {
                memory_id: id,
                owner_id: owner_id.clone(),
                availability: availability(input),
                lifecycle_eligible: input.credit_eligible,
                profile,
                vector,
                vector_binding_id: set.id.0,
                evidence_digest: digest,
            });
        }
    }
    let semantic = HistoricalSemanticPlan::plan(
        owner,
        &owner_id,
        SemanticPolicy::default(),
        &manifest.timeline,
        sources,
        targets,
    )
    .map_err(|e| fail(&e.to_string()))?;
    let completed = cva.historical_completed_initial_dream_passes()?;
    // The Dream planner needs descriptors for every possible root endpoint, while
    // only epoch-born memories may be event sources. Unsupported proofs suppress
    // source events for preexisting recipients without removing them as roots.
    let dream_ids: BTreeSet<_> = manifest.memory_inputs.keys().copied().collect();
    let dream_sources: BTreeSet<_> = epoch_scope
        .as_ref()
        .map_or_else(|| dream_ids.clone(), |scope| scope.cohort.clone());
    if dream_sources
        .iter()
        .any(|id| !manifest.memory_inputs.contains_key(id))
    {
        return Err(fail(
            "epoch-born Dream source is outside the recipient population",
        ));
    }
    let memories: Vec<_> = dream_ids
        .iter()
        .filter_map(|id| manifest.memory_inputs.get(id).map(|input| (*id, input)))
        .map(|(id, input)| HistoricalDreamMemory {
            id,
            cohort: policy_cohorts[&id]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            birth_position: input.availability,
            credit_eligible: input.credit_eligible,
        })
        .collect();
    let mut proofs = BTreeMap::new();
    let mut representatives = BTreeMap::new();
    for &id in &dream_ids {
        if !dream_sources.contains(&id) {
            proofs.insert(id, HistoricalFirstCycleProof::Unsupported);
            continue;
        }
        let Some(pass) = completed.get(&id) else {
            // A transformed/legacy settled marker cannot prove direct publication,
            // empty roots or a final-graph approximation.
            proofs.insert(id, HistoricalFirstCycleProof::Unsupported);
            continue;
        };
        let (roots, duplicate_origin) = cva.initial_dream_committed_evidence(pass)?;
        if roots != pass.roots {
            return Err(fail(
                "retained historical first-cycle roots conflict with accepted graph evidence",
            ));
        }
        for (root, representative) in cva.historical_dream_representatives(id, &roots)? {
            if representatives
                .insert(root, representative)
                .is_some_and(|old| old != representative)
            {
                return Err(fail("conflicting historical representative authority"));
            }
        }
        let evidence = Value::map([
            ("owner_uuid", Value::bytes(owner)),
            ("source_id", Value::bytes(id.0)),
            ("pass_id", Value::text(&pass.pass_id)),
            ("first_revision", Value::Unsigned(pass.first_revision)),
            (
                "graph_version_before",
                Value::Unsigned(pass.graph_version_before),
            ),
            (
                "accepted_settlement_turn",
                pass.accepted_settlement_turn
                    .map_or(Value::Null, Value::Unsigned),
            ),
            (
                "roots",
                Value::Array(roots.iter().map(|id| Value::bytes(id.0)).collect()),
            ),
            ("duplicate_origin", Value::Bool(duplicate_origin)),
        ])
        .digest("reliquary:historical-first-cycle-evidence:v1")?;
        proofs.insert(
            id,
            HistoricalFirstCycleProof::Completed {
                roots,
                duplicate_origin,
                evidence_digest: evidence,
            },
        );
    }
    let dream = plan_historical_dream_events(
        owner,
        manifest.import_id,
        &memories,
        &proofs,
        &[],
        &representatives,
    )
    .map_err(|e| fail(&e.to_string()))?;
    let mut ordered = Vec::new();
    for event in &semantic.events {
        ordered.push((
            event.event_id,
            HistoricalPropagationEvent::from_semantic(event).map_err(|e| fail(&e.to_string()))?,
        ));
    }
    ordered.extend(
        dream
            .events
            .iter()
            .map(|event| (event.event_id, event.propagation.clone())),
    );
    ordered.sort_by_key(|(id, event)| (event.position, *id));
    let mut seen = BTreeSet::new();
    if ordered.iter().any(|(id, _)| !seen.insert(*id)) {
        return Err(fail("conflicting canonical historical event identities"));
    }
    Ok(NativeHistoricalEvents {
        semantic,
        dream,
        ordered,
    })
}
