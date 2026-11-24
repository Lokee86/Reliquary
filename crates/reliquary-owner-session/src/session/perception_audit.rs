//! Metadata-first evidence capture for an authorized REL audit.
use super::perception_lifecycle::support::{has_direct_source, owner_hex, text_field};
use super::*;
use reliquary_freshness_store::FreshnessEventCut;
pub use reliquary_freshness_store::FreshnessState as PerceptionSourceGrade;
use reliquary_memory_core::{MemoryBody, MemoryRecord};
use reliquary_perception_store::observation::{
    ObservationActivity, ObservationCitation, ObservationDraft, ObservationId, ObservationSource,
    ObservationStanding,
};
use reliquary_perception_store::{EntityEvidenceChange, PRINCIPAL_ENTITY_KIND};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

mod progress;
pub use progress::*;

const MAX_CAPTURE_PAGE: usize = 128;
const MAX_NEIGHBORS: usize = 8;
const MAX_PARTICIPANTS: usize = 4;

/// Independently advancing source clocks. Metadata cursors remain meaningful
/// after checkpoint reopen and verified copies of the same owner history.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerceptionSourceCut {
    pub owner_id: [u8; 16],
    pub history_generation: [u8; 32],
    pub memory: u64,
    pub graph: u64,
    pub entity: u64,
    pub observation: u64,
    pub freshness: u64,
    pub settlements: u64,
    pub rel_turn: u64,
    pub freshness_event: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerceptionMemorySource {
    pub id: MemoryId,
    pub revision: u64,
    pub body_id: MemoryBodyId,
}

#[derive(Clone, Debug)]
pub struct PerceptionMemoryMetadata {
    pub source: PerceptionMemorySource,
    pub grade: PerceptionSourceGrade,
}

#[derive(Clone, Debug)]
pub struct PerceptionMemoryEvidence {
    pub metadata: PerceptionMemoryMetadata,
    pub title: String,
    pub content: String,
    pub source_lineage: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct PerceptionParticipantEvidence {
    pub entity_id: EntityId,
    pub entity_revision: u64,
    pub canonical_name: String,
    pub mention: MemoryEntityMentionKey,
    pub mention_revision: u32,
    pub retained_ordinal: Option<u64>,
}

/// An affected Memory can be unreadable at the new cut. Retaining its ID
/// allows the caller to invalidate old evidence and look at surviving links.
#[derive(Clone, Debug)]
pub struct PerceptionSourceChanges {
    pub memories: Vec<MemoryId>,
    pub derived: Vec<ObservationId>,
    pub ego_memories: Vec<MemoryId>,
    pub memory_through: u64,
    pub graph_through: u64,
    pub entity_through: u64,
    pub observation_through: u64,
    pub freshness_through: u64,
}

impl RelSession {
    pub fn perception_source_cut(&self) -> PerceptionSourceCut {
        let rel_turn = self.rel_turn_count();
        PerceptionSourceCut {
            owner_id: self.owner_id,
            history_generation: self.history_generation,
            memory: self.memory_version(),
            graph: self.graph_version(),
            entity: self.common.perception.version(),
            observation: self.perception_review_version(),
            freshness: self.freshness.memory_change_position(),
            settlements: self.common.dreams.settlement_count(),
            rel_turn,
            freshness_event: self.freshness.cut_through_turn(rel_turn).event_id_inclusive,
        }
    }

    /// A progress-only audit publication doesn't invalidate the standing sweep.
    pub fn perception_review_version(&self) -> u64 {
        self.observations.semantic_frontier()
    }

    pub fn perception_settlements_page(
        &self,
        after: u64,
        through: u64,
        limit: usize,
    ) -> Vec<(u64, MemoryId)> {
        self.common
            .dreams
            .settlements_after(after, through, limit.min(MAX_CAPTURE_PAGE))
            .collect()
    }

