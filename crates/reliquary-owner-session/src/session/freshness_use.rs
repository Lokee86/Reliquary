use super::*;

impl RelSession {
    /// Stable accepted-use receipt lookup for retry suppression.
    pub fn accepted_external_use(&self, use_id: &str) -> Option<&FreshnessUseReceipt> {
        self.freshness.accepted_use(use_id)
    }

    /// Return an immutable non-reinforcing retrieval snapshot. No read path
    /// writes a Freshness event.
    pub fn retrieval_snapshot(
        &self,
        profile: Option<CompatibilityProfileId>,
    ) -> Result<RelRetrievalSnapshot, OwnerSessionError> {
        if let Some(id) = profile {
            if self.common.profiles.get_active(id).is_none() {
                return Err(OwnerSessionError::Record(
                    "embedding profile is unavailable".into(),
                ));
            }
        }
        let rel_turn = self.archive.rel_turn_count();
        let memory_version = self.common.memories.memory_version();
        let mut memories = Vec::new();
        let mut vectors = BTreeMap::new();
        for id in self.common.memories.memory_ids() {
            if self
                .common
                .memories
                .record_at_cut(id, memory_version)
                .is_none()
            {
                continue;
            }
            let revision = self.common.memories.current(id)?;
            if revision.record.archived
                || matches!(
                    revision.payload,
                    reliquary_memory_core::PayloadState::Absent
                )
            {
                continue;
            }
            if let Some(profile_id) = profile {
                if let Some(row) = self.memory_vector_row(profile_id, revision.record.body_id)? {
                    vectors.insert(revision.record.body_id, row);
                }
            }
            let freshness = self
                .freshness
                .record_at(FreshnessMemoryId(id.0), rel_turn)
                .map_err(OwnerSessionError::Record)?;
            memories.push(RelRetrievalMemory {
                revision,
                freshness,
            });
        }
        Ok(RelRetrievalSnapshot {
            owner_id: self.owner_id,
            rel_turn,
            memory_version,
            graph_version: self.common.graph.graph_version(),
            memories,
            active_relations: self
                .common
                .graph
                .history()
                .filter(|r| {
                    r.active && self.common.graph.relation(r.source, r.target, r.kind) == Some(*r)
                })
                .copied()
                .collect(),
            community: self.common.communities.current().cloned(),
            vectors,
        })
    }
}

