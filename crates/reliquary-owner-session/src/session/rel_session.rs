use super::*;

impl RelSession {
    /// Replay all currently composable REL domains. Graph closure, accepted
    /// Episode order, and Dream first-cycle population remain explicit inputs.
    pub fn open(
        container: &Container,
        graph_admission: &GraphAdmission,
        accepted_episodes: impl IntoIterator<Item = AcceptedEpisodeOrder>,
        dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<Self, OwnerSessionError> {
        require_kind(container, FileKind::Reliquary)?;
        let archive =
            Archive::open(container).map_err(|e| OwnerSessionError::Record(e.to_string()))?;
        let echo = reliquary_archive::EchoStore::open(container)
            .map_err(|e| OwnerSessionError::Record(e.to_string()))?;
        let accepted_episodes = accepted_episodes.into_iter().collect::<Vec<_>>();
        for order in &accepted_episodes {
            let episode = archive
                .episode(reliquary_archive::EpisodeId(order.episode_id.0))
                .ok_or(OwnerSessionError::Insomnia(
                    InsomniaError::UnacceptedEpisode,
                ))?;
            if order.source_through_ns != episode.source_through_ns
                || order.conversation_id != episode.conversation_id
                || order.start_node_id != episode.start_node_id
            {
                return Err(OwnerSessionError::Insomnia(
                    InsomniaError::UnacceptedEpisode,
                ));
            }
        }
        // Archive is the durable authority for accepted Episodes. Rebuild
        // the scheduling admission from it so reopen does not depend on a
        // caller retaining a separate list of work candidates.
        let accepted_episodes = archive
            .episodes()
            .into_iter()
            .map(|episode| AcceptedEpisodeOrder {
                episode_id: WorkEpisodeId(episode.id.0),
                source_through_ns: episode.source_through_ns,
                conversation_id: episode.conversation_id,
                start_node_id: episode.start_node_id,
            })
            .collect::<Vec<_>>();
        let grouped = validate_grouped_replay_admission(container, &archive)?;
        let grouped_completions = grouped.completions;
        let local_owner_id: String = container
            .owner_id()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if grouped_completions.values().any(|completion| {
            completion
                .external_memory_refs
                .iter()
                .any(|reference| reference.owner_id == local_owner_id)
        }) {
            return Err(OwnerSessionError::Record(
                "external Memory reference points to the local REL owner".into(),
            ));
        }
        let common = open_common(container, graph_admission, dream_admissions, true)?;
        for completion in grouped_completions.values() {
            if completion
                .memory_ids
                .iter()
                .any(|id| !common.memories.contains_memory(MemoryId(*id)))
            {
                return Err(OwnerSessionError::Record(
                    "grouped completion references an absent owner Memory".into(),
                ));
            }
        }
        let freshness = FreshnessStore::open_with_grouped_dream(
            container,
            |payload| {
                let completion = GroupedInsomniaCompletion::decode(payload)?;
                if !completion.routing_metadata.is_empty() {
                    return Err("grouped routing metadata replay is unsupported".into());
                }
                match (
                    completion.freshness_birth_turn,
                    completion.freshness_policy_version,
                ) {
                    (Some(turn), Some(policy)) => Ok(Some((
                        turn,
                        policy,
                        completion
                            .freshness_birth_memory_ids
                            .into_iter()
                            .map(FreshnessMemoryId)
                            .collect(),
                    ))),
                    (None, None) => Ok(None),
                    _ => Err("incomplete grouped Freshness proof".into()),
                }
            },
            |payload, freshness| {
                let record = crate::records::dream_empty::InitialDreamSettlement::decode(payload)?
                    .ok_or("missing Dream settlement")?;
                if record.owner != container.owner_id() {
                    return Err("Dream Freshness owner mismatch".into());
                }
                let begin = common
                    .dreams
                    .begin(&record.pass_id)
                    .ok_or("Dream Freshness lacks Begin")?;
                let effects = freshness.validate_initial_dream_lifecycle(
                    FreshnessMemoryId(record.source.0),
                    begin.turn,
                    record.accepted_turn,
                    record.graph_version,
                    record.community_generation,
                    &record.freshness_effects,
                    record.admit,
                )?;
                if record.credit_roots.is_empty() != effects.is_empty()
                    || effects.contains_key(&FreshnessMemoryId(record.source.0))
                    || record.credit_roots.iter().any(|id| {
                        effects.get(&FreshnessMemoryId(id.0))
                            != Some(&freshness.policy().linkage_principal)
                    })
                {
                    return Err("Dream initial roots differ from Freshness evidence".into());
                }
                Ok(record.freshness_effects)
            },
        )
        .map_err(OwnerSessionError::Record)?;
        let insomnia =
            InsomniaStore::open_with_grouped_completion(container, accepted_episodes, |payload| {
                let completion = GroupedInsomniaCompletion::decode(payload)
                    .map_err(|_| InsomniaError::CorruptRecord("grouped completion"))?;
                let episode_id = WorkEpisodeId(completion.episode_id);
                completion
                    .validate_replay_episode(episode_id.0)
                    .map_err(|_| InsomniaError::CorruptRecord("grouped completion proof"))?;
                Ok(Some(AcceptedCompletionState {
                    episode_id,
                    attempt: completion.attempt,
                    started_at_ns: completion.started_at_ns,
                    completed_at_ns: completion.completed_at_ns,
                    extractor_model: completion.extractor_model,
                    extractor_version: completion.extractor_version,
                    memory_ids: completion
                        .memory_ids
                        .into_iter()
                        .map(WorkMemoryId)
                        .collect(),
                    external_memory_refs: completion
                        .external_memory_refs
                        .into_iter()
                        .map(|reference| reliquary_insomnia_store::ExternalMemoryRef {
                            owner_id: reference.owner_id,
                            memory_id: WorkMemoryId(reference.memory_id),
                        })
                        .collect(),
                    rejected_count: completion.rejected_count,
                }))
            })?;
        let imports = ImportEpochStore::open(container).map_err(OwnerSessionError::Record)?;
        super::native_import::validate_native_import_receipts(container, &imports)?;
        let archive_vectors = ArchiveVectorStore::open(container, &common.packed_vectors)?;
        let generations =
            VectorGenerationStore::open(container, &common.profiles, &archive_vectors)?;
        super::archive_prune::validate_archive_prune(&archive, &common.maintenance)?;
        super::retired_profile_prune::validate_archive_profile_prune(
            &common.profiles,
            &common.packed_vectors,
            &common.memory_vectors,
            &archive_vectors,
            &generations,
            &common.maintenance,
        )?;
        super::archive_vector_publication::validate_generation_sources(
            &archive,
            &common,
            &archive_vectors,
            &generations,
        )?;
        let external_index =
            ExternalIndexStore::open(container).map_err(OwnerSessionError::Record)?;
        let external_references =
            ExternalReferenceStore::open(container).map_err(OwnerSessionError::Record)?;
        let external_memories = ExternalMemoryStore::open(container, &common.memories)
            .map_err(OwnerSessionError::Record)?;
        let container_path = canonical_container_path(container)?;
        let history_generation = owner_history_generation_from_container(container)
            .map_err(OwnerSessionError::Record)?;
        let container_file_identity = container.physical_file_identity()?;
        let writer_lease_proof = container.writer_lease_proof();
        Ok(Self {
            owner_id: container.owner_id(),
            archive,
            echo,
            common,
            freshness,
            insomnia,
            imports,
            archive_vectors,
            generations,
            external_index,
            external_references,
            external_memories,
            grouped_completions,
            native_import_grouped_epochs: grouped.import_epochs,
            write_poisoned: false,
            container_identity: container.identity(),
            container_path,
            history_generation,
            container_file_identity,
            writer_lease_proof,
            #[cfg(test)]
            fail_after_grouped_sync: false,
        })
    }

    pub fn open_path(
        path: impl AsRef<Path>,
        graph_admission: &GraphAdmission,
        accepted_episodes: impl IntoIterator<Item = AcceptedEpisodeOrder>,
        dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<Self, OwnerSessionError> {
        let container = Container::open_read_only_as(path, FileKind::Reliquary)?;
        Self::open(
            &container,
            graph_admission,
            accepted_episodes,
            dream_admissions,
        )
    }

    /// Open and fully admit one current-format REL from durable owner evidence.
    ///
    /// Graph closure, accepted Episodes, and current Dream Begin populations are
    /// reconstructed inside owner-session. Callers cannot weaken replay by
    /// supplying an empty or partial admission set.
    pub fn open_path_current(path: impl AsRef<Path>) -> Result<Self, OwnerSessionError> {
        let container = Container::open_read_only_as(path, FileKind::Reliquary)?;
        Self::open_current(&container)
    }

    pub(super) fn open_current(container: &Container) -> Result<Self, OwnerSessionError> {
        require_kind(container, FileKind::Reliquary)?;
        // This value conveys only the already-opened Container owner identity.
        // `open_common` reconstructs the actual Graph closure and Dream
        // populations from the durable semantic owners before admission.
        let owner_identity = GraphAdmission::for_changes(container.owner_id(), [], []);
        Self::open(container, &owner_identity, [], [])
    }

    pub fn owner_id(&self) -> [u8; 16] {
        self.owner_id
    }

    pub fn scope(&self) -> OwnerScope {
        self.container_identity.scope.into()
    }

    pub fn archive_node(&self, conversation: &str, node: &str) -> Option<&Node> {
        self.archive.node(conversation, node)
    }
    pub fn archive_branch(
        &self,
        conversation: &str,
        branch: &str,
    ) -> Option<&reliquary_archive::Branch> {
        self.archive.branch(conversation, branch)
    }
    pub fn archive_version(&self) -> u64 {
        self.archive.archive_version()
    }
    pub fn conversation_tombstone(
        &self,
        conversation: &str,
    ) -> Option<&reliquary_archive::ConversationTombstone> {
        self.archive.conversation_tombstone(conversation)
    }
    /// Read immutable Echo evidence for one exact conversation/message source.
    pub fn echo_for_turn(
        &self,
        conversation: &str,
        message: &str,
    ) -> Vec<reliquary_archive::EchoEvent> {
        self.echo.for_turn(conversation, message)
    }
    /// Read immutable Echo evidence for one conversation in deterministic order.
    pub fn echo_for_conversation(&self, conversation: &str) -> Vec<reliquary_archive::EchoEvent> {
        self.echo.for_conversation(conversation)
    }
    pub fn archive_source_attachments(
        &self,
        conversation: &str,
        node: &str,
    ) -> Vec<reliquary_archive::SourceAttachment> {
        self.archive.files_for_source(conversation, node)
    }
    pub fn episode(&self, id: reliquary_archive::EpisodeId) -> Option<&reliquary_archive::Episode> {
        self.archive.episode(id)
    }
    pub fn rel_turn_count(&self) -> u64 {
        self.archive.rel_turn_count()
    }
    pub fn accepted_turn_sources(&self) -> Vec<reliquary_archive::AcceptedTurnSource> {
        self.archive.accepted_turn_sources()
    }
    pub fn freshness_record(
        &self,
        id: FreshnessMemoryId,
    ) -> Result<Option<FreshnessRecord>, OwnerSessionError> {
        self.freshness
            .record_at(id, u64::MAX)
            .map_err(OwnerSessionError::Record)
    }
    pub fn freshness_record_at(
        &self,
        id: FreshnessMemoryId,
        turn: u64,
    ) -> Result<Option<FreshnessRecord>, OwnerSessionError> {
        self.freshness
            .record_at(id, turn)
            .map_err(OwnerSessionError::Record)
    }
    pub fn freshness_record_through_turn(
        &self,
        id: FreshnessMemoryId,
        turn: u64,
    ) -> Result<Option<FreshnessRecord>, OwnerSessionError> {
        Ok(self
            .freshness
            .records_through_turn(turn)
            .map_err(OwnerSessionError::Record)?
            .get(&id)
            .copied())
    }
    pub fn freshness_policy(&self) -> reliquary_freshness_store::FreshnessPolicy {
        self.freshness.policy()
    }
    pub fn insomnia_work(&self, id: WorkEpisodeId) -> Option<&InsomniaWork> {
        self.insomnia.work(id)
    }
    pub fn insomnia_receipt(
        &self,
        id: WorkEpisodeId,
        attempt: u32,
    ) -> Option<&AcceptedCompletionState> {
        self.insomnia.receipt(id, attempt)
    }

    /// Commit one authenticated, same-owner grouped Insomnia completion.
    /// Candidate extraction and external-owner routing remain workflow-owned.

    pub fn import_status(&self, import_id: &str) -> Option<ImportEpochStatus> {
        self.imports.status(import_id)
    }
    pub fn archive_vector_infos(&self) -> Vec<ArchiveVectorInfo> {
        self.archive_vectors.infos()
    }
    pub fn vector_generation(&self, id: VectorGenerationId) -> Option<VectorGeneration> {
        self.generations.generation(id)
    }
    pub fn external_source_index(&self, namespace: &str) -> Option<&SourceState> {
        self.external_index.sources.get(namespace)
    }

    /// Read the latest reference-only host manifest admitted for this exact
    /// namespace and authorizing principal.
    pub fn external_reference_snapshot(
        &self,
        namespace: &str,
        principal: &str,
    ) -> Option<&ExternalReferenceSnapshotReceipt> {
        self.external_references.get(namespace, principal)
    }

    #[cfg(test)]
    pub(crate) fn inject_grouped_projection_failure(&mut self) {
        self.fail_after_grouped_sync = true;
    }
}

impl RelSession {
    /// Rehydrate the immutable turn range from the accepted Archive Episode.
    pub fn episode_turns(
        &self,
        id: reliquary_archive::EpisodeId,
    ) -> Result<Option<Vec<reliquary_archive::ResolvedTurn>>, OwnerSessionError> {
        self.archive
            .episode_turns(id)
            .map_err(|error| OwnerSessionError::Record(error.to_string()))
    }

    /// Rehydrate one accepted source turn from this owner's Archive.
    pub fn resolved_archive_turn(
        &self,
        conversation_id: &str,
        node_id: &str,
    ) -> Result<Option<reliquary_archive::ResolvedTurn>, OwnerSessionError> {
        self.archive
            .resolved_turn(conversation_id, node_id)
            .map_err(|error| OwnerSessionError::Record(error.to_string()))
    }

    pub fn prove_routed_user_memory(
        &self,
        destination: &PhyWriter,
        draft: &MemoryDraft,
        source_quote: &str,
        authority_quote: Option<&str>,
    ) -> Result<RoutedUserMemoryProof, OwnerSessionError> {
        // This constructor trusts Application to supply a semantically
        // validated candidate. It independently checks the owner-local
        // Archive identity, principal, quote and PHY privacy boundary.
        let source = draft.source_ref.as_ref().ok_or_else(|| {
            OwnerSessionError::Record("routed Memory lacks owner-qualified source proof".into())
        })?;
        let episode_id = reliquary_archive::EpisodeId(source.source_episode_id);
        let episode = self.archive.episode(episode_id).ok_or_else(|| {
            OwnerSessionError::Record("routed Memory source Episode is unavailable".into())
        })?;
        let source_turn = self
            .archive_evidence_turn(&episode.conversation_id, &source.source_node_id)?
            .ok_or_else(|| {
                OwnerSessionError::Record("routed Memory source turn is unavailable".into())
            })?;
        let episode_turns = self.archive_evidence_episode(episode_id)?.ok_or_else(|| {
            OwnerSessionError::Record("routed Memory source Episode is unavailable".into())
        })?;
        let in_episode = episode_turns
            .iter()
            .any(|turn| turn.node_id == source_turn.node_id);
        let expected_source_time_ns = if let Some(quote) = authority_quote {
            if !matches!(draft.authority_kind.as_str(), "adoption" | "retention")
                || quote.trim().is_empty()
            {
                return Err(OwnerSessionError::Record(
                    "invalid assistant authority quote".into(),
                ));
            }
            let (Some(conversation_id), Some(node_id)) = (
                source.content_source_conversation_id.as_deref(),
                source.content_source_node_id.as_deref(),
            ) else {
                return Err(OwnerSessionError::Record(
                    "adoption Memory lacks its assistant authority source".into(),
                ));
            };
            let authority_turn = self
                .archive_evidence_turn(conversation_id, node_id)?
                .ok_or_else(|| {
                    OwnerSessionError::Record(
                        "adoption assistant authority turn is unavailable".into(),
                    )
                })?;
            if authority_turn.role != "assistant"
                || authority_turn.timestamp_ns > source_turn.timestamp_ns
                || !authority_turn.content.contains(quote)
            {
                return Err(OwnerSessionError::Record(
                    "adoption authority source is not an earlier accepted assistant turn".into(),
                ));
            }
            authority_turn.timestamp_ns
        } else {
            if draft.authority_kind == "adoption"
                || source.content_source_conversation_id.is_some()
                || source.content_source_node_id.is_some()
            {
                return Err(OwnerSessionError::Record(
                    "routed Memory lacks its assistant authority quote".into(),
                ));
            }
            source_turn.timestamp_ns
        };
        let owner_hex: String = self
            .owner_id
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let destination_owner_id = destination.owner_id;
        let privacy_valid = draft.scope == "user"
            && draft.source_node_id.is_none()
            && draft.content_source_conversation_id.is_none()
            && draft.content_source_node_id.is_none()
            && draft.grounding_source_conversation_id.is_none()
            && draft.grounding_source_node_id.is_none()
            && draft.source_episode_id.is_none();
        if destination_owner_id == self.owner_id
            || source.owner_id != owner_hex
            || source.principal_id.as_deref().is_none_or(str::is_empty)
            || source.principal_id != source_turn.principal_id
            || source_turn.role != "user"
            || source_turn.principal_id.is_none()
            || source.source_node_id != source_turn.node_id
            || draft.source_time_ns != Some(expected_source_time_ns)
            || source_quote.trim().is_empty()
            || !source_turn.content.contains(source_quote)
            || !in_episode
            || !privacy_valid
            || draft.mutation_id.trim().is_empty()
        {
            return Err(OwnerSessionError::Record(
                "routed Memory does not match authenticated user Archive evidence".into(),
            ));
        }
        Ok(RoutedUserMemoryProof {
            source_owner_id: self.owner_id,
            destination_owner_id,
            source_episode_id: episode_id.0,
            draft: draft.clone(),
        })
    }
}