    pub fn perception_source_ids_page(
        &self,
        after: Option<MemoryId>,
        limit: usize,
    ) -> Vec<MemoryId> {
        self.common
            .memories
            .memory_ids_page(after, limit.min(MAX_CAPTURE_PAGE))
    }

    pub fn perception_memory_changes_page(
        &self,
        after: u64,
        through: u64,
        limit: usize,
    ) -> Vec<(u64, MemoryId)> {
        self.common
            .memories
            .changes_after(after, through, limit.min(MAX_CAPTURE_PAGE))
            .collect()
    }

    pub fn perception_source_metadata(
        &self,
        cut: &PerceptionSourceCut,
        ids: &[MemoryId],
    ) -> Result<Vec<PerceptionMemoryMetadata>, OwnerSessionError> {
        self.check_perception_source_cut(cut)?;
        if ids.len() > MAX_CAPTURE_PAGE {
            return Err(OwnerSessionError::ManagementLimit);
        }
        let records = ids
            .iter()
            .filter_map(|id| self.eligible_perception_memory(cut, *id))
            .collect::<Vec<_>>();
        let freshness = self
            .freshness
            .records_for_memories_at(
                &FreshnessEventCut {
                    rel_turn: cut.rel_turn,
                    event_id_inclusive: cut.freshness_event.clone(),
                },
                records.iter().map(|record| FreshnessMemoryId(record.id.0)),
            )
            .map_err(OwnerSessionError::Record)?;
        let policy = self.freshness.policy();
        records
            .into_iter()
            .filter_map(|record| {
                let score = freshness.get(&FreshnessMemoryId(record.id.0))?;
                Some(
                    score
                        .state_at_with_policy(cut.rel_turn, &policy)
                        .map(|grade| PerceptionMemoryMetadata {
                            source: PerceptionMemorySource {
                                id: record.id,
                                revision: record.revision,
                                body_id: record.body_id,
                            },
                            grade,
                        })
                        .map_err(|e| OwnerSessionError::Record(e.to_string())),
                )
            })
            .collect()
    }

    pub fn perception_memory_evidence(
        &self,
        cut: &PerceptionSourceCut,
        ids: &[MemoryId],
    ) -> Result<Vec<PerceptionMemoryEvidence>, OwnerSessionError> {
        self.perception_source_metadata(cut, ids)?
            .into_iter()
            .map(|metadata| {
                let (record, body) = self
                    .common
                    .memories
                    .revision_projection(metadata.source.id, Some(metadata.source.revision))?;
                let MemoryBody { title, content } = body
                    .ok_or(OwnerSessionError::ManagementEvidence)?
                    .into_owned();
                Ok(PerceptionMemoryEvidence {
                    metadata,
                    title,
                    content,
                    source_lineage: perception_lineage(record),
                })
            })
            .collect()
    }

