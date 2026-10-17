//! H1 evidence assembled only while the Cva owner is exclusively borrowed.
use super::{
    encoding::{Value, policy},
    fail, file_digest, input,
};
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
    #[cfg(test)]
    pub value: Value,
}
pub(super) fn record_value(record: FreshnessRecord) -> Value {
    Value::map([
        ("score", Value::Signed(i64::from(record.score))),
        (
            "admitted_at_turn",
            record.admitted_at_turn.map_or(Value::Null, Value::Unsigned),
        ),
        (
            "accounted_turn",
            record.accounted_turn.map_or(Value::Null, Value::Unsigned),
        ),
    ])
}
fn sorted(values: Vec<Value>) -> Result<Value, CvaError> {
    let mut keyed = values
        .into_iter()
        .map(|v| v.encoded().map(|key| (key, v)))
        .collect::<Result<Vec<_>, _>>()?;
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    if keyed.windows(2).any(|p| p[0].0 == p[1].0) {
        return Err(fail("duplicate historical proof identity"));
    }
    Ok(Value::Array(keyed.into_iter().map(|(_, v)| v).collect()))
}
pub(super) fn community_proof(
    snapshot: Option<crate::CommunitySnapshot>,
    graph_version: u64,
    ids: &BTreeSet<MemoryId>,
) -> Result<Value, CvaError> {
    let Some(mut snapshot) = snapshot else {
        return Ok(Value::map([("mode", Value::text("unknown-outside-v1"))]));
    };
    // Malformed present evidence rejects even when the version is stale.
    if (!snapshot.resolution.is_finite() || snapshot.resolution <= 0.0)
        || !snapshot.quality.is_finite()
        || snapshot.generation == 0
        || snapshot.algorithm_version != crate::COMMUNITY_ALGORITHM_VERSION
    {
        return Err(fail("malformed historical Community parameters"));
    }
    snapshot.communities.sort_by_key(|c| c.id);
    let mut communities = Vec::new();
    let mut seen = BTreeSet::new();
    let mut names = BTreeSet::new();
    for mut community in snapshot.communities {
        community.members.sort();
        if community.members.is_empty()
            || !names.insert(community.id)
            || community
                .members
                .iter()
                .any(|id| !ids.contains(id) || !seen.insert(*id))
        {
            return Err(fail(
                "malformed, duplicate or foreign historical Community membership",
            ));
        }
        communities.push(Value::map([
            ("community_id", Value::bytes(community.id.0)),
            (
                "members",
                Value::Array(
                    community
                        .members
                        .into_iter()
                        .map(|id| Value::bytes(id.0))
                        .collect(),
                ),
            ),
        ]));
    }
    let membership = Value::Array(communities);
    let evidence = Value::map([
        ("generation", Value::Unsigned(snapshot.generation)),
        (
            "derived_graph_version",
            Value::Unsigned(snapshot.derived_graph_version),
        ),
        (
            "algorithm_version",
            Value::Unsigned(u64::from(snapshot.algorithm_version)),
        ),
        ("seed", Value::Unsigned(snapshot.seed)),
        (
            "resolution",
            Value::bytes(snapshot.resolution.to_le_bytes()),
        ),
        ("quality", Value::bytes(snapshot.quality.to_le_bytes())),
        ("membership", membership.clone()),
        (
            "membership_digest",
            Value::bytes(membership.digest("reliquary:historical-community-membership:v1")?),
        ),
    ]);
    Ok(Value::map([
        (
            "mode",
            Value::text(if snapshot.derived_graph_version == graph_version {
                "validated-v1"
            } else {
                "unknown-outside-v1"
            }),
        ),
        (
            "membership",
            if snapshot.derived_graph_version == graph_version {
                membership
            } else {
                Value::Array(vec![])
            },
        ),
        ("snapshot", evidence),
    ]))
}
fn graph_proof(
    cva: &mut Cva,
    ids: &BTreeSet<MemoryId>,
) -> Result<(Value, crate::freshness::MemoryAdjacency), CvaError> {
    let mut relations = cva.graph.active_semantic_relations();
    relations.sort_by_key(|r| (r.source, r.target, r.kind.code()));
    let mut typed = Vec::new();
    let mut neighbors = BTreeMap::<MemoryId, BTreeSet<MemoryId>>::new();
    let mut duplicates = BTreeSet::new();
    let mut excluded = BTreeMap::<u8, u64>::new();
    for relation in relations {
        for node in [relation.source, relation.target] {
            if let Some(id) = node.as_memory() {
                if !ids.contains(&id) {
                    return Err(fail("dangling/foreign historical graph endpoint"));
                }
            } else {
                *excluded.entry(node.kind.code()).or_default() += 1;
            }
        }
        if let (Some(a), Some(b)) = (relation.source.as_memory(), relation.target.as_memory()) {
            neighbors.entry(a).or_default().insert(b);
            neighbors.entry(b).or_default().insert(a);
            if relation.kind.code() == crate::GraphRelationKind::DuplicateOf.code() {
                duplicates.insert(a);
                duplicates.insert(b);
            }
        }
        typed.push(Value::map([
            (
                "source_kind",
                Value::Unsigned(u64::from(relation.source.kind.code())),
            ),
            ("source_id", Value::bytes(relation.source.id)),
            (
                "target_kind",
                Value::Unsigned(u64::from(relation.target.kind.code())),
            ),
            ("target_id", Value::bytes(relation.target.id)),
            ("kind", Value::Unsigned(u64::from(relation.kind.code()))),
            ("origin", Value::Unsigned(u64::from(relation.origin.code()))),
            ("global_version", Value::Unsigned(relation.global_version)),
            ("graph_version", Value::Unsigned(relation.graph_version)),
        ]));
    }
    for &id in ids {
        if let Some(target) = cva.memory(id)?.superseded_by {
            if !ids.contains(&target) {
                return Err(fail("dangling/foreign supersession authority"));
            }
            duplicates.insert(id);
            duplicates.insert(target);
        }
    }
    let mut authorities = Vec::new();
    for id in duplicates {
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
        authorities.push(Value::map([
            ("memory_id", Value::bytes(id.0)),
            ("representative", Value::bytes(representative.0)),
            (
                "body_id",
                Value::bytes(cva.memory_body_id(representative)?.0),
            ),
        ]));
    }
    let adjacency = Value::Array(
        neighbors
            .clone()
            .into_iter()
            .map(|(id, ns)| {
                Value::map([
                    ("memory_id", Value::bytes(id.0)),
                    (
                        "neighbors",
                        Value::Array(ns.into_iter().map(|n| Value::bytes(n.0)).collect()),
                    ),
                ])
            })
            .collect(),
    );
    let typed = Value::Array(typed);
    let authority = Value::Array(authorities);
    let passes = cva.historical_completed_initial_dream_passes()?;
    let mut dream = Vec::new();
    for (id, pass) in passes {
        if id != pass.source_id
            || !ids.contains(&id)
            || pass.first_revision != 1
            || pass.turn > cva.rel_turn_count()
            || pass
                .accepted_settlement_turn
                .is_none_or(|p| p > cva.rel_turn_count())
            || pass
                .preexisting_ids
                .iter()
                .chain(&pass.roots)
                .any(|id| !ids.contains(id))
        {
            return Err(fail("invalid owner-local historical Dream proof"));
        }
        let (roots, duplicate_origin) = cva.initial_dream_committed_evidence(&pass)?;
        if roots != pass.roots {
            return Err(fail(
                "retained first-cycle roots conflict with accepted graph proof",
            ));
        }
        let representatives = cva.historical_dream_representatives(id, &pass.roots)?;
        dream.push(Value::map([
            ("source_id", Value::bytes(id.0)),
            ("pass_id", Value::text(pass.pass_id)),
            ("first_revision", Value::Unsigned(pass.first_revision)),
            ("duplicate_origin", Value::Bool(duplicate_origin)),
            ("turn", Value::Unsigned(pass.turn)),
            (
                "settlement_turn",
                pass.accepted_settlement_turn
                    .map_or(Value::Null, Value::Unsigned),
            ),
            (
                "graph_version_before",
                Value::Unsigned(pass.graph_version_before),
            ),
            (
                "preexisting_ids",
                Value::Array(
                    pass.preexisting_ids
                        .into_iter()
                        .map(|id| Value::bytes(id.0))
                        .collect(),
                ),
            ),
            (
                "roots",
                Value::Array(pass.roots.iter().map(|id| Value::bytes(id.0)).collect()),
            ),
            (
                "representatives",
                Value::Array(
                    representatives
                        .into_iter()
                        .map(|(id, rep)| {
                            Value::map([
                                ("root", Value::bytes(id.0)),
                                ("representative", Value::bytes(rep.0)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "mode",
                Value::text(if pass.roots.is_empty() {
                    "verified-no-link"
                } else {
                    "actual-first-cycle"
                }),
            ),
        ]));
    }
    Ok((
        Value::map([
            ("graph_version", Value::Unsigned(cva.graph_version())),
            (
                "memory_graph_version",
                Value::Unsigned(cva.memory_graph_version()),
            ),
            ("relationships", typed.clone()),
            (
                "relationship_digest",
                Value::bytes(typed.digest("reliquary:historical-relationships:v1")?),
            ),
            ("adjacency", adjacency.clone()),
            (
                "adjacency_digest",
                Value::bytes(adjacency.digest("reliquary:historical-adjacency:v1")?),
            ),
            ("duplicate_authority", authority.clone()),
            (
                "duplicate_authority_digest",
                Value::bytes(authority.digest("reliquary:historical-duplicate-authority:v1")?),
            ),
            ("dream_proof", Value::Array(dream)),
            (
                "excluded_endpoint_kinds",
                Value::Array(
                    excluded
                        .into_iter()
                        .map(|(kind, count)| {
                            Value::map([
                                ("kind", Value::Unsigned(u64::from(kind))),
                                ("count", Value::Unsigned(count)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]),
        neighbors,
    ))
}
fn vector_proof(
    cva: &mut Cva,
    order: &[crate::freshness::historical_planner::SourceKey],
    requested_complete: bool,
) -> Result<(Value, Vec<Value>), CvaError> {
    use crate::cva_semantic_source::SemanticSourceReadiness;
    let generation = cva
        .vector_generations
        .generations()
        .iter()
        .max_by_key(|g| g.vector_version)
        .copied();
    let mut sources = Vec::new();
    for (index, key) in order.iter().enumerate() {
        let node = cva
            .archive
            .nodes
            .get(&key.conversation_id, &key.node_id)
            .ok_or_else(|| fail("missing historical source"))?;
        sources.push(Value::map([
            ("node", input::node_binding(node)),
            ("position", Value::Unsigned(index as u64 + 1)),
        ]));
    }
    let mut fragments = Vec::new();
    let mut unavailable = 0u64;
    let mut invalid = 0u64;
    let mut mixed = 0u64;
    for fragment in cva.fragments() {
        let nodes = cva.archive.fragment_nodes_for(&fragment)?;
        let position = cva.semantic_fragment_position(fragment.id)?;
        let activity_nodes = nodes
            .iter()
            .filter(|n| {
                cva.activity_position_for_turn(&n.conversation_id, &n.id)
                    .is_some()
            })
            .count();
        mixed += u64::from(activity_nodes > 1);
        let readiness = generation
            .map(|g| cva.prepare_semantic_fragment(fragment.id, g.id))
            .transpose()?;
        let (mode, evidence) = match readiness {
            Some(SemanticSourceReadiness::Ready(source)) => {
                invalid += source.invalid_vectors as u64;
                let rows = source
                    .vectors
                    .into_iter()
                    .map(|v| {
                        let bytes: Vec<_> =
                            v.components.iter().flat_map(|f| f.to_le_bytes()).collect();
                        Value::map([
                            ("fragment_id", Value::bytes(v.fragment_id.0)),
                            ("row", Value::Unsigned(v.row)),
                            ("row_digest", Value::bytes(Sha256::digest(bytes))),
                        ])
                    })
                    .collect();
                (
                    "complete",
                    Value::map([
                        ("source_digest", Value::bytes(source.source_digest)),
                        ("generation_id", Value::bytes(source.generation.id.0)),
                        (
                            "profile_id",
                            Value::bytes(source.generation.compatibility_profile_id.0),
                        ),
                        (
                            "invalid_vectors",
                            Value::Unsigned(source.invalid_vectors as u64),
                        ),
                        ("usable_rows", Value::Array(rows)),
                    ]),
                )
            }
            _ => {
                unavailable += u64::from(position.is_some());
                (
                    if position.is_some() {
                        "unavailable"
                    } else {
                        "non-activity-only"
                    },
                    Value::map([(
                        "reason",
                        Value::text("missing-or-incomplete-fragment-evidence"),
                    )]),
                )
            }
        };
        fragments.push(Value::map([
            ("fragment_id", Value::bytes(fragment.id.0)),
            ("conversation_id", Value::text(&fragment.conversation_id)),
            ("start_node_id", Value::text(&fragment.start_node_id)),
            ("end_node_id", Value::text(&fragment.end_node_id)),
            (
                "represented_nodes",
                Value::Array(nodes.iter().map(input::node_binding).collect()),
            ),
            (
                "availability",
                position.map_or(Value::Null, Value::Unsigned),
            ),
            ("mode", Value::text(mode)),
            ("evidence", evidence.clone()),
            (
                "coverage_digest",
                Value::bytes(evidence.digest("reliquary:historical-fragment-coverage:v1")?),
            ),
        ]));
    }
    let mut profiles = Vec::new();
    for profile in cva.compatibility_profiles.profiles() {
        let descriptor = crate::compatibility_profile_codec::encode_profile(&profile)
            .map_err(|e| fail(&e.to_string()))?;
        profiles.push(Value::map([
            ("profile_id", Value::bytes(profile.id.0)),
            ("descriptor", Value::bytes(&descriptor)),
            (
                "descriptor_digest",
                Value::bytes(Sha256::digest(descriptor)),
            ),
        ]));
    }
    let mut targets = Vec::new();
    let mut ids = cva.memory_ids();
    ids.sort();
    for id in ids {
        let body = cva.memory_body_id(id)?;
        for profile in cva.compatibility_profiles.profiles() {
            let Some(location) = cva.memory_vector_location(profile.id, body) else {
                targets.push(Value::map([
                    ("memory_id", Value::bytes(id.0)),
                    ("body_id", Value::bytes(body.0)),
                    ("profile_id", Value::bytes(profile.id.0)),
                    ("status", Value::text("missing-binding")),
                ]));
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
            targets.push(Value::map([
                ("memory_id", Value::bytes(id.0)),
                ("body_id", Value::bytes(body.0)),
                ("profile_id", Value::bytes(profile.id.0)),
                ("binding_id", Value::bytes(set.id.0)),
                ("packed_id", Value::bytes(set.packed_vector_id.0)),
                ("row", Value::Unsigned(location.row)),
                ("row_digest", Value::bytes(Sha256::digest(row))),
                (
                    "status",
                    Value::text(if usable {
                        "usable"
                    } else {
                        "excluded-invalid-or-incompatible"
                    }),
                ),
            ]));
        }
    }
    let generations = match generation {
        Some(g) => Value::Array(vec![Value::map([
            ("generation_id", Value::bytes(g.id.0)),
            ("profile_id", Value::bytes(g.compatibility_profile_id.0)),
            ("archive_vector_id", Value::bytes(g.archive_vector_id.0)),
            (
                "source_archive_version",
                Value::Unsigned(g.source_archive_version),
            ),
            ("vector_version", Value::Unsigned(g.vector_version)),
        ])]),
        None => Value::Array(vec![]),
    };
    let embedding_pass = if requested_complete {
        super::embedding::manifest_pass(cva, &fragments)?
    } else {
        Value::text("not-requested")
    };
    Ok((
        Value::map([
            ("profiles", Value::Array(profiles)),
            ("selected_generations", generations),
            ("target_bindings", Value::Array(targets)),
            ("fragment_sources", Value::Array(fragments)),
            (
                "coverage",
                Value::map([
                    ("activity_sources", Value::Unsigned(order.len() as u64)),
                    (
                        "fragments",
                        Value::Unsigned(cva.archive.fragments.len() as u64),
                    ),
                    ("unavailable_fragments", Value::Unsigned(unavailable)),
                    ("invalid_source_rows", Value::Unsigned(invalid)),
                    ("mixed_source_fragments", Value::Unsigned(mixed)),
                ]),
            ),
            ("embedding_pass", embedding_pass),
        ]),
        sources,
    ))
}
#[cfg(test)]
pub(super) fn build(cva: &mut Cva) -> Result<Manifest, CvaError> {
    build_with_fill(cva, false)
}
pub(super) fn build_with_fill(
    cva: &mut Cva,
    requested_complete: bool,
) -> Result<Manifest, CvaError> {
    let owner = cva.historical_reconstruction_owner()?;
    // Validate ancestry before the timeline planner follows parent chains.
    input::required_nodes(cva, cva.rel_turn_count())?;
    let timeline = cva
        .historical_freshness_timeline()
        .map_err(|e| fail(&e.to_string()))?;
    let corpus_digest = input::corpus_digest(cva, cva.rel_turn_count())?;
    let import_id = historical_import_id(owner, corpus_digest);
    let admission = input::admission_proof(cva)?;
    let ids: BTreeSet<_> = cva.memory_ids().into_iter().collect();
    for &id in &ids {
        if !cva.freshness_is_publication_birth(id)
            || !cva.initial_dream_pass_settled(&crate::dream_freshness::initial_pass_id(owner, id))
        {
            return Err(fail(
                "import cohort lacks settled first-cycle/publication evidence",
            ));
        }
    }
    if admission.initial.is_empty() {
        return Err(fail("import has no supported active cohort"));
    }
    let replacement_ids = admission.initial.keys().copied().collect();
    let superseded = cva
        .freshness
        .provisional_import_events(&replacement_ids, 0, cva.rel_turn_count())
        .map_err(CvaError::Freshness)?;
    let mut untouched = Vec::new();
    for entry in cva
        .freshness
        .replay_entries()
        .map_err(CvaError::Freshness)?
    {
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
    let replacement = Value::map([
        (
            "superseded_event_ids",
            Value::Array(superseded.iter().map(Value::text).collect()),
        ),
        ("untouched_events", sorted(untouched)?),
        (
            "reconstructed_memories",
            Value::Array(
                admission
                    .initial
                    .keys()
                    .map(|id| Value::bytes(id.0))
                    .collect(),
            ),
        ),
        (
            "untouched_memories",
            Value::Array(
                ids.iter()
                    .filter(|id| !admission.initial.contains_key(id))
                    .map(|id| Value::bytes(id.0))
                    .collect(),
            ),
        ),
        (
            "isolation",
            Value::text("verified-disjoint-provisional-dream-effects-v1"),
        ),
    ]);
    let (graph, adjacency) = graph_proof(cva, &ids)?;
    let snapshot = cva.community_snapshot();
    let community = community_proof(snapshot.clone(), cva.memory_graph_version(), &ids)?;
    let memberships = snapshot
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
    let (vectors, sources) = vector_proof(cva, &timeline.activity_order, requested_complete)?;
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
    // Each retained payload family has an ordered accepted-prefix commitment.
    let mut prefixes = BTreeMap::<Vec<u8>, Vec<Value>>::new();
    let mut scan = crate::Container::open_read_only_for_scan(cva.container.path())?;
    scan.visit_payloads::<CvaError>(|_, payload| {
        prefixes
            .entry(payload.iter().take(8).copied().collect())
            .or_default()
            .push(Value::bytes(Sha256::digest(payload)));
        Ok(())
    })?;
    let families = Value::Array(
        prefixes
            .into_iter()
            .map(|(family, payloads)| {
                Value::map([
                    ("family", Value::bytes(family)),
                    ("payload_digests", Value::Array(payloads)),
                ])
            })
            .collect(),
    );
    if std::fs::metadata(cva.container.path())
        .map_err(|e| fail(&e.to_string()))?
        .len()
        != input_bytes
        || file_digest(cva.container.path())? != prefix
    {
        return Err(fail("ImportInputDrift: discard and refreeze the manifest"));
    }
    let token = Value::map([
        (
            "global_version",
            Value::Unsigned(cva.latest_global_version()),
        ),
        ("archive_version", Value::Unsigned(cva.archive_version())),
        ("activity_cut", Value::Unsigned(cva.rel_turn_count())),
        ("memory_version", Value::Unsigned(cva.memory_version())),
        ("graph_version", Value::Unsigned(cva.graph_version())),
        (
            "memory_graph_version",
            Value::Unsigned(cva.memory_graph_version()),
        ),
        ("vector_version", Value::Unsigned(cva.vector_version())),
        (
            "community_generation",
            cva.community_snapshot()
                .map_or(Value::Null, |s| Value::Unsigned(s.generation)),
        ),
        ("accepted_prefix_digest", Value::bytes(prefix)),
        ("accepted_prefix_bytes", Value::Unsigned(input_bytes)),
        ("accepted_payload_families", families),
        (
            "dream_proof_digest",
            Value::bytes(graph.digest("reliquary:historical-dream-input:v1")?),
        ),
        (
            "freshness_proof_digest",
            Value::bytes(replacement.digest("reliquary:historical-freshness-input:v1")?),
        ),
    ]);
    let value = Value::map([
        ("schema_version", Value::Unsigned(2)),
        ("policy_version", Value::Unsigned(3)),
        ("owner_uuid", Value::bytes(owner)),
        ("import_id", Value::bytes(import_id)),
        ("corpus_digest", Value::bytes(corpus_digest)),
        (
            "archive_cut",
            Value::map([
                ("archive_version", Value::Unsigned(cva.archive_version())),
                ("activity_count", Value::Unsigned(cva.rel_turn_count())),
                (
                    "last_source",
                    timeline.activity_order.last().map_or(Value::Null, |k| {
                        input::source_key(&k.conversation_id, &k.node_id)
                    }),
                ),
                (
                    "activity_order_digest",
                    Value::bytes(order.digest("reliquary:historical-activity-order:v1")?),
                ),
            ]),
        ),
        ("source_records", Value::Array(sources)),
        ("memory_records", Value::Array(admission.memories)),
        ("cohort_records", Value::Array(admission.cohorts)),
        ("graph_proof", graph),
        ("community_proof", community),
        ("vector_proof", vectors),
        (
            "scoring_policy",
            Value::map([
                ("base_descriptor", Value::bytes(base_bytes)),
                ("base_digest", Value::bytes(base_hash.finalize())),
                ("score_min", Value::Signed(-100)),
                ("score_max", Value::Signed(100)),
                (
                    "semantic_descriptor",
                    Value::bytes(
                        semantic
                            .canonical_descriptor()
                            .map_err(|e| fail(&e.to_string()))?
                            .as_bytes(),
                    ),
                ),
                (
                    "semantic_digest",
                    Value::bytes(semantic.digest().map_err(|e| fail(&e.to_string()))?),
                ),
                ("historical_descriptor", historical_policy),
                ("historical_digest", Value::bytes(policy_digest)),
                (
                    "event_order",
                    Value::text("admission-then-position-event-id-v1"),
                ),
            ]),
        ),
        ("replacement_proof", replacement),
        ("input_token", token),
    ]);
    let digest = value.digest("reliquary:historical-manifest:v1")?;
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
