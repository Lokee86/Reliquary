//! User-authorized corrections and terminal retirement through one REL writer.
use super::*;
use crate::records::user_memory::{MemoryEvidence, UserMemoryEffect, UserMemoryRecord};
use reliquary_memory_core::{MemorySourceRef, memory_id};

pub struct UserMemoryCorrection {
    pub id: MemoryId,
    pub revision: u64,
    pub body: MemoryBodyId,
    pub title: String,
    pub content: String,
    pub created_at_ns: i64,
}

pub struct UserMemoryCorrectionResult {
    pub archived_revision: u64,
    pub replacement: MemoryRevision,
    pub created: bool,
}

impl RelWriter {
    pub fn has_user_memory_request(
        &self,
        principal: &str,
        key: &str,
        hash: [u8; 32],
    ) -> Result<bool, OwnerSessionError> {
        self.user_memory_request(principal, key, hash)
            .map(|record| record.is_some())
    }

    fn user_memory_request(
        &self,
        principal: &str,
        key: &str,
        hash: [u8; 32],
    ) -> Result<Option<&UserMemoryRecord>, OwnerSessionError> {
        match self.session.common.user_memory_requests.get(key) {
            Some(record) if record.principal == principal && record.request_hash == hash => {
                Ok(Some(record))
            }
            Some(_) => Err(OwnerSessionError::ManagementConflict),
            None => Ok(None),
        }
    }

    pub fn correct_user_memory(
        &mut self,
        principal: &str,
        key: String,
        request_hash: [u8; 32],
        request: UserMemoryCorrection,
    ) -> Result<UserMemoryCorrectionResult, OwnerSessionError> {
        if let Some(record) = self.user_memory_request(principal, &key, request_hash)? {
            if !matches!(record.effect, UserMemoryEffect::Correction { .. }) {
                return Err(OwnerSessionError::ManagementConflict);
            }
            return self.correction_result(record, false);
        }
        self.check_user_memory_writer()?;
        let previous = MemoryEvidence::new(request.id, request.revision, request.body);
        let current = self.session.common.memories.current(request.id)?;
        let source = current.record;
        if !previous.matches(&source)
            || source.archived
            || source.superseded_by.is_some()
            || !matches!(current.payload, PayloadState::Available(_))
            || request.created_at_ns < source.updated_at_ns
        {
            return Err(OwnerSessionError::ManagementStale);
        }
        if reliquary_memory_core::memory_body_id(&request.title, &request.content) == source.body_id
        {
            return Err(OwnerSessionError::ManagementEvidence);
        }

        let owner = self.owner_id();
        let source_node = format!("management:{key}");
        let draft = MemoryDraft {
            title: request.title,
            content: request.content,
            category: source.category,
            memory_type: source.memory_type,
            authority_kind: "direct".into(),
            temporal_status: "current".into(),
            scope: source.scope,
            lifecycle_state: "knowledge".into(),
            archived: false,
            parent_id: None,
            superseded_by: None,
            source_node_id: Some(source_node.clone()),
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_ref: Some(MemorySourceRef {
                owner_id: owner.iter().map(|byte| format!("{byte:02x}")).collect(),
                principal_id: Some(principal.into()),
                source_episode_id: [0; 32],
                source_node_id: source_node,
                content_source_conversation_id: None,
                content_source_node_id: None,
                grounding_source_conversation_id: None,
                grounding_source_node_id: None,
            }),
            source_time_ns: Some(request.created_at_ns),
            temporal_inference: None,
            mutation_id: key.clone(),
            created_at_ns: request.created_at_ns,
            updated_at_ns: request.created_at_ns,
        };
        let common = &self.session.common;
        let prepared =
            common
                .memories
                .prepare_management_correction(request.id, request.revision, draft)?;
        let record = UserMemoryRecord {
            owner,
            principal: principal.into(),
            key,
            request_hash,
            memory_cut: common.memories.memory_version(),
            previous,
            effect: UserMemoryEffect::Correction {
                graph_cut: common.graph.graph_version(),
                creation: prepared.creation,
                delta: prepared.supersession.delta,
            },
        };
        self.publish_user_memory(&record, prepared.body.as_deref())?;
        self.correction_result(&record, true)
    }

