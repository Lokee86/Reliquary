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
    let maintenance = OwnerMaintenanceStore::open(container).map_err(OwnerSessionError::Record)?;
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
