//! Owner-local evidence for the private, settled PHY Memory Web.

use super::*;
use reliquary_memory_core::{MemoryBody, MemoryRecord};

const MAX_PAGE: usize = 125;
const MAX_CHANGES_PER_STREAM: usize = MAX_PAGE / 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PhyAuditCut {
    pub owner_id: [u8; 16],
    pub history_generation: [u8; 32],
    pub memory: u64,
    pub graph: u64,
    pub settlements: u64,
}

impl PhyAuditCut {
    pub fn is_at_or_before(self, current: Self) -> bool {
        self.owner_id == current.owner_id
            && self.history_generation == current.history_generation
            && self.memory <= current.memory
            && self.graph <= current.graph
            && self.settlements <= current.settlements
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PhyAuditCursor {
    pub memory: u64,
    pub settlements: u64,
}

impl From<PhyAuditCut> for PhyAuditCursor {
    fn from(cut: PhyAuditCut) -> Self {
        Self {
            memory: cut.memory,
            settlements: cut.settlements,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PhyAuditChange {
    pub memory: MemoryId,
    pub previous_revision: Option<u64>,
    pub current_revision: Option<u64>,
}

pub struct PhyAuditChangesPage {
    pub changes: Vec<PhyAuditChange>,
    pub next: PhyAuditCursor,
}

pub struct PhyAuditInitialPage {
    pub sources: Vec<MemoryId>,
    pub next: Option<MemoryId>,
}

macro_rules! phy_audit_reads {
    ($session:ty) => {
        impl $session {
            pub fn phy_audit_cut(&self) -> PhyAuditCut {
                PhyAuditCut {
                    owner_id: self.owner_id,
                    history_generation: self.history_generation,
                    memory: self.memory_version(),
                    graph: self.graph_version(),
                    settlements: self.common.dreams.settlement_count(),
                }
            }

            pub fn phy_audit_initial_page(
                &self,
                cut: PhyAuditCut,
                after: Option<MemoryId>,
            ) -> Result<PhyAuditInitialPage, OwnerSessionError> {
                let read = self.phy_audit_read(cut)?;
                let ids = self.common.memories.memory_ids_page(after, MAX_PAGE);
                let next = (ids.len() == MAX_PAGE).then(|| ids[MAX_PAGE - 1]);
                let sources = ids
                    .into_iter()
                    .filter(|id| read.current(*id).is_some())
                    .collect();
                Ok(PhyAuditInitialPage { sources, next })
            }

            pub fn phy_audit_changes_page(
                &self,
                previous: PhyAuditCut,
                through: PhyAuditCut,
                after: PhyAuditCursor,
            ) -> Result<PhyAuditChangesPage, OwnerSessionError> {
                self.phy_audit_read(previous)?;
                self.phy_audit_read(through)?.changes_since(previous, after)
            }

            pub fn phy_audit_evidence(
                &self,
                cut: PhyAuditCut,
                id: MemoryId,
            ) -> Result<Option<(MemoryRecord, MemoryBody)>, OwnerSessionError> {
                let read = self.phy_audit_read(cut)?;
                let Some(record) = read.current(id) else {
                    return Ok(None);
                };
                let (_, body) = self
                    .common
                    .memories
                    .revision_projection(id, Some(record.revision))?;
                Ok(body.map(|body| (record.clone(), body.into_owned())))
            }

            pub fn phy_audit_lexical_candidates(
                &self,
                cut: PhyAuditCut,
                query: &str,
                limit: usize,
            ) -> Result<Vec<MemoryId>, OwnerSessionError> {
                let read = self.phy_audit_read(cut)?;
                Ok(self.common.dream_read_index.lexical_candidates(
                    query,
                    limit.min(MAX_PAGE),
                    |id| read.current(id).is_some(),
                ))
            }

            pub fn phy_audit_graph_links(
                &self,
                cut: PhyAuditCut,
                source: MemoryId,
            ) -> Result<Vec<(MemoryId, Relation)>, OwnerSessionError> {
                let read = self.phy_audit_read(cut)?;
                if read.current(source).is_none() {
                    return Err(OwnerSessionError::ManagementStale);
                }
                Ok(super::perception_audit::memory_graph_links(
                    &self.common.graph,
                    cut.graph,
                    source,
                    |_, other| read.current(other).is_some(),
                ))
            }

            fn phy_audit_read(
                &self,
                cut: PhyAuditCut,
            ) -> Result<PhyAuditRead<'_>, OwnerSessionError> {
                if !cut.is_at_or_before(self.phy_audit_cut()) {
                    return Err(OwnerSessionError::ManagementStale);
                }
                Ok(PhyAuditRead {
                    common: &self.common,
                    cut,
                })
            }
        }
    };
}

phy_audit_reads!(PhySession);
phy_audit_reads!(PhyWriter);

struct PhyAuditRead<'a> {
    common: &'a CommonState,
    cut: PhyAuditCut,
}

impl PhyAuditRead<'_> {
    fn current(&self, id: MemoryId) -> Option<&MemoryRecord> {
        let captured = self.at_cut(self.cut, id)?;
        let head = self.common.memories.revision_record(id, None).ok()?;
        (captured.revision == head.revision
            && self.common.memories.contains_body_payload(head.body_id))
        .then_some(captured)
    }

    fn at_cut(&self, cut: PhyAuditCut, id: MemoryId) -> Option<&MemoryRecord> {
        let record = self.common.memories.record_at_cut(id, cut.memory)?;
        let settled = self
            .common
            .dreams
            .settlement_position(id)
            .is_some_and(|position| position <= cut.settlements)
            || self
                .common
                .memories
                .has_retained_phy_import_authority_at(id, cut.memory);
        (settled
            && !record.archived
            && record.superseded_by.is_none()
            && matches!(record.lifecycle_state.as_str(), "knowledge" | "canonical"))
        .then_some(record)
    }

    fn changes_since(
        &self,
        previous: PhyAuditCut,
        after: PhyAuditCursor,
    ) -> Result<PhyAuditChangesPage, OwnerSessionError> {
        if after.memory < previous.memory
            || after.memory > self.cut.memory
            || after.settlements < previous.settlements
            || after.settlements > self.cut.settlements
            || previous.graph > self.cut.graph
        {
            return Err(OwnerSessionError::ManagementStale);
        }
        let memory = self.common.memories.changes_after(
            after.memory,
            self.cut.memory,
            MAX_CHANGES_PER_STREAM,
        );
        let settlements = self.common.dreams.settlements_after(
            after.settlements,
            self.cut.settlements,
            MAX_CHANGES_PER_STREAM,
        );
        let mut next = after;
        let mut sources = BTreeSet::new();
        for (position, id) in memory {
            next.memory = position;
            sources.insert(id);
        }
        for (position, id) in settlements {
            next.settlements = position;
            sources.insert(id);
        }
        let changes = sources
            .into_iter()
            .filter_map(|memory| {
                let previous_revision = self.at_cut(previous, memory).map(|r| r.revision);
                let current_revision = self.at_cut(self.cut, memory).map(|r| r.revision);
                (previous_revision != current_revision).then_some(PhyAuditChange {
                    memory,
                    previous_revision,
                    current_revision,
                })
            })
            .collect();
        Ok(PhyAuditChangesPage { changes, next })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_phy_birth_reopens_and_changes_count_once_per_memory() {
        let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        std::fs::create_dir_all(&parent).unwrap();
        let temp = tempfile::tempdir_in(parent).unwrap();
        let mut source = PhyWriter::create_path(temp.path().join("source.phy"), [4; 16]).unwrap();
        let path = temp.path().join("destination.phy");
        let mut phy = PhyWriter::create_path(&path, [7; 16]).unwrap();
        let before = phy.phy_audit_cut();

        let draft = MemoryDraft {
            title: "Reading order".into(),
            content: "Prefer the short version first.".into(),
            category: "fact".into(),
            memory_type: "project".into(),
            authority_kind: "direct".into(),
            temporal_status: "current".into(),
            scope: "user".into(),
            lifecycle_state: "knowledge".into(),
            archived: false,
            parent_id: None,
            superseded_by: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_ref: None,
            source_time_ns: Some(3),
            temporal_inference: None,
            mutation_id: "accepted-phy-fixture".into(),
            created_at_ns: 3,
            updated_at_ns: 3,
        };

        let accepted = source
            .common
            .memories
            .publish(&mut source.container, None, 0, draft.clone())
            .unwrap()
            .revision;
        let id = accepted.record.id;
        assert!(
            source
                .phy_audit_evidence(source.phy_audit_cut(), id)
                .unwrap()
                .is_none()
        );
        let unsettled = source.phy_audit_cut();
        let mut retained_delta = accepted.clone();
        retained_delta.record.revision = 2;
        retained_delta.record.memory_version = 2;
        retained_delta.record.updated_at_ns = 4;
        retained_delta.record.mutation_id = "accepted-phy-correction".into();
        source.writer_lease_proof = source.container.writer_lease_proof();
        source
            .import_accepted_memory_revision(1, 4, &retained_delta)
            .unwrap();
        let page = source
            .phy_audit_changes_page(unsettled, source.phy_audit_cut(), unsettled.into())
            .unwrap();
        assert_eq!(
            page.changes,
            [PhyAuditChange {
                memory: id,
                previous_revision: None,
                current_revision: Some(2),
            }]
        );

        phy.import_accepted_memory_revision(1, 3, &accepted)
            .unwrap();
        let initial = phy.phy_audit_cut();
        assert_eq!(
            phy.phy_audit_initial_page(initial, None).unwrap().sources,
            [id]
        );
        assert_eq!(
            phy.phy_audit_evidence(initial, id)
                .unwrap()
                .unwrap()
                .0
                .source_time_ns,
            Some(3)
        );

        let revision = |phy: &mut PhyWriter, number, archived| {
            let mut draft = draft.clone();
            draft.archived = archived;
            draft.updated_at_ns += number as i64;
            draft.mutation_id = format!("fixture-revision-{number}");
            let mut record = accepted.record.clone();
            record.revision = number;
            record.memory_version = number;
            record.archived = archived;
            record.updated_at_ns = draft.updated_at_ns;
            record.mutation_id = draft.mutation_id;
            phy.import_accepted_memory_revision(
                number,
                record.updated_at_ns,
                &MemoryRevision {
                    record,
                    payload: accepted.payload.clone(),
                },
            )
            .unwrap();
        };
        revision(&mut phy, 2, false);
        revision(&mut phy, 3, false);
        let revised = phy.phy_audit_cut();
        let page = phy
            .phy_audit_changes_page(initial, revised, initial.into())
            .unwrap();
        assert_eq!(
            page.changes,
            [PhyAuditChange {
                memory: id,
                previous_revision: Some(1),
                current_revision: Some(3)
            }]
        );
        assert_eq!(page.next, revised.into());
        drop(phy);

        let mut phy = PhyWriter::open_path_current(&path).unwrap();
        assert_eq!(phy.phy_audit_cut(), revised);
        let imported = phy
            .phy_audit_changes_page(before, revised, before.into())
            .unwrap();
        assert_eq!(
            imported.changes,
            [PhyAuditChange {
                memory: id,
                previous_revision: None,
                current_revision: Some(3)
            }]
        );
        let record = phy.phy_audit_evidence(revised, id).unwrap().unwrap().0;
        assert_eq!(record.revision, 3);

        revision(&mut phy, 4, true);
        let withdrawn = phy.phy_audit_cut();
        let page = phy
            .phy_audit_changes_page(revised, withdrawn, revised.into())
            .unwrap();
        assert_eq!(
            page.changes,
            [PhyAuditChange {
                memory: id,
                previous_revision: Some(3),
                current_revision: None
            }]
        );
        assert!(phy.phy_audit_evidence(revised, id).unwrap().is_none());
        assert!(matches!(
            phy.phy_audit_initial_page(
                PhyAuditCut {
                    history_generation: [9; 32],
                    ..revised
                },
                None
            ),
            Err(OwnerSessionError::ManagementStale),
        ));
    }
}
