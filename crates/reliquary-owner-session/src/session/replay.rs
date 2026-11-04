use super::*;

pub(super) fn open_common(
    container: &Container,
    graph_admission: &GraphAdmission,
    dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    replay_grouped_insomnia: bool,
) -> Result<CommonState, OwnerSessionError> {
    let memories = if replay_grouped_insomnia {
        MemoryStore::open_with_grouped_completion(container, |payload| {
            let completion = GroupedInsomniaCompletion::decode(payload)
                .map_err(|_| MemoryError::Corrupt("grouped completion"))?;
            if !completion.external_memory_refs.is_empty() {
                return Err(MemoryError::Corrupt(
                    "routed grouped completion requires owner authentication",
                ));
            }
            if !completion.routing_metadata.is_empty() {
                return Err(MemoryError::Corrupt(
                    "grouped routing metadata replay is unsupported",
                ));
            }
            let bodies = completion
                .bodies
                .into_iter()
                .map(|body| (body.id, body.bytes))
                .collect();
            Ok(Some((bodies, completion.memory_records)))
        })?
    } else {
        MemoryStore::open(container)?
    };
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
    if graph_admission.owner_id() != Some(container.owner_id()) {
        return Err(OwnerSessionError::Graph(GraphError::WrongOwner));
    }
    // The persisted owners, not an external list, establish replay closure.
    // Principal associations remain fail-closed until principal identity has
    // an owner-authenticated historical proof.
    let derived_admission = GraphAdmission::for_replay(
        container.owner_id(),
        memories.memory_ids().map(|id| id.0),
        [],
        perception.entities().map(|entity| (entity.id.0, false)),
        [],
        [],
    );
    let graph = GraphStore::open(container, &derived_admission)?;
    validate_graph_prefix(container)?;
    for relation in graph.history() {
        for node in [relation.source, relation.target] {
            let exists = match node {
                NodeRef::Memory(id) => memories.contains_memory(MemoryId(id)),
                NodeRef::Entity(id) => perception.entity(EntityId(id)).is_some(),
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
    let dream_admissions = dream_admissions.into_iter().collect::<Vec<_>>();
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
    Ok(CommonState {
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
    })
}

fn validate_graph_prefix(container: &Container) -> Result<(), OwnerSessionError> {
    let mut memories = std::collections::BTreeSet::new();
    let mut memory_bodies = std::collections::BTreeSet::new();
    let mut profiles = std::collections::BTreeSet::new();
    let mut packed_vectors = std::collections::BTreeSet::new();
    let mut entities = std::collections::BTreeSet::new();
    let mut graph_version = 0;
    for frame in container.read_frames()? {
        let payload = reliquary_container::decode_publication(&frame.payload)?
            .filter(|envelope| envelope.body.starts_with(b"RELMCRT1"))
            .map_or(frame.payload.as_slice(), |envelope| envelope.body);
        if payload.starts_with(b"RELMCRT1") {
            let record = reliquary_memory_core::decode_grouped_creation(payload)?;
            memories.insert(record.id.0);
            memory_bodies.insert(record.body_id.0);
        } else if payload.starts_with(b"RELINSC7") {
            let completion =
                GroupedInsomniaCompletion::decode(payload).map_err(OwnerSessionError::Record)?;
            for record in completion.memory_records {
                let record = reliquary_memory_core::decode_grouped_creation(&record)?;
                memories.insert(record.id.0);
                memory_bodies.insert(record.body_id.0);
            }
        }
        if let Some(profile) = reliquary_vector_store::profile::decode_profile(payload)? {
            profiles.insert(profile.id.0);
        }
        if let Some(packed) = reliquary_vector_store::store::packed_id_in_frame(payload)? {
            packed_vectors.insert(packed.0);
        }
        if let Some(mapping) = reliquary_vector_store::memory::sources_in_frame(payload)? {
            if mapping
                .bodies
                .iter()
                .any(|id| !memory_bodies.contains(&id.0))
            {
                return Err(OwnerSessionError::Record(
                    "Memory vector references a body outside its physical prefix".into(),
                ));
            }
            if !profiles.contains(&mapping.profile.0) || !packed_vectors.contains(&mapping.packed.0)
            {
                return Err(OwnerSessionError::Record(
                    "Memory vector references backing outside its physical prefix".into(),
                ));
            }
        }
        match reliquary_perception_store::source_publication(payload)? {
            Some(reliquary_perception_store::SourcePublication::EntityCreation(id)) => {
                entities.insert(id.0);
            }
            Some(reliquary_perception_store::SourcePublication::Mention(key))
                if !memories.contains(&key.memory_id) =>
            {
                return Err(OwnerSessionError::Record(
                    "Entity mention references a Memory outside its physical prefix".into(),
                ));
            }
            _ => {}
        }
        if let Some(relation) = reliquary_graph::relation_in_frame(payload)? {
            for endpoint in [relation.source, relation.target] {
                let found = match endpoint {
                    NodeRef::Memory(id) => memories.contains(&id),
                    NodeRef::Entity(id) => entities.contains(&id),
                };
                if !found {
                    return Err(OwnerSessionError::Record(
                        "Graph endpoint is absent from its physical prefix".into(),
                    ));
                }
            }
            graph_version = relation.graph_version;
        }
        if reliquary_graph::community_graph_version_in_frame(payload)?
            .is_some_and(|version| version > graph_version)
        {
            return Err(OwnerSessionError::Record(
                "Community references a Graph cut outside its physical prefix".into(),
            ));
        }
    }
    Ok(())
}