    /// Follow only accepted, indexed changes since the last accepted page.
    pub fn perception_source_changes(
        &self,
        after: &PerceptionSourceCut,
        through: &PerceptionSourceCut,
        previous_run: &PerceptionSourceCut,
        per_stream: usize,
    ) -> Result<PerceptionSourceChanges, OwnerSessionError> {
        self.check_perception_source_cut(after)?;
        self.check_perception_source_cut(through)?;
        self.check_perception_source_cut(previous_run)?;
        let limit = per_stream.clamp(1, MAX_CAPTURE_PAGE);
        let memory = self
            .common
            .memories
            .changes_after(after.memory, through.memory, limit + 1)
            .collect::<Vec<_>>();
        let graph = self
            .common
            .graph
            .relation_changes_after(after.graph, through.graph, limit + 1)
            .collect::<Vec<_>>();
        let entity = self
            .common
            .perception
            .evidence_changes_after(after.entity, through.entity, limit + 1)
            .collect::<Vec<_>>();
        let freshness = self
            .freshness
            .memory_changes_after(after.freshness, through.freshness, limit + 1)
            .collect::<Vec<_>>();
        let mut sources = BTreeSet::new();
        for &(version, id) in memory.iter().take(limit) {
            let latest = self
                .common
                .memories
                .terminal_at_cut(id, through.memory)
                .map(|record| record.memory_version)
                .or_else(|| {
                    self.common
                        .memories
                        .record_at_cut(id, through.memory)
                        .map(|record| record.memory_version)
                });
            if latest == Some(version) {
                sources.insert(id);
            }
        }
        for relation in graph.iter().take(limit) {
            for endpoint in [relation.source, relation.target] {
                if let NodeRef::Memory(id) = endpoint {
                    sources.insert(MemoryId(id));
                }
            }
        }
        for &(_, change) in entity.iter().take(limit) {
            match change {
                EntityEvidenceChange::Mention(key) => {
                    sources.insert(MemoryId(key.memory_id));
                }
                EntityEvidenceChange::Entity(id) => {
                    sources.extend(
                        self.common
                            .perception
                            .resolved_mentions_for_entity(id)
                            .take(MAX_NEIGHBORS)
                            .map(|mention| MemoryId(mention.key.memory_id)),
                    );
                }
            }
        }
        let records = self.observations.records();
        let start = records
            .partition_point(|(receipt, _)| receipt.publication_version <= after.observation);
        let end = records
            .partition_point(|(receipt, _)| receipt.publication_version <= through.observation);
        let owned = &records[start.min(end)..end];
        let mut derived = BTreeSet::new();
        let mut observation_through = through.observation;
        for (index, (receipt, _)) in owned.iter().take(MAX_CAPTURE_PAGE + 1).enumerate() {
            if derived.len() >= limit || index == MAX_CAPTURE_PAGE {
                observation_through = receipt.publication_version - 1;
                break;
            }
            // A native batch can publish several claims. Include its entire
            // currently eligible set before advancing past that batch.
            for &(id, revision) in &receipt.revisions {
                if self.observation(id).is_some_and(|head| {
                    head.standing == ObservationStanding::Current
                        && head.revision.revision == revision
                }) {
                    derived.insert(id);
                }
            }
        }
        let freshness_ids = freshness
            .iter()
            .take(limit)
            .map(|(_, id)| MemoryId(id.0))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let previously_fresh = self
            .perception_source_metadata(previous_run, &freshness_ids)?
            .into_iter()
            .filter(|row| row.grade == PerceptionSourceGrade::Fresh)
            .map(|row| row.source.id)
            .collect::<BTreeSet<_>>();
        let ego_memories = self
            .perception_source_metadata(through, &freshness_ids)?
            .into_iter()
            .filter(|row| {
                row.grade == PerceptionSourceGrade::Fresh
                    && !sources.contains(&row.source.id)
                    && !previously_fresh.contains(&row.source.id)
            })
            .map(|row| row.source.id)
            .collect();
        Ok(PerceptionSourceChanges {
            memories: sources.into_iter().collect(),
            derived: derived.into_iter().collect(),
            ego_memories,
            memory_through: if memory.len() > limit {
                memory[limit - 1].0
            } else {
                through.memory
            },
            graph_through: if graph.len() > limit {
                graph[limit - 1].graph_version
            } else {
                through.graph
            },
            entity_through: if entity.len() > limit {
                entity[limit - 1].0
            } else {
                through.entity
            },
            observation_through,
            freshness_through: if freshness.len() > limit {
                freshness[limit - 1].0
            } else {
                through.freshness
            },
        })
    }

    pub fn perception_lexical_candidates(&self, query: &str, limit: usize) -> Vec<MemoryId> {
        self.common
            .dream_read_index
            .lexical_candidates(query, limit.min(MAX_CAPTURE_PAGE))
    }

