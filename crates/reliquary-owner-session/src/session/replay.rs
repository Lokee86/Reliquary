use super::*;
use crate::records::user_memory::UserMemoryRecord;

pub(super) fn open_common(
    container: &Container,
    graph_admission: &GraphAdmission,
    dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    replay_grouped_insomnia: bool,
) -> Result<CommonState, OwnerSessionError> {
    if graph_admission.owner_id() != Some(container.owner_id()) {
        return Err(OwnerSessionError::Graph(GraphError::WrongOwner));
    }
    open_common_for_owner(
        container,
        container.owner_id(),
        dream_admissions,
        replay_grouped_insomnia,
    )
}

pub(super) fn open_common_for_owner(
    container: &Container,
    expected_owner_id: [u8; 16],
    dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    replay_grouped_insomnia: bool,
) -> Result<CommonState, OwnerSessionError> {
    if expected_owner_id != container.owner_id() {
        return Err(OwnerSessionError::Graph(GraphError::WrongOwner));
    }
    let mut grouped_body_ids = BTreeSet::new();
    let mut user_memory_requests = BTreeMap::new();
    let mut memories = MemoryStore::open_with_management_groups(
        container,
        |payload| {
            if !replay_grouped_insomnia {
                return Err(MemoryError::Corrupt(
                    "PHY cannot replay REL grouped completion",
                ));
            }
            let completion = GroupedInsomniaCompletion::decode(payload)
                .map_err(|_| MemoryError::Corrupt("grouped completion"))?;
            if !completion.routing_metadata.is_empty() {
                return Err(MemoryError::Corrupt(
                    "grouped routing metadata replay is unsupported",
                ));
            }
            for bytes in &completion.memory_records {
                grouped_body_ids
                    .insert(reliquary_memory_core::decode_grouped_creation(bytes)?.body_id);
            }
            let bodies = completion
                .bodies
                .into_iter()
                .map(|body| (body.id, body.bytes))
                .collect();
            Ok(Some((bodies, completion.memory_records)))
        },
        |payload| {
            crate::records::dream_supersession::GroupedDreamSupersession::decode(payload)
                .map(|record| record.map(|record| record.memory_event()))
                .map_err(|_| MemoryError::Corrupt("invalid Dream supersession carrier"))
        },
        |payload| {
            crate::records::dream_empty::InitialDreamSettlement::decode(payload)
                .map(|record| record.and_then(|r| r.memory_event()))
                .map_err(|_| MemoryError::Corrupt("invalid initial Dream lifecycle"))
        },
        |payload| {
            let record = crate::records::dream_batch::DreamBatchRecord::decode(payload)
                .map_err(|_| MemoryError::Corrupt("invalid Dream batch"))?
                .ok_or(MemoryError::Corrupt("missing Dream batch"))?;
            if record.owner != container.owner_id() {
                return Err(MemoryError::Corrupt("foreign Dream batch"));
            }
            Ok(record.lifecycles.iter().map(|l| l.event()).collect())
        },
        |payload| {
            if !replay_grouped_insomnia {
                return Err(MemoryError::Corrupt(
                    "PHY cannot replay user REL Memory mutation",
                ));
            }
            let record = UserMemoryRecord::decode(payload)
                .map_err(|_| MemoryError::Corrupt("invalid grouped user Memory mutation"))?
                .ok_or(MemoryError::Corrupt("missing grouped user Memory mutation"))?;
            if record.owner != container.owner_id()
                || user_memory_requests.contains_key(&record.key)
            {
                return Err(MemoryError::Corrupt(
                    "foreign or duplicate user Memory mutation",
                ));
            }
            let effect = record
                .memory_effect()
                .map_err(|_| MemoryError::Corrupt("invalid user Memory mutation effect"))?;
            user_memory_requests.insert(record.key.clone(), record);
            Ok(effect)
        },
    )?;
    let perception = PerceptionStore::open(container)?;
    for mention in perception.mentions() {
        let history = memories
            .history(MemoryId(mention.key.memory_id))
            .map_err(|_| {
                OwnerSessionError::Record("Entity mention references an absent owner Memory".into())
            })?;
        if let Some(revision) = history.revisions.first()
            && let reliquary_memory_core::PayloadState::Available(body) = &revision.payload
        {
            let text = match mention.key.field {
                reliquary_perception_store::MemoryTextField::Title => &body.title,
                reliquary_perception_store::MemoryTextField::Content => &body.content,
            };
            let start = mention.key.start_byte as usize;
            let end = mention.key.end_byte as usize;
            if text.get(start..end).is_none() {
                return Err(OwnerSessionError::Record(
                    "Entity mention span is outside its Memory body".into(),
                ));
            }
        }
    }
    // The persisted owners, not an external list, establish replay closure.
    // Principal identity is validated by the owner-local Entity replay.
    let derived_admission = GraphAdmission::for_replay(
        container.owner_id(),
        memories.memory_ids().map(|id| id.0),
        [],
        perception.entities().map(|entity| {
            (
                entity.id.0,
                entity.kind == reliquary_perception_store::PRINCIPAL_ENTITY_KIND,
            )
        }),
        perception.historical_entities().map(|id| id.0),
        [],
    );
    let graph = GraphStore::open_with_dream_batches(
        container,
        &derived_admission,
        |payload| {
            if payload.starts_with(b"RELMCOR") {
                UserMemoryRecord::decode(payload)
                    .map(|record| record.and_then(|record| record.relation()))
                    .map_err(|_| GraphError::CorruptRecord("invalid grouped user supersession"))
            } else if payload.starts_with(b"RELDRS") {
                crate::records::dream_supersession::GroupedDreamSupersession::decode(payload)
                    .map(|record| record.map(|record| record.relation()))
                    .map_err(|_| GraphError::CorruptRecord("invalid Dream supersession carrier"))
            } else {
                crate::records::dream_group::GroupedDreamRelation::decode(payload)
                    .map(|record| record.map(|record| record.relation()))
                    .map_err(|_| GraphError::CorruptRecord("invalid grouped Dream carrier"))
            }
        },
        |payload, graph| {
            let record = crate::records::dream_batch::DreamBatchRecord::decode(payload)
                .map_err(|_| GraphError::CorruptRecord("invalid Dream batch"))?
                .ok_or(GraphError::CorruptRecord("missing Dream batch"))?;
            if record.owner != container.owner_id() || record.graph_version != graph.graph_version()
            {
                return Err(GraphError::WrongOwner);
            }
            record
                .validate_direct(graph)
                .map_err(|_| GraphError::CorruptRecord("invented Dream direct evidence"))?;
            record
                .graph_records
                .iter()
                .map(|r| reliquary_graph::dream_relation_in_frame(r, container.owner_id()))
                .collect()
        },
    )?;
    super::replay_prefix::validate_graph_prefix(container, &memories)?;
    for relation in graph.history() {
        for node in [relation.source, relation.target] {
            let exists = match node {
                NodeRef::Memory(id) => memories.contains_memory(MemoryId(id)),
                NodeRef::Entity(id) => {
                    perception.entity(EntityId(id)).is_some()
                        || perception.entity_tombstone(EntityId(id)).is_some()
                }
            };
            if !exists {
                return Err(OwnerSessionError::Record(
                    "Graph endpoint is absent from the owner stores".into(),
                ));
            }
        }
    }
    let communities = CommunityStore::open(container, &graph)?;
    let ego = EgoStore::open(container)?;
    let packed_vectors = PackedVectorStore::open(container)?;
    let profiles = CompatibilityProfileStore::open(container)?;
    let memory_vectors = MemoryVectorStore::open(container, &packed_vectors, &profiles)?;
    if memory_vectors
        .referenced_body_ids()
        .any(|id| !memories.references_body(reliquary_memory_core::MemoryBodyId(id.0)))
    {
        return Err(OwnerSessionError::Record(
            "Memory vector references a body absent from owner history".into(),
        ));
    }
    let maintenance = OwnerMaintenanceStore::open(container).map_err(OwnerSessionError::Record)?;
    super::entity_prune::validate_entity_prune(&perception, &maintenance)?;
    super::relationship_prune::validate_relationship_prune(&perception, &maintenance)?;
    let pruned_bodies = super::maintenance_prune_proof::validate_memory_prune(
        container,
        &memories,
        &memory_vectors,
        &maintenance,
        &grouped_body_ids,
    )?;
    memories.retain_pruned_body_fence(pruned_bodies);
    let mut dream_admissions = dream_admissions.into_iter().collect::<Vec<_>>();
    for frame in container.read_frames()? {
        let frontier = crate::records::dream_frontier::FrontierRecord::decode(&frame.payload)
            .map_err(OwnerSessionError::Record)?;
        let (begin, frontier_cut) = match frontier.as_ref().and_then(|record| record.begin.as_ref())
        {
            Some(payload) => (
                Some(
                    crate::records::dream::dream_begin_codec::decode(payload)
                        .map_err(OwnerSessionError::Record)?,
                ),
                frontier.as_ref().map(|record| record.memory_version),
            ),
            None if frame.payload.starts_with(b"CVDREAM2") => (
                Some(
                    crate::records::dream::dream_begin_codec::decode(&frame.payload)
                        .map_err(OwnerSessionError::Record)?,
                ),
                None,
            ),
            None => (None, None),
        };
        let Some(begin) = begin else { continue };
        let population = match &begin.population {
            crate::records::dream::dream_begin_codec::BeginPopulation::CompactCut { .. } => {
                super::dream_population::birth_population(container, &memories, begin.source_id)?
            }
            crate::records::dream::dream_begin_codec::BeginPopulation::ExplicitSet(ids) => {
                ids.clone()
            }
        };
        let memory_cut = frontier_cut.unwrap_or_else(|| match begin.population {
            crate::records::dream::dream_begin_codec::BeginPopulation::CompactCut {
                memory_cut,
                ..
            } => memory_cut,
            crate::records::dream::dream_begin_codec::BeginPopulation::ExplicitSet(_) => memories
                .first_memory_version(begin.source_id)
                .unwrap_or_default(),
        });
        let derived = DreamBeginAdmission {
            owner_uuid: container.owner_id(),
            pass_id: begin.pass_id.clone(),
            source_id: begin.source_id,
            turn: begin.turn,
            graph_version_before: begin.graph_version_before,
            preexisting_ids: population,
            memory_cut,
        };
        // Preserve an explicit trusted admission when one was supplied. The
        // Dream store below validates every durable Begin field against it;
        // notably, the legacy explicit-set encoding does not carry its
        // physical Memory cut, so replacing that cut with an inferred value
        // would reject otherwise valid retained evidence.
        if !dream_admissions.iter().any(|a| a.pass_id == begin.pass_id) {
            dream_admissions.push(derived);
        }
    }

    for admission in &dream_admissions {
        if admission.owner_uuid != container.owner_id()
            || !memories.contains_memory(admission.source_id)
            || admission
                .preexisting_ids
                .iter()
                .any(|id| !memories.contains_memory(*id))
            || admission.memory_cut > memories.memory_version()
            || admission.graph_version_before > graph.graph_version()
            || (admission.memory_cut != 0
                && std::iter::once(&admission.source_id)
                    .chain(admission.preexisting_ids.iter())
                    .any(|id| {
                        memories
                            .first_memory_version(*id)
                            .is_none_or(|version| version > admission.memory_cut)
                    }))
        {
            return Err(OwnerSessionError::Record(
                "Dream admission references unavailable owner history".into(),
            ));
        }
    }
    super::dream_prefix::validate_compact_dream_prefix(container, &dream_admissions)?;
    let dreams =
        DreamEvidenceStore::open(container, dream_admissions).map_err(OwnerSessionError::Record)?;
    super::dream_initial_replay::validate(container, &dreams, &memories, &graph)?;
    let dream_pairs = super::dream_pairs::DreamPairs::open(container)?;
    let dream_read_index = super::dream_read_index::DreamReadIndex::build(
        &memories,
        &packed_vectors,
        &profiles,
        &memory_vectors,
    )?;
    Ok(CommonState {
        user_memory_requests,
        memories,
        perception,
        graph,
        communities,
        ego,
        packed_vectors,
        profiles,
        memory_vectors,
        maintenance,
        dreams,
        dream_pairs,
        dream_read_index,
    })
}