    pub fn retire_user_memory(
        &mut self,
        principal: &str,
        key: String,
        request_hash: [u8; 32],
        request: MemoryRetirementRequest,
    ) -> Result<(TerminalTombstone, bool), OwnerSessionError> {
        if let Some(record) = self.user_memory_request(principal, &key, request_hash)? {
            if !matches!(record.effect, UserMemoryEffect::Retirement { .. }) {
                return Err(OwnerSessionError::ManagementConflict);
            }
            return self.retirement_result(record).map(|result| (result, false));
        }
        self.check_user_memory_writer()?;
        let owner = self.owner_id();
        let common = &self.session.common;
        super::perception_lifecycle::validate_retirement(common, owner, &request)?;
        let tombstone = common
            .memories
            .prepare_management_retirement(request.memory_id, request.memory_revision)?;
        let record = UserMemoryRecord {
            owner,
            principal: principal.into(),
            key,
            request_hash,
            memory_cut: common.memories.memory_version(),
            previous: MemoryEvidence::new(
                request.memory_id,
                request.memory_revision,
                request.body_id,
            ),
            effect: UserMemoryEffect::Retirement {
                successor: MemoryEvidence::new(
                    request.superseded_by,
                    request.successor_revision,
                    request.successor_body_id,
                ),
                tombstone,
            },
        };
        self.publish_user_memory(&record, None)?;
        self.retirement_result(&record).map(|result| (result, true))
    }

    fn check_user_memory_writer(&self) -> Result<(), OwnerSessionError> {
        self.ensure_write_ready()?;
        let lease = self.container.writer_lease_proof();
        if lease.is_none() || lease != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "user Memory writer requires reopen".into(),
            ));
        }
        Ok(())
    }

    fn publish_user_memory(
        &mut self,
        record: &UserMemoryRecord,
        body: Option<&[u8]>,
    ) -> Result<(), OwnerSessionError> {
        let bytes = record.encode().map_err(OwnerSessionError::Record)?;
        let effect = record.memory_effect().map_err(OwnerSessionError::Record)?;
        let relation = record.relation();
        let admission = GraphAdmission::for_changes(
            record.owner,
            [record.previous.id, memory_id(&record.key).0],
            [],
        );
        if let Some(edge) = relation {
            self.session
                .common
                .graph
                .validate_management_supersession(&admission, edge)?;
        }
        let publish = (|| -> Result<(), OwnerSessionError> {
            if let Some(body) = body {
                self.container.append(body)?;
            }
            // Only this frame publishes both revisions and the link, or the tombstone.
            self.container.append(&bytes)?;
            self.container.sync()?;
            let common = &mut self.session.common;
            common.memories.apply_management_group(body, effect)?;
            if let Some(edge) = relation {
                common
                    .graph
                    .apply_management_supersession(&admission, edge)?;
                common
                    .refresh_dream_index(&[MemoryId(record.previous.id), memory_id(&record.key)])?;
            } else {
                common.refresh_dream_index(&[MemoryId(record.previous.id)])?;
            }
            common
                .user_memory_requests
                .insert(record.key.clone(), record.clone());
            Ok(())
        })();
        if publish.is_err() {
            self.session.write_poisoned = true;
        } else {
            self.session.writer_lease_proof = self.container.writer_lease_proof();
        }
        publish
    }

    fn correction_result(
        &self,
        record: &UserMemoryRecord,
        created: bool,
    ) -> Result<UserMemoryCorrectionResult, OwnerSessionError> {
        let replacement = self
            .session
            .common
            .memories
            .revision(memory_id(&record.key), 1)?;
        let archived_revision = record
            .previous
            .revision
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        Ok(UserMemoryCorrectionResult {
            archived_revision,
            replacement,
            created,
        })
    }

    fn retirement_result(
        &self,
        record: &UserMemoryRecord,
    ) -> Result<TerminalTombstone, OwnerSessionError> {
        self.session
            .common
            .memories
            .terminal_at_cut(MemoryId(record.previous.id), u64::MAX)
            .ok_or(OwnerSessionError::ManagementMissing)
    }
}