    pub fn perception_graph_neighbors(&self, source: MemoryId) -> Vec<MemoryId> {
        let anchor = NodeRef::Memory(source.0);
        self.common
            .graph
            .incident_relations(anchor)
            .filter_map(|relation| match relation.source {
                NodeRef::Memory(id) if id != source.0 => Some(MemoryId(id)),
                _ => match relation.target {
                    NodeRef::Memory(id) if id != source.0 => Some(MemoryId(id)),
                    _ => None,
                },
            })
            .take(MAX_NEIGHBORS)
            .collect()
    }

    pub fn perception_relationship_sources(&self, entity: EntityId) -> Vec<MemoryId> {
        let local = owner_hex(&self.owner_id);
        let mut seen = BTreeSet::new();
        self.common
            .perception
            .relationships_for_local_participant(entity)
            .take(MAX_NEIGHBORS)
            .flat_map(|relationship| &relationship.evidence)
            .filter(|reference| reference.owner_id == local && seen.insert(reference.memory_id))
            .map(|reference| MemoryId(reference.memory_id))
            .take(MAX_NEIGHBORS)
            .collect()
    }

    pub fn has_equivalent_manual_relationship(&self, draft: &ObservationDraft) -> bool {
        let participants = draft.profile.participants();
        let Some(first) = participants.first() else {
            return false;
        };
        let owner = owner_hex(&self.owner_id);
        let proposition = normalized_summary(&draft.proposition);
        self.common
            .perception
            .relationships_for_local_participant(first.entity_id)
            .any(|row| {
                row.participants.len() == participants.len()
                    && normalized_summary(&row.summary) == proposition
                    && row.participants.iter().all(|prior| {
                        prior.entity.owner_id == owner
                            && participants.iter().any(|current| {
                                current.entity_id == prior.entity.entity_id
                                    && prior
                                        .role
                                        .as_deref()
                                        .zip(current.role.as_deref())
                                        .is_none_or(|(a, b)| {
                                            normalized_summary(a) == normalized_summary(b)
                                        })
                            })
                    })
            })
    }

    pub fn perception_source_activity(
        &self,
        cite: &ObservationCitation,
    ) -> Option<ObservationActivity> {
        let ObservationSource::Memory(source_id) = cite.source else {
            return None;
        };
        let current = self
            .common
            .memories
            .revision_record(MemoryId(source_id), None)
            .ok()?;
        if current.revision != cite.revision || current.body_id.0 != cite.body_id {
            return None;
        }
        let source = current.source_ref.as_ref()?;
        let conversation = source
            .content_source_conversation_id
            .as_deref()
            .or(source.grounding_source_conversation_id.as_deref())?;
        let node = source
            .content_source_node_id
            .as_deref()
            .unwrap_or(&source.source_node_id);
        let turn = self
            .archive
            .activity_position_for_turn(conversation, node)?;
        Some(ObservationActivity {
            turn,
            source: source_id,
        })
    }

    /// Supply exact active local mention anchors. The proposal can select a
    /// role for a participant while owner-session keeps authority over identity.
    pub fn perception_participant_evidence(
        &self,
        cut: &PerceptionSourceCut,
        source: PerceptionMemorySource,
    ) -> Result<Vec<PerceptionParticipantEvidence>, OwnerSessionError> {
        self.check_perception_source_cut(cut)?;
        let Some(record) = self.eligible_perception_memory(cut, source.id) else {
            return Ok(Vec::new());
        };
        if record.revision != source.revision || record.body_id != source.body_id {
            return Ok(Vec::new());
        }
        let (_, body) = self
            .common
            .memories
            .revision_projection(source.id, Some(source.revision))?;
        let body = body.ok_or(OwnerSessionError::ManagementEvidence)?;
        let mut seen = BTreeSet::new();
        Ok(self
            .common
            .perception
            .resolved_mentions_in_memory(source.id.0)
            .filter_map(|(entity_id, mention)| {
                let entity = self
                    .common
                    .perception
                    .entity_at_cut(entity_id, cut.entity)?;
                if entity.kind == PRINCIPAL_ENTITY_KIND || seen.contains(&entity_id) {
                    return None;
                }
                let text = text_field(&body, mention.key.field);
                let surface =
                    text.get(mention.key.start_byte as usize..mention.key.end_byte as usize)?;
                if !super::identity_evidence::quote_names_entity(
                    surface,
                    &entity.canonical_name,
                    &entity.aliases,
                ) {
                    return None;
                }
                seen.insert(entity_id);
                Some(PerceptionParticipantEvidence {
                    entity_id,
                    entity_revision: entity.revision,
                    canonical_name: entity.canonical_name.clone(),
                    mention: mention.key,
                    mention_revision: mention.revision,
                    retained_ordinal: mention.retained_ordinal,
                })
            })
            .take(MAX_PARTICIPANTS)
            .collect())
    }