impl RelWriter {
    /// Record authenticated external access after the trusted Gateway seam.
    /// The caller's accepted turn must be at or before the current REL cut;
    /// exact retries are idempotent in FreshnessStore.
    pub fn record_external_accepted_use(
        &mut self,
        use_id: &str,
        sources: Vec<FreshnessMemoryId>,
        accepted_turn: u64,
        proof: FreshnessEventProof,
    ) -> Result<bool, OwnerSessionError> {
        self.ensure_write_ready()?;
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "REL owner session is stale; reopen before Freshness writes".into(),
            ));
        }
        if let Some(existing) = self.session.freshness.accepted_use(use_id) {
            let mut normalized = sources.clone();
            normalized.sort();
            normalized.dedup();
            if existing.accepted_turn != accepted_turn || existing.sources != normalized {
                return Err(OwnerSessionError::Record(
                    "accepted-use identity collides with different receipt evidence".into(),
                ));
            }
            let result = self.session.freshness.accept_use(
                &mut self.container,
                use_id.to_owned(),
                accepted_turn,
                sources,
                proof,
            );
            return match result {
                Ok(changed) => {
                    self.session.writer_lease_proof = self.container.writer_lease_proof();
                    Ok(changed)
                }
                Err(error) => {
                    if before != self.container.writer_lease_proof() {
                        self.session.write_poisoned = true;
                    }
                    Err(OwnerSessionError::Record(error))
                }
            };
        }
        if accepted_turn > self.session.archive.rel_turn_count() {
            return Err(OwnerSessionError::Record(
                "accepted use turn is beyond current REL activity".into(),
            ));
        }
        let current_graph = self.session.common.graph.graph_version();
        if proof.validate().is_err() || proof.graph_version > current_graph {
            return Err(OwnerSessionError::Record(
                "accepted-use proof is not a valid historical graph cut".into(),
            ));
        }
        let community = self.session.common.communities.current();
        if proof.community_generation.is_some() != proof.community_graph_version.is_some()
            || proof.community_generation.is_some_and(|generation| {
                community.is_none_or(|snapshot| generation > snapshot.generation)
            })
            || proof
                .community_graph_version
                .is_some_and(|version| version > current_graph)
            || proof.community_graph_version.is_some_and(|version| {
                version > community.map_or(current_graph, |snapshot| snapshot.graph_version)
            })
        {
            return Err(OwnerSessionError::Record(
                "accepted-use proof is not a valid historical community cut".into(),
            ));
        }
        let result = self.session.freshness.accept_use(
            &mut self.container,
            use_id.to_owned(),
            accepted_turn,
            sources,
            proof,
        );
        match result {
            Ok(changed) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(changed)
            }
            Err(error) => {
                if before != self.container.writer_lease_proof() {
                    self.session.write_poisoned = true;
                }
                Err(OwnerSessionError::Record(error))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::ReliquaryScope;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
            fs::create_dir_all(&root).unwrap();
            Self(root.join(format!(
                "freshness-use-{}-{}.rel",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[test]
    fn retrieval_snapshot_is_nonreinforcing_and_accepted_use_is_durable_idempotent() {
        let path = Temp::new();
        let owner = [77; 16];
        let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();
        let draft = MemoryDraft {
            title: "snapshot title".into(),
            content: "snapshot body".into(),
            category: "fact".into(),
            memory_type: "semantic".into(),
            authority_kind: "user".into(),
            temporal_status: "current".into(),
            scope: "project".into(),
            lifecycle_state: "extracted".into(),
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
            source_time_ns: Some(1),
            temporal_inference: None,
            mutation_id: "freshness-snapshot-test".into(),
            created_at_ns: 1,
            updated_at_ns: 1,
        };
        let published = writer
            .session
            .common
            .memories
            .publish_versioned_creation(&mut writer.container, draft.clone(), 1)
            .unwrap();
        let source = FreshnessMemoryId(published.revision.record.id.0);
        let mut deleted = draft.clone();
        deleted.mutation_id = "freshness-tombstoned-test".into();
        let deleted = writer
            .session
            .common
            .memories
            .publish_versioned_creation(&mut writer.container, deleted, 2)
            .unwrap();
        writer
            .session
            .common
            .memories
            .tombstone(&mut writer.container, deleted.revision.record.id, 1)
            .unwrap();
        let mut archived = draft.clone();
        archived.archived = true;
        archived.mutation_id = "freshness-archived-test".into();
        writer
            .session
            .common
            .memories
            .publish_versioned_creation(&mut writer.container, archived, 3)
            .unwrap();
        writer
            .session
            .freshness
            .create_birth(&mut writer.container, source, 0, false)
            .unwrap();
        writer.session.writer_lease_proof = writer.container.writer_lease_proof();
        let proof = FreshnessEventProof {
            graph_version: writer.session.common.graph.graph_version(),
            community_generation: None,
            community_graph_version: None,
        };
        let before = writer.read_session().retrieval_snapshot(None).unwrap();
        assert_eq!(before.rel_turn, 0);
        assert_eq!(before.memories.len(), 1);
        assert_eq!(before.memories[0].freshness.unwrap().score, 100);
        assert!(
            writer
                .read_session()
                .accepted_external_use("host-use-1")
                .is_none()
        );
        let again = writer.read_session().retrieval_snapshot(None).unwrap();
        assert_eq!(before, again);
        assert!(
            writer
                .read_session()
                .accepted_external_use("host-use-1")
                .is_none()
        );
        let mut second = draft;
        second.mutation_id = "freshness-cut-advance-test".into();
        let second = writer
            .session
            .common
            .memories
            .publish_versioned_creation(&mut writer.container, second, 2)
            .unwrap();
        let admission = GraphAdmission::for_changes(
            owner,
            [published.revision.record.id.0, second.revision.record.id.0],
            [],
        );
        writer
            .session
            .common
            .graph
            .put(
                &mut writer.container,
                &admission,
                &[reliquary_graph::RelationChange {
                    source: NodeRef::Memory(published.revision.record.id.0),
                    target: NodeRef::Memory(second.revision.record.id.0),
                    kind: reliquary_graph::RelationKind::Topical,
                    active: true,
                    origin: reliquary_graph::RelationOrigin::User,
                }],
            )
            .unwrap();
        writer.session.writer_lease_proof = writer.container.writer_lease_proof();
        assert_eq!(
            writer.session.common.graph.graph_version(),
            proof.graph_version + 1
        );
        assert!(
            writer
                .record_external_accepted_use("host-use-1", vec![source], 0, proof)
                .unwrap()
        );
        assert!(
            !writer
                .record_external_accepted_use("host-use-1", vec![source], 0, proof)
                .unwrap()
        );
        assert!(
            writer
                .record_external_accepted_use("host-use-1", vec![source], 1, proof)
                .is_err()
        );
        let ids = writer
            .session
            .common
            .memories
            .memory_ids()
            .map(|id| id.0)
            .collect::<Vec<_>>();
        let admission = GraphAdmission::for_changes(owner, ids, []);
        writer
            .session
            .common
            .graph
            .put(
                &mut writer.container,
                &admission,
                &[reliquary_graph::RelationChange {
                    source: NodeRef::Memory(published.revision.record.id.0),
                    target: NodeRef::Memory(second.revision.record.id.0),
                    kind: reliquary_graph::RelationKind::Topical,
                    active: false,
                    origin: reliquary_graph::RelationOrigin::User,
                }],
            )
            .unwrap();
        writer.session.writer_lease_proof = writer.container.writer_lease_proof();
        assert!(
            writer
                .read_session()
                .retrieval_snapshot(None)
                .unwrap()
                .active_relations
                .is_empty()
        );
        assert_eq!(
            writer
                .read_session()
                .accepted_external_use("host-use-1")
                .unwrap()
                .accepted_turn,
            0
        );
        // Advance REL activity after the accepted use. Retrying its exact
        // immutable receipt remains idempotent at its original turn cut.
        let mut parent = None;
        for ix in 0..3 {
            let id = format!("later-{ix}");
            writer
                .session
                .archive
                .accept_turn(
                    &mut writer.container,
                    reliquary_archive::TurnInput {
                        id: id.clone(),
                        conversation_id: "later-conversation".into(),
                        parent_id: parent.clone(),
                        role: "user".into(),
                        principal_id: Some("alice".into()),
                        timestamp_ns: ix + 1,
                        content: "later activity".into(),
                        attachments: vec![],
                        project_attachments: vec![],
                    },
                )
                .unwrap();
            parent = Some(id);
        }
        writer.session.writer_lease_proof = writer.container.writer_lease_proof();
        assert!(
            writer
                .read_session()
                .retrieval_snapshot(None)
                .unwrap()
                .rel_turn
                > 0
        );
        assert!(
            !writer
                .record_external_accepted_use("host-use-1", vec![source], 0, proof)
                .unwrap()
        );
        drop(writer);
        let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
        let replay = RelSession::open_path(&path.0, &admission, [], []).unwrap();
        assert_eq!(
            replay.accepted_external_use("host-use-1").unwrap().sources,
            vec![source]
        );
        assert!(replay.retrieval_snapshot(None).unwrap().rel_turn > 0);
    }
}
