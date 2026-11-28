use super::*;
use reliquary_archive::EpisodeId;
use reliquary_memory_core::MemoryRecord;

/// Check exact stored provenance against the Archive at the specified cut.
/// Memory and Archive clocks are independent. Historical publication replay
/// passes its physical Archive prefix, while content reads pass today's head.
#[derive(Clone, Copy)]
pub(super) struct SourceEligibility<'a> {
    archive: &'a Archive,
    owner: [u8; 16],
    cut: u64,
}

impl<'a> SourceEligibility<'a> {
    pub(super) fn new(archive: &'a Archive, owner: [u8; 16], cut: u64) -> Self {
        Self {
            archive,
            owner,
            cut,
        }
    }

    pub(super) fn cut(self) -> u64 {
        self.cut
    }

    pub(super) fn has_removals(self) -> bool {
        self.archive.source_removal_version() != 0
    }

    pub(super) fn allows(self, record: &MemoryRecord) -> bool {
        if self.cut == 0 || self.archive.source_removal_version() == 0 {
            return true;
        }
        let source = record.source_ref.as_ref();
        if source.is_some_and(|source| !matches_owner(&source.owner_id, &self.owner)) {
            return true;
        }
        let direct = [
            (
                record.content_source_conversation_id.as_deref(),
                record.content_source_node_id.as_deref(),
            ),
            (
                record.grounding_source_conversation_id.as_deref(),
                record.grounding_source_node_id.as_deref(),
            ),
        ];
        let explicit = source.into_iter().flat_map(|source| {
            [
                (
                    source.content_source_conversation_id.as_deref(),
                    source.content_source_node_id.as_deref(),
                ),
                (
                    source.grounding_source_conversation_id.as_deref(),
                    source.grounding_source_node_id.as_deref(),
                ),
            ]
        });
        if direct
            .into_iter()
            .chain(explicit)
            .any(|(conversation, node)| {
                matches!((conversation, node), (Some(conversation), Some(node))
                if self.archive.source_removed_at(conversation, node, self.cut))
            })
        {
            return false;
        }
        let typed_episode = source.map(|source| source.source_episode_id);
        let legacy_episode = record
            .source_episode_id
            .filter(|id| Some(*id) != typed_episode);
        [typed_episode, legacy_episode]
            .into_iter()
            .flatten()
            .map(EpisodeId)
            .filter_map(|id| {
                let episode = self.archive.episode(id)?;
                let version = self.archive.episode_archive_version(id)?;
                (version <= self.cut).then_some((episode, version))
            })
            .all(|(episode, version)| {
                !self.archive.source_range_removed_at(
                    &episode.conversation_id,
                    &episode.start_node_id,
                    &episode.end_node_id,
                    version,
                    self.cut,
                )
            })
    }

    pub(super) fn allows_entity(
        self,
        entity: &Entity,
        memories: &MemoryStore,
        memory_cut: u64,
    ) -> bool {
        if self.archive.source_removal_version() == 0 {
            return true;
        }
        // Native mention admission records the exact authoring Memory in the
        // Entity publication identity. Manual Entities retain their wording.
        let Some(source) = entity
            .mutation_id
            .strip_prefix("entity-mention:")
            .and_then(|mutation| mutation.split_once(':'))
            .and_then(|(memory, _)| parse_memory_id(memory))
        else {
            return true;
        };
        memories
            .revision_records_at_cut(MemoryId(source), memory_cut)
            .and_then(|records| records.first())
            .is_none_or(|record| self.allows(record))
    }
}

impl RelSession {
    pub(super) fn source_eligibility(&self) -> SourceEligibility<'_> {
        SourceEligibility::new(&self.archive, self.owner_id, self.archive.archive_version())
    }

    /// Check a retained revision against current source removal without reading
    /// its body. Callers also validate any current-head or delivery requirements.
    pub fn memory_revision_source_eligible(&self, id: MemoryId, revision: u64) -> bool {
        self.common
            .memories
            .revision_record(id, Some(revision))
            .is_ok_and(|record| self.source_eligibility().allows(record))
    }

    pub(super) fn memory_source_removed(&self, id: MemoryId) -> bool {
        self.common
            .memories
            .revision_records_at_cut(id, self.memory_version())
            .and_then(|records| records.last())
            .is_some_and(|record| !self.source_eligibility().allows(record))
    }

    pub(super) fn eligible_memory_history(
        &self,
        id: MemoryId,
        cut: u64,
    ) -> Result<Option<MemoryHistory>, OwnerSessionError> {
        let Some(records) = self.common.memories.revision_records_at_cut(id, cut) else {
            return Ok(None);
        };
        let eligibility = self.source_eligibility();
        if records.iter().all(|record| eligibility.allows(record)) {
            return Ok(Some(self.common.memories.history_at_cut(id, cut)?));
        }
        let tombstone = self.management_read().memory_terminal(id, cut);
        let revisions = records
            .iter()
            .map(|record| {
                if eligibility.allows(record) {
                    self.common.memories.revision(id, record.revision)
                } else {
                    let mut record = record.clone();
                    record.temporal_inference = None;
                    Ok(MemoryRevision {
                        record,
                        payload: reliquary_memory_core::PayloadState::Absent,
                    })
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(MemoryHistory {
            revisions,
            tombstone,
        }))
    }

    pub(super) fn relation_source_eligible(&self, relation: &Relation) -> bool {
        if self.archive.source_removal_version() == 0 {
            return true;
        }
        [relation.source, relation.target]
            .into_iter()
            .all(|node| match node {
                NodeRef::Memory(id) => !self.memory_source_removed(MemoryId(id)),
                NodeRef::Entity(id) => self.entity_source_eligible(EntityId(id)),
            })
    }

    pub(super) fn entity_source_eligible(&self, id: EntityId) -> bool {
        self.common
            .perception
            .entity(id)
            .is_none_or(|entity| self.entity_revision_source_eligible(entity))
    }

    pub(super) fn entity_revision_source_eligible(&self, entity: &Entity) -> bool {
        self.source_eligibility().allows_entity(
            entity,
            &self.common.memories,
            self.memory_version(),
        )
    }

    pub(super) fn relationship_source_eligible(&self, relationship: &Relationship) -> bool {
        if self.archive.source_removal_version() == 0 {
            return true;
        }
        relationship.evidence.iter().all(|entry| {
            !matches_owner(&entry.owner_id, &self.owner_id)
                || !self.memory_source_removed(MemoryId(entry.memory_id))
        }) && relationship.participants.iter().all(|entry| {
            !matches_owner(&entry.entity.owner_id, &self.owner_id)
                || self.entity_source_eligible(entry.entity.entity_id)
        })
    }

    pub(super) fn eligible_community_snapshot(&self) -> Option<&CommunitySnapshot> {
        let snapshot = self.common.communities.current()?;
        if self.archive.source_removal_version() == 0 {
            return Some(snapshot);
        }
        snapshot
            .communities
            .iter()
            .flat_map(|group| &group.members)
            .all(|id| !self.memory_source_removed(MemoryId(*id)))
            .then_some(snapshot)
    }
}

fn matches_owner(hex: &str, bytes: &[u8; 16]) -> bool {
    const HEX: &[u8] = b"0123456789abcdef";
    hex.as_bytes().iter().copied().eq(bytes
        .iter()
        .flat_map(|byte| [HEX[(byte >> 4) as usize], HEX[(byte & 15) as usize]]))
}

fn parse_memory_id(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 || !value.is_ascii() {
        return None;
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[2 * index..2 * index + 2], 16).ok()?;
    }
    Some(bytes)
}