    fn check_perception_source_cut(
        &self,
        cut: &PerceptionSourceCut,
    ) -> Result<(), OwnerSessionError> {
        if cut.owner_id != self.owner_id
            || cut.history_generation != self.history_generation
            || cut.memory > self.memory_version()
            || cut.graph > self.graph_version()
            || cut.entity > self.common.perception.version()
            || cut.observation > self.observation_publication_version()
            || cut.freshness > self.freshness.memory_change_position()
            || cut.settlements > self.common.dreams.settlement_count()
            || cut.rel_turn > self.rel_turn_count()
        {
            return Err(OwnerSessionError::ManagementStale);
        }
        Ok(())
    }

    fn eligible_perception_memory(
        &self,
        cut: &PerceptionSourceCut,
        id: MemoryId,
    ) -> Option<&MemoryRecord> {
        let record = self.common.memories.record_at_cut(id, cut.memory)?;
        let current = self.common.memories.revision_record(id, None).ok()?;
        let valid = !record.archived
            && record.superseded_by.is_none()
            && matches!(record.lifecycle_state.as_str(), "knowledge" | "canonical")
            && current.revision == record.revision
            && has_direct_source(record)
            && self.common.memories.contains_body_payload(record.body_id)
            && record
                .source_ref
                .as_ref()
                .is_some_and(|source| source.owner_id == owner_hex(&self.owner_id))
            && record.source_episode_id.is_none_or(|episode| {
                self.insomnia
                    .work(WorkEpisodeId(episode))
                    .is_none_or(|work| {
                        work.state == reliquary_insomnia_store::InsomniaWorkState::Complete
                    })
            });
        if !valid {
            return None;
        }
        let settled = self
            .common
            .dreams
            .settlement_position(id)
            .is_some_and(|pos| pos <= cut.settlements);
        // Migrated Web membership is backed by the converter's retained
        // Freshness authority. It can supply existing evidence without
        // registering a new settlement in the live cadence.
        let retained = self
            .retained_freshness_import()
            .is_some_and(|(_, snapshot)| {
                snapshot.settlement_cut <= cut.rel_turn
                    && snapshot
                        .records
                        .get(&FreshnessMemoryId(id.0))
                        .is_some_and(|entry| entry.admitted_at_turn.is_some())
            });
        (settled || retained).then_some(record)
    }
}

fn perception_lineage(record: &MemoryRecord) -> [u8; 32] {
    let Some(source) = &record.source_ref else {
        return record.id.0;
    };
    let original_turn = source
        .grounding_source_conversation_id
        .as_deref()
        .zip(source.grounding_source_node_id.as_deref())
        .or_else(|| {
            source
                .content_source_conversation_id
                .as_deref()
                .zip(source.content_source_node_id.as_deref())
        });
    let (namespace, node) = original_turn.unwrap_or((&source.owner_id, &source.source_node_id));
    let mut hash = Sha256::new();
    hash.update(b"reliquary-perception-source-v1\0");
    hash.update((namespace.len() as u64).to_le_bytes());
    hash.update(namespace.as_bytes());
    hash.update(node.as_bytes());
    hash.finalize().into()
}

fn normalized_summary(value: &str) -> String {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}
