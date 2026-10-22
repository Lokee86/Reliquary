//! H1 evidence assembled only while the Cva owner is exclusively borrowed.
use super::{
    encoding::{Value, policy},
    fail, file_digest, input,
};
pub(super) use crate::history::community_proof::{community_proof, record_value, sorted};
use crate::{Cva, CvaError, MemoryId, freshness::FreshnessRecord};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(super) struct PropagationInputs {
    pub adjacency: crate::freshness::MemoryAdjacency,
    pub memberships: crate::freshness::CommunityMembership,
    pub authorized: BTreeSet<MemoryId>,
    pub availability: BTreeMap<MemoryId, crate::freshness::historical_planner::MemoryAvailability>,
}
pub(super) struct Manifest {
    pub propagation: PropagationInputs,
    pub input_digest: [u8; 32],
    pub input_bytes: u64,
    pub memory_inputs: BTreeMap<MemoryId, input::HistoricalMemoryInput>,
    pub timeline: crate::freshness::historical_planner::HistoricalTimeline,
    pub digest: [u8; 32],
    pub corpus_digest: [u8; 32],
    pub import_id: [u8; 32],
    pub initial: BTreeMap<MemoryId, FreshnessRecord>,
    pub superseded: BTreeSet<String>,
    /// Whether this proof is bound to one authenticated import epoch.
    pub epoch: bool,
    /// Born and provisionally affected recipients; legacy retains the owner set.
    pub cohort: BTreeSet<MemoryId>,
    /// Pre-start activity watermark for an epoch.
    pub base_cut: u64,
    /// Digest of the durable ImportStart evidence for an epoch.
    pub start_evidence_digest: [u8; 32],
    #[cfg(test)]
    pub value: Value,
}
fn graph_proof(
    cva: &mut Cva,
    ids: &BTreeSet<MemoryId>,
    cohort: &BTreeSet<MemoryId>,
    scoped: bool,
) -> Result<(Value, crate::freshness::MemoryAdjacency), CvaError> {
    use crate::history::graph_proof as proof;
    let mut selection = proof::select(
        cva.graph.active_semantic_relations(),
        ids,
        scoped,
        &cva.memories,
    )?;
    for &id in ids {
        let target = cva.memory(id)?.superseded_by;
        let exists = target.is_none_or(|target| cva.memories.contains_memory(target));
        proof::observe_supersession(&mut selection, id, target, exists, ids, scoped)?;
    }
    let mut authorities = Vec::new();
    for id in selection.duplicates.iter().copied() {
        let representative = crate::dream_canonical::historical_duplicate_representative(
            cva,
            id,
            &cva.graph_relations(),
        )
        .map_err(|e| fail(&e.to_string()))?
        .ok_or_else(|| fail("unresolved historical duplicate authority"))?;
        let memory = cva.memory(representative)?;
        if memory.archived
            || memory.superseded_by.is_some()
            || !matches!(memory.lifecycle_state.as_str(), "knowledge" | "canonical")
        {
            return Err(fail("unresolved historical duplicate authority"));
        }
        let body_id = cva.memory_body_id(representative)?;
        authorities.push(proof::authority_value(id, representative, body_id));
    }
    let passes = cva.historical_completed_initial_dream_passes()?;
    let turn_count = cva.rel_turn_count();
    let mut dream = Vec::new();
    for (id, pass) in passes {
        if !cohort.contains(&id) {
            continue;
        }
        proof::validate_pass(id, &pass, cohort, ids, turn_count)?;
        let (roots, duplicate_origin) = cva.initial_dream_committed_evidence(&pass)?;
        if roots != pass.roots {
            return Err(fail(
                "retained first-cycle roots conflict with accepted graph proof",
            ));
        }
        let representatives = cva.historical_dream_representatives(id, &pass.roots)?;
        dream.push(proof::dream_value(
            id,
            pass,
            duplicate_origin,
            representatives,
        ));
    }
    proof::finalize(
        selection,
        authorities,
        dream,
        cva.graph_version(),
        cva.memory_graph_version(),
    )
    .map_err(CvaError::from)
}
fn vector_proof(
    cva: &mut Cva,
    order: &[crate::freshness::historical_planner::SourceKey],
    requested_complete: bool,
    target_ids: &BTreeSet<MemoryId>,
    scoped_nodes: Option<&BTreeSet<(String, String)>>,
    scoped_base_cut: Option<u64>,
) -> Result<(Value, Vec<Value>), CvaError> {
    use crate::cva_semantic_source::SemanticSourceReadiness;
    use crate::history::vector_proof::{
        VectorFragmentReadiness, VectorNodeKey, VectorProofBuilder, VectorRowFacts,
        VectorTargetBinding,
    };

    let generation = cva
        .vector_generations
        .generations()
        .iter()
        .max_by_key(|g| g.vector_version)
        .copied();
    let mut proof = VectorProofBuilder::new();
    for (index, key) in order.iter().enumerate() {
        let position = index as u64 + 1;
        let key_facts = VectorNodeKey {
            conversation_id: key.conversation_id.clone(),
            node_id: key.node_id.clone(),
        };
        if !VectorProofBuilder::include_source(&key_facts, position, scoped_nodes, scoped_base_cut)
        {
            continue;
        }
        let node = cva
            .archive
            .nodes
            .get(&key.conversation_id, &key.node_id)
            .ok_or_else(|| fail("missing historical source"))?;
        proof.record_source(input::node_binding(node), position);
    }

    for fragment in cva.fragments() {
        let nodes = cva.archive.fragment_nodes_for(&fragment)?;
        let node_keys: Vec<_> = nodes
            .iter()
            .map(|node| VectorNodeKey {
                conversation_id: node.conversation_id.clone(),
                node_id: node.id.clone(),
            })
            .collect();
        if !VectorProofBuilder::fragment_scope(&node_keys, scoped_nodes)? {
            continue;
        }
        let position = cva.semantic_fragment_position(fragment.id)?;
        if !VectorProofBuilder::fragment_after_base(position, scoped_base_cut) {
            continue;
        }
        let activity_nodes = nodes
            .iter()
            .filter(|n| {
                cva.activity_position_for_turn(&n.conversation_id, &n.id)
                    .is_some()
            })
            .count();
        let readiness = generation
            .map(|g| cva.prepare_semantic_fragment(fragment.id, g.id))
            .transpose()?
            .map(|readiness| match readiness {
                SemanticSourceReadiness::Ready(source) => VectorFragmentReadiness {
                    complete: true,
                    source_digest: source.source_digest,
                    generation_id: source.generation.id.0,
                    profile_id: source.generation.compatibility_profile_id.0,
                    invalid_vectors: source.invalid_vectors as u64,
                    rows: source
                        .vectors
                        .into_iter()
                        .map(|vector| VectorRowFacts {
                            fragment_id: vector.fragment_id.0,
                            row: vector.row,
                            component_bytes: vector
                                .components
                                .iter()
                                .flat_map(|component| component.to_le_bytes())
                                .collect(),
                        })
                        .collect(),
                },
                SemanticSourceReadiness::Pending => VectorFragmentReadiness {
                    complete: false,
                    source_digest: [0; 32],
                    generation_id: [0; 32],
                    profile_id: [0; 32],
                    invalid_vectors: 0,
                    rows: Vec::new(),
                },
            });
        proof.record_fragment(
            fragment.id.0,
            &fragment.conversation_id,
            &fragment.start_node_id,
            &fragment.end_node_id,
            nodes.iter().map(input::node_binding).collect(),
            position,
            activity_nodes,
            readiness,
        )?;
    }

    for profile in cva.compatibility_profiles.profiles() {
        let descriptor = crate::compatibility_profile_codec::encode_profile(&profile)
            .map_err(|e| fail(&e.to_string()))?;
        proof.record_profile(profile.id.0, descriptor);
    }

    let mut ids: Vec<_> = target_ids.iter().copied().collect();
    ids.sort();
    for id in ids {
        let body = cva.memory_body_id(id)?;
        for profile in cva.compatibility_profiles.profiles() {
            let Some(location) = cva.memory_vector_location(profile.id, body) else {
                proof.record_target(VectorTargetBinding {
                    memory_id: id.0,
                    body_id: body.0,
                    profile_id: profile.id.0,
                    binding_id: None,
                    packed_id: None,
                    row: None,
                    row_bytes: None,
                    usable: false,
                });
                continue;
            };
            let set = cva
                .memory_vectors(location.set_id)
                .map_err(|e| fail(&e.to_string()))?;
            if set.compatibility_profile_id != profile.id
                || set.memory_body_ids.get(location.row as usize) != Some(&body)
            {
                return Err(fail("historical target vector binding conflict"));
            }
            let packed = cva.packed_vectors(set.packed_vector_id)?;
            let row = packed
                .row(location.row as usize)
                .ok_or_else(|| fail("missing historical target row"))?;
            let compatible = packed.schema().dimensions == profile.dimensions
                && packed.schema().scalar == crate::ScalarType::F32;
            let components: Vec<_> = row
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            let usable = compatible
                && crate::freshness::semantic_policy::cosine_similarity(&components, &components)
                    .is_ok();
            proof.record_target(VectorTargetBinding {
                memory_id: id.0,
                body_id: body.0,
                profile_id: profile.id.0,
                binding_id: Some(set.id.0),
                packed_id: Some(set.packed_vector_id.0),
                row: Some(location.row),
                row_bytes: Some(row.to_vec()),
                usable,
            });
        }
    }

    let generation_facts = generation.map(|g| {
        (
            g.id.0,
            g.compatibility_profile_id.0,
            g.archive_vector_id.0,
            g.source_archive_version,
            g.vector_version,
        )
    });
    let embedding_pass = if requested_complete {
        super::embedding::manifest_pass(cva, &proof.fragments_for_embedding())?
    } else {
        Value::text("not-requested")
    };
    Ok(proof.finish(generation_facts, embedding_pass))
}
#[cfg(test)]
pub(super) fn build(cva: &mut Cva) -> Result<Manifest, CvaError> {
    build_with_fill(cva, false)
}
pub(super) fn build_with_fill(
    cva: &mut Cva,
    requested_complete: bool,
) -> Result<Manifest, CvaError> {
    let scope = if cva.open_import_epoch().is_some() {
        Some(cva.import_epoch_scope()?)
    } else {
        None
    };
    let owner = if let Some(scope) = &scope {
        let owner = cva
            .owner_uuid()
            .ok_or_else(|| fail("historical import requires a durable REL owner"))?;
        if owner != scope.start.owner_uuid
            || cva.freshness_policy() != crate::freshness::FreshnessPolicy::default_v1()
        {
            return Err(fail("unsupported import owner or policy"));
        }
        owner
    } else {
        cva.historical_reconstruction_owner()?
    };
    // Validate ancestry before the timeline planner follows parent chains.
    input::required_nodes(&cva.archive, cva.rel_turn_count())?;
    let timeline = cva
        .historical_freshness_timeline()
        .map_err(|e| fail(&e.to_string()))?;
    let corpus_digest = if let Some(scope) = &scope {
        input::scoped_corpus_digest(cva, &scope.source_nodes)?
    } else {
        input::corpus_digest(cva, cva.rel_turn_count())?
    };
    let import_id = scope.as_ref().map_or_else(
        || historical_import_id(owner, corpus_digest),
        |scope| decode_import_id(&scope.start.import_id),
    );
    let admission = input::admission_proof_scoped(cva, scope.as_ref())?;
    let born_ids: BTreeSet<_> = scope.as_ref().map_or_else(
        || cva.memory_ids().into_iter().collect(),
        |scope| scope.cohort.clone(),
    );
    for &id in &born_ids {
        if !cva.freshness_is_publication_birth(id)
            || !cva
                .initial_dream_pass_settled(&crate::dream_pass_identity::initial_pass_id(owner, id))
        {
            return Err(fail(
                "import cohort lacks settled first-cycle/publication evidence",
            ));
        }
    }
    if admission.initial.is_empty() && scope.is_none() {
        return Err(fail("import has no supported active cohort"));
    }
    let owner_ids: BTreeSet<_> = if scope.is_none() {
        cva.memory_ids().into_iter().collect()
    } else {
        BTreeSet::new()
    };
    let ids: BTreeSet<_> = admission.memory_inputs.keys().copied().collect();
    let replacement_ids: BTreeSet<_> = admission.initial.keys().copied().collect();
    let scoped_event_evidence = if let Some(scope) = &scope {
        let freeze_prefix = std::fs::metadata(cva.container.path())
            .map_err(|error| fail(&error.to_string()))?
            .len();
        let mut frozen = cva.freshness.clone();
        frozen
            .bind_history_source(&cva.container, freeze_prefix)
            .map_err(CvaError::Freshness)?;
        Some(
            frozen
                .import_event_evidence_in_scope(&crate::freshness_storage::ImportEventScope {
                    owner,
                    import_id,
                    start_evidence_digest: scope.start_digest,
                    born_memories: scope.cohort.clone(),
                    affected_recipients: replacement_ids.clone(),
                    source_nodes: scope.source_nodes.clone(),
                    start_prefix: scope.marker_end,
                    freeze_prefix,
                    base_cut: scope.base_cut,
                    freeze_cut: cva.rel_turn_count(),
                })
                .map_err(CvaError::Freshness)?,
        )
    } else {
        None
    };
    let superseded = if let Some(evidence) = &scoped_event_evidence {
        evidence.event_ids.clone()
    } else {
        cva.freshness
            .provisional_import_events(&replacement_ids, 0, cva.rel_turn_count())
            .map_err(CvaError::Freshness)?
    };
    let mut manifest_cohort = if scope.is_some() {
        born_ids.clone()
    } else {
        owner_ids.clone()
    };
    if let Some(evidence) = &scoped_event_evidence {
        manifest_cohort.extend(evidence.affected_recipients.iter().copied());
    }
    let mut untouched = Vec::new();
    let base_recipient_records = if scope.is_some() {
        ids.difference(&born_ids)
            .map(|id| {
                let record = admission.initial.get(id).ok_or_else(|| {
                    fail("scoped pre-start recipient lacks its authenticated base record")
                })?;
                Ok(Value::map([
                    ("memory_id", Value::bytes(id.0)),
                    ("base_record", record_value(*record)),
                ]))
            })
            .collect::<Result<Vec<_>, CvaError>>()?
    } else {
        for entry in cva
            .freshness
            .replay_entries()
            .map_err(CvaError::Freshness)?
        {
            if matches!(
                &entry,
                crate::freshness_storage::FreshnessReplayEntry::AcceptedUseReceipt { .. }
            ) {
                return Err(fail(
                    "legacy reconstruction cannot replace retained accepted use without an import base",
                ));
            }
            if let crate::freshness_storage::FreshnessReplayEntry::Event {
                event_id, effects, ..
            } = entry
                && !superseded.contains(&event_id)
            {
                untouched.push(Value::map([
                    ("event_id", Value::text(event_id)),
                    (
                        "recipients",
                        Value::Array(effects.keys().map(|id| Value::bytes(id.0)).collect()),
                    ),
                ]));
            }
        }
        Vec::new()
    };
    let reconstructed_memories: BTreeSet<_> = if scope.is_some() {
        manifest_cohort.clone()
    } else {
        admission.initial.keys().copied().collect()
    };
    let untouched_memory_ids: BTreeSet<_> = if scope.is_some() {
        ids.difference(&reconstructed_memories).copied().collect()
    } else {
        owner_ids
            .difference(&reconstructed_memories)
            .copied()
            .collect()
    };
    let mut replacement = Value::map([
        (
            "superseded_event_ids",
            Value::Array(superseded.iter().map(Value::text).collect()),
        ),
        ("untouched_events", sorted(untouched)?),
        (
            if scope.is_some() {
                "born_and_provisionally_affected_memories"
            } else {
                "reconstructed_memories"
            },
            Value::Array(
                reconstructed_memories
                    .iter()
                    .map(|id| Value::bytes(id.0))
                    .collect(),
            ),
        ),
        (
            if scope.is_some() {
                "recipients_without_provisional_effects"
            } else {
                "untouched_memories"
            },
            Value::Array(
                untouched_memory_ids
                    .iter()
                    .map(|id| Value::bytes(id.0))
                    .collect(),
            ),
        ),
        (
            "isolation",
            Value::text("verified-disjoint-provisional-dream-effects-v1"),
        ),
    ]);
    if scope.is_some() {
        let Value::Map(fields) = &mut replacement else {
            unreachable!("historical replacement proof root is a map");
        };
        fields.push((
            "base_recipient_records".to_owned(),
            Value::Array(base_recipient_records),
        ));
    }
    let (graph, adjacency) = graph_proof(cva, &ids, &born_ids, scope.is_some())?;
    let snapshot = cva.community_snapshot();
    let scoped_snapshot = if scope.is_some() {
        snapshot.clone().map(|mut snapshot| {
            for community in &mut snapshot.communities {
                community.members.retain(|id| ids.contains(id));
            }
            snapshot
                .communities
                .retain(|community| !community.members.is_empty());
            snapshot
        })
    } else {
        snapshot.clone()
    };
    let community = community_proof(scoped_snapshot.clone(), cva.memory_graph_version(), &ids)?;
    let memberships = scoped_snapshot
        .filter(|s| s.derived_graph_version == cva.memory_graph_version())
        .into_iter()
        .flat_map(|s| s.communities)
        .flat_map(|c| c.members.into_iter().map(move |id| (id, c.id)))
        .collect();
    let availability = admission
        .memory_inputs
        .iter()
        .map(|(&id, input)| {
            use crate::freshness::historical_planner::MemoryAvailability;
            let value = match input.availability {
                Some(position) if input.credit_eligible => {
                    MemoryAvailability::Supported { position }
                }
                Some(_) => MemoryAvailability::CreditExcluded,
                None => MemoryAvailability::Unsupported,
            };
            (id, value)
        })
        .collect();
    let propagation = PropagationInputs {
        adjacency,
        memberships,
        authorized: ids.clone(),
        availability,
    };
    let (vectors, sources) = vector_proof(
        cva,
        &timeline.activity_order,
        requested_complete,
        &ids,
        scope.as_ref().map(|scope| &scope.source_nodes),
        scope.as_ref().map(|scope| scope.base_cut),
    )?;
    let order = Value::Array(
        timeline
            .activity_order
            .iter()
            .map(|k| input::source_key(&k.conversation_id, &k.node_id))
            .collect(),
    );
    let historical_policy = policy();
    let policy_digest = historical_policy.digest("reliquary:historical-policy:v1")?;
    if policy_digest != crate::freshness::historical_planner::HISTORICAL_POLICY_DIGEST {
        return Err(fail("unsupported historical policy digest"));
    }
    let base = cva.freshness_policy();
    let mut base_bytes = Vec::new();
    base_bytes.extend(base.decay_turns_per_point.to_le_bytes());
    for n in [
        base.access_principal,
        base.linkage_principal,
        base.local_hop_cost,
        base.outside_hop_cost,
    ] {
        base_bytes.extend(n.to_le_bytes());
    }
    let mut base_hash = Sha256::new();
    base_hash.update(b"reliquary:freshness-policy:v1\0");
    base_hash.update(&base_bytes);
    let semantic = crate::freshness::semantic_policy::SemanticPolicy::default();
    cva.container.sync()?;
    let input_bytes = std::fs::metadata(cva.container.path())
        .map_err(|e| fail(&e.to_string()))?
        .len();
    let prefix = file_digest(cva.container.path())?;
    if std::fs::metadata(cva.container.path())
        .map_err(|e| fail(&e.to_string()))?
        .len()
        != input_bytes
        || file_digest(cva.container.path())? != prefix
    {
        return Err(fail("ImportInputDrift: discard and refreeze the manifest"));
    }
    let semantic_descriptor = semantic
        .canonical_descriptor()
        .map_err(|error| CvaError::Freshness(error.to_string()));
    let semantic_digest = semantic
        .digest()
        .map_err(|error| CvaError::Freshness(error.to_string()));
    let encoded = crate::history::manifest_proof::build(
        crate::history::manifest_proof::ManifestProofParts {
            owner,
            import_id,
            corpus_digest,
            global_version: cva.latest_global_version(),
            archive_version: cva.archive_version(),
            activity_cut: cva.rel_turn_count(),
            memory_version: cva.memory_version(),
            graph_version: cva.graph_version(),
            memory_graph_version: cva.memory_graph_version(),
            vector_version: cva.vector_version(),
            community_generation: cva.community_snapshot().map(|snapshot| snapshot.generation),
            graph,
            replacement,
            order,
            last_source: timeline
                .activity_order
                .last()
                .map(|key| (key.conversation_id.clone(), key.node_id.clone())),
            source_records: sources,
            memory_records: admission.memories,
            cohort_records: admission.cohorts,
            community,
            vector: vectors,
            base_descriptor: base_bytes,
            base_digest: base_hash.finalize().into(),
            semantic_descriptor,
            semantic_digest,
            historical_policy,
            historical_digest: policy_digest,
            scope: scope
                .as_ref()
                .map(|scope| crate::history::manifest_proof::ImportScopeProof {
                    start_digest: scope.start_digest,
                    base_cut: scope.base_cut,
                    import_key: scope.start.import_key.clone(),
                    source_nodes: scope.source_nodes.clone(),
                    episodes: scope.episodes.clone(),
                    recipient_cohort: manifest_cohort.clone(),
                }),
        },
    )?;
    let value = encoded.value;
    let digest = encoded.digest;
    Ok(Manifest {
        propagation,
        input_digest: prefix,
        input_bytes,
        memory_inputs: admission.memory_inputs,
        timeline,
        digest,
        corpus_digest,
        import_id,
        initial: admission.initial,
        superseded,
        epoch: scope.is_some(),
        cohort: manifest_cohort,
        base_cut: scope.as_ref().map_or(0, |scope| scope.base_cut),
        start_evidence_digest: scope.as_ref().map_or(prefix, |scope| scope.start_digest),
        #[cfg(test)]
        value,
    })
}

pub(super) fn historical_import_id(owner: [u8; 16], corpus: [u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary:historical-import:v1\0");
    hash.update([7]);
    hash.update(2u32.to_le_bytes());
    for (key, value) in [
        ("corpus_digest", corpus.as_slice()),
        ("owner_uuid", owner.as_slice()),
    ] {
        hash.update([5]);
        hash.update((key.len() as u32).to_le_bytes());
        hash.update(key.as_bytes());
        hash.update([4]);
        hash.update((value.len() as u32).to_le_bytes());
        hash.update(value);
    }
    hash.finalize().into()
}

fn decode_import_id(value: &str) -> [u8; 32] {
    let mut out = [0; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        let pair = &value[index * 2..index * 2 + 2];
        *byte = u8::from_str_radix(pair, 16).expect("validated durable import ID");
    }
    out
}
