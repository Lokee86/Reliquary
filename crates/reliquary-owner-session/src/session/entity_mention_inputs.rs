use super::*;
use reliquary_memory_core::PayloadState;
use reliquary_perception_store::{MemoryEntityResolutionStatus, PRINCIPAL_ENTITY_KIND};

const MEMORY_SCAN_LIMIT: usize = 128;
const GRAPH_NEIGHBOR_LIMIT: usize = 16;
const LEXICAL_BYTES_LIMIT: usize = 512 * 1024;

/// Read facts used only to rank candidates within this owner.
pub struct EntityMentionInputs {
    pub revision: MemoryRevision,
    pub entities: Vec<Entity>,
    pub existing: Option<MemoryEntityResolution>,
    pub source_entities: Vec<EntityId>,
    pub supporting_memories: Vec<EntityCandidateMemory>,
}

pub struct EntityCandidateMemory {
    pub id: MemoryId,
    pub lexical_text: Option<String>,
    pub graph_neighbor: bool,
    pub entities: Vec<EntityId>,
}

impl CommonState {
    fn entity_mention_inputs(
        &self,
        key: MemoryEntityMentionKey,
    ) -> Result<EntityMentionInputs, OwnerSessionError> {
        let source = MemoryId(key.memory_id);
        let revision = self.memories.current(source)?;
        let node = NodeRef::Memory(source.0);
        let mut neighbors = self
            .graph
            .outgoing(node)
            .into_iter()
            .chain(self.graph.incoming(node))
            .filter(|edge| edge.active)
            .filter_map(|edge| {
                match if edge.source == node {
                    edge.target
                } else {
                    edge.source
                } {
                    NodeRef::Memory(id) if id != source.0 => Some(MemoryId(id)),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        neighbors.sort_unstable();
        neighbors.dedup();
        neighbors.truncate(GRAPH_NEIGHBOR_LIMIT);
        let mut postings = std::collections::BTreeMap::<MemoryId, usize>::new();
        if let Some(terms) = self.dream_read_index.source_terms.get(&source) {
            let mut lanes: Vec<_> = terms
                .iter()
                .filter_map(|term| self.dream_read_index.terms.get(term).map(|ids| (term, ids)))
                .collect();
            lanes.sort_by_key(|(term, ids)| (ids.len(), *term));
            for (_, ids) in lanes.into_iter().take(32) {
                for id in ids.iter().filter(|id| **id != source).take(32) {
                    *postings.entry(*id).or_default() += 1;
                }
            }
        }
        let mut ranked: Vec<_> = postings.into_iter().collect();
        ranked.sort_by_key(|(id, hits)| (std::cmp::Reverse(*hits), *id));
        let lexical_ids: Vec<_> = ranked
            .into_iter()
            .take(MEMORY_SCAN_LIMIT)
            .map(|(id, _)| id)
            .collect();
        let mut ids = neighbors.clone();
        ids.extend_from_slice(&lexical_ids);
        ids.sort_unstable();
        ids.dedup();
        let ids: std::collections::BTreeSet<_> =
            ids.into_iter().filter(|id| *id != source).collect();
        let entity_associations = |memory: MemoryId| {
            let node = NodeRef::Memory(memory.0);
            let mut entities: Vec<_> = self
                .perception
                .mentions_for_memory(memory.0)
                .take(64)
                .filter_map(|mention| match mention.status {
                    MemoryEntityResolutionStatus::Resolved { entity_id, .. } => Some(entity_id),
                    _ => None,
                })
                .collect();
            entities.extend(
                self.graph
                    .outgoing(node)
                    .into_iter()
                    .chain(self.graph.incoming(node))
                    .filter(|edge| {
                        edge.active && edge.kind == reliquary_graph::RelationKind::EntityAssociation
                    })
                    .filter_map(|edge| {
                        match if edge.source == node {
                            edge.target
                        } else {
                            edge.source
                        } {
                            NodeRef::Entity(id) => Some(EntityId(id)),
                            _ => None,
                        }
                    }),
            );
            entities.retain(|id| {
                self.perception
                    .entity(*id)
                    .is_some_and(|entity| entity.kind != PRINCIPAL_ENTITY_KIND)
            });
            entities.sort_unstable();
            entities.dedup();
            entities
        };
        let source_entities = entity_associations(source);
        let mut supporting_memories = Vec::new();
        let mut bytes = 0;
        for id in ids {
            let Ok(memory) = self.memories.current(id) else {
                continue;
            };
            if memory.record.archived {
                continue;
            }
            let entities = entity_associations(id);
            if entities.is_empty() {
                continue;
            }
            let graph_neighbor = neighbors.binary_search(&id).is_ok();
            let lexical_text = if lexical_ids.contains(&id)
                && let PayloadState::Available(body) = memory.payload
                && bytes + body.title.len() + body.content.len() < LEXICAL_BYTES_LIMIT
            {
                bytes += body.title.len() + body.content.len() + 1;
                Some(format!("{}\n{}", body.title, body.content))
            } else {
                None
            };
            supporting_memories.push(EntityCandidateMemory {
                id,
                lexical_text,
                graph_neighbor,
                entities,
            });
        }
        Ok(EntityMentionInputs {
            revision,
            entities: self.perception.entities().cloned().collect(),
            existing: self.perception.mention(key).cloned(),
            source_entities,
            supporting_memories,
        })
    }
}

macro_rules! mention_inputs {
    ($($owner:ty),+) => {$(impl $owner {
        pub fn entity_mention_inputs(&self, key: MemoryEntityMentionKey) -> Result<EntityMentionInputs, OwnerSessionError> {
            self.common.entity_mention_inputs(key)
        }
    })+};
}
mention_inputs!(RelSession, PhySession, PhyWriter);
