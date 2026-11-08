//! Exact owner identity/version cuts at the physical effect prefix.
use super::*;
use crate::records::user_memory::{UserMemoryEffect, UserMemoryRecord};
pub(super) fn validate_graph_prefix(
    container: &Container,
    memory_store: &MemoryStore,
) -> Result<(), OwnerSessionError> {
    let mut memories = std::collections::BTreeSet::new();
    let mut memory_bodies = std::collections::BTreeSet::new();
    let mut birth_bodies = std::collections::BTreeMap::new();
    let mut profiles = std::collections::BTreeSet::new();
    let mut packed_vectors = std::collections::BTreeSet::new();
    let mut entities = std::collections::BTreeSet::new();
    let mut graph_version = 0;
    let mut memory_head = 0;
    for frame in container.read_frames()? {
        if let Some(frontier) =
            crate::records::dream_frontier::FrontierRecord::decode(&frame.payload)
                .map_err(OwnerSessionError::Record)?
        {
            if frontier.owner != container.owner_id()
                || frontier.graph_version != graph_version
                || frontier.memory_version != memory_head
                || std::iter::once(&frontier.source)
                    .chain(frontier.candidates.iter())
                    .any(|o| {
                        !memory_store
                            .record_at_cut(MemoryId(o.id), memory_head)
                            .is_some_and(|r| {
                                r.revision == o.revision && r.body_id.0 == o.body && !r.archived
                            })
                    })
            {
                return Err(OwnerSessionError::StaleDreamCut);
            }
            if let Some(payload) = &frontier.begin {
                let begin = crate::records::dream::dream_begin_codec::decode(payload)
                    .map_err(OwnerSessionError::Record)?;
                let population = super::dream_population::birth_population(
                    container,
                    memory_store,
                    begin.source_id,
                )?;
                if begin.population
                    != crate::records::dream::dream_begin_codec::BeginPopulation::ExplicitSet(
                        population,
                    )
                {
                    return Err(OwnerSessionError::StaleDreamCut);
                }
            }
        }
        if frame.payload.starts_with(b"RELDR")
            && ![
                b"RELDRM01",
                b"RELDRP02",
                b"RELDRG01",
                b"RELDRS01",
                b"RELDRF01",
                b"RELDRF02",
                b"RELDRB01",
                b"RELDRC01",
            ]
            .iter()
            .any(|magic| frame.payload.starts_with(magic.as_slice()))
        {
            return Err(OwnerSessionError::Record(
                "unsupported Dream carrier".into(),
            ));
        }
        if reliquary_container::decode_publication(&frame.payload)?.is_some_and(|p| {
            p.body.starts_with(b"RELDR")
                || p.body.starts_with(b"RELMCOR")
                || p.body.starts_with(b"RELMRET")
        }) {
            return Err(OwnerSessionError::Record(
                "wrapped grouped owner carrier is unsupported".into(),
            ));
        }
        let user_memory =
            UserMemoryRecord::decode(&frame.payload).map_err(OwnerSessionError::Record)?;
        if let Some(record) = &user_memory {
            validate_user_memory_prefix(
                container,
                memory_store,
                memory_head,
                graph_version,
                record,
            )?;
        }
        let batch = crate::records::dream_batch::DreamBatchRecord::decode(&frame.payload)
            .map_err(OwnerSessionError::Record)?;
        if let Some(record) = &batch {
            if record.owner != container.owner_id()
                || record.memory_version != memory_head
                || record.graph_version != graph_version
                || record.observed.iter().any(|o| {
                    !memory_store
                        .record_at_cut(MemoryId(o.id), memory_head)
                        .is_some_and(|r| r.revision == o.revision && r.body_id.0 == o.body)
                })
                || record.pairs.iter().any(|p| {
                    [p.source, p.candidate].iter().any(|id| {
                        !memory_store
                            .record_at_cut(MemoryId(*id), memory_head)
                            .is_some_and(|r| !r.archived)
                    })
                })
            {
                return Err(OwnerSessionError::StaleDreamCut);
            }
        }
        let children: Vec<&[u8]> = match &batch {
            Some(record) => record
                .lifecycles
                .iter()
                .map(|l| l.delta.as_slice())
                .chain(record.graph_records.iter().map(Vec::as_slice))
                .collect(),
            None => user_memory.as_ref().map_or_else(
                || vec![frame.payload.as_slice()],
                UserMemoryRecord::history_frames,
            ),
        };
        for child in children {
            let payload = reliquary_container::decode_publication(child)?
                .filter(|envelope| envelope.body.starts_with(b"RELMCRT1"))
                .map_or(child, |envelope| envelope.body);
            if payload.starts_with(b"RELMCRT1") {
                let record = reliquary_memory_core::decode_grouped_creation(payload)?;
                memory_head = record.memory_version;
                memories.insert(record.id.0);
                memory_bodies.insert(record.body_id.0);
                if record.revision == 1 {
                    birth_bodies.insert(record.id.0, record.body_id.0);
                }
            } else if payload.starts_with(b"RELINSC7") {
                let completion = GroupedInsomniaCompletion::decode(payload)
                    .map_err(OwnerSessionError::Record)?;
                for record in completion.memory_records {
                    let record = reliquary_memory_core::decode_grouped_creation(&record)?;
                    memory_head = record.memory_version;
                    memories.insert(record.id.0);
                    memory_bodies.insert(record.body_id.0);
                    if record.revision == 1 {
                        birth_bodies.insert(record.id.0, record.body_id.0);
                    }
                }
            }
            if let Some((_, version)) = reliquary_memory_core::history_change_in_frame(payload)? {
                memory_head = version;
            }
            if let Some(record) =
                crate::records::dream_empty::InitialDreamSettlement::decode(payload)
                    .map_err(OwnerSessionError::Record)?
            {
                if let Some(delta) = record.memory_delta {
                    memory_head = reliquary_memory_core::history_change_in_frame(&delta)?
                        .ok_or(OwnerSessionError::StaleDreamCut)?
                        .1;
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
                if !profiles.contains(&mapping.profile.0)
                    || !packed_vectors.contains(&mapping.packed.0)
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
            if let Some((owner, effect)) = reliquary_graph::user_association_in_frame(payload)? {
                if owner != container.owner_id()
                    || effect.graph_version != graph_version
                    || [effect.source, effect.target].iter().any(|endpoint| {
                        !memory_store
                            .record_at_cut(MemoryId(endpoint.id), memory_head)
                            .is_some_and(|r| {
                                r.revision == endpoint.revision
                                    && r.body_id.0 == endpoint.body
                                    && !r.archived
                            })
                    })
                {
                    return Err(OwnerSessionError::Record(
                        "user association lacks owner physical-prefix proof".into(),
                    ));
                }
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
            } else if let Some(group) =
                crate::records::dream_group::GroupedDreamRelation::decode(payload)
                    .map_err(OwnerSessionError::Record)?
            {
                if group.0.owner_id != container.owner_id()
                    || group.0.graph_version != graph_version
                    || [group.0.source, group.0.candidate].iter().any(|observed| {
                        !memory_store
                            .record_at_cut(observed.id, memory_head)
                            .is_some_and(|r| {
                                r.revision == observed.revision
                                    && r.body_id == observed.body_id
                                    && !r.archived
                            })
                    })
                {
                    return Err(OwnerSessionError::Record(
                        "grouped Dream relation lacks owner physical-prefix proof".into(),
                    ));
                }
                graph_version = group.relation().graph_version;
            } else if let Some(group) =
                crate::records::dream_supersession::GroupedDreamSupersession::decode(payload)
                    .map_err(OwnerSessionError::Record)?
            {
                if group.cut.owner_id != container.owner_id()
                    || group.cut.graph_version != graph_version
                    || [group.cut.source, group.cut.candidate]
                        .iter()
                        .any(|observed| {
                            !memory_store
                                .record_at_cut(observed.id, memory_head)
                                .is_some_and(|r| {
                                    r.revision == observed.revision
                                        && r.body_id == observed.body_id
                                        && !r.archived
                                })
                        })
                {
                    return Err(OwnerSessionError::Record(
                        "Dream supersession lacks owner physical-prefix proof".into(),
                    ));
                }
                graph_version = group.relation().graph_version;
                memory_head = reliquary_memory_core::history_change_in_frame(&group.delta)?
                    .ok_or(OwnerSessionError::StaleDreamCut)?
                    .1;
            }
            if reliquary_graph::rename_graph_version_in_frame(payload)?
                .is_some_and(|version| version != graph_version)
            {
                return Err(OwnerSessionError::Record(
                    "Community rename does not match its physical Graph prefix".into(),
                ));
            }
            if reliquary_graph::community_graph_version_in_frame(payload)?
                .is_some_and(|version| version > graph_version)
            {
                return Err(OwnerSessionError::Record(
                    "Community references a Graph cut outside its physical prefix".into(),
                ));
            }
        }
        if let Some(relation) = user_memory.as_ref().and_then(UserMemoryRecord::relation) {
            graph_version = relation.graph_version;
        }
    }
    Ok(())
}

fn validate_user_memory_prefix(
    container: &Container,
    memories: &MemoryStore,
    memory_head: u64,
    graph_version: u64,
    record: &UserMemoryRecord,
) -> Result<(), OwnerSessionError> {
    let invalid =
        || OwnerSessionError::Record("user Memory mutation differs from owner prefix".into());
    if record.owner != container.owner_id() || record.memory_cut != memory_head {
        return Err(invalid());
    }
    let previous = memories
        .record_at_cut(MemoryId(record.previous.id), memory_head)
        .filter(|previous| record.previous.matches(previous))
        .ok_or_else(invalid)?;
    match &record.effect {
        UserMemoryEffect::Correction {
            graph_cut,
            creation,
            ..
        } => {
            let replacement = reliquary_memory_core::decode_grouped_creation(creation)?;
            if *graph_cut != graph_version
                || previous.archived
                || previous.superseded_by.is_some()
                || replacement.body_id == previous.body_id
                || replacement.category != previous.category
                || replacement.memory_type != previous.memory_type
                || replacement.scope != previous.scope
            {
                return Err(invalid());
            }
        }
        UserMemoryEffect::Retirement { successor, .. } => {
            if !previous.archived
                || previous.authority_kind != "direct"
                || previous.source_ref.is_none()
                || previous.superseded_by != Some(MemoryId(successor.id))
                || !memories
                    .record_at_cut(MemoryId(successor.id), memory_head)
                    .is_some_and(|r| {
                        successor.matches(r) && !r.archived && r.superseded_by.is_none()
                    })
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}
