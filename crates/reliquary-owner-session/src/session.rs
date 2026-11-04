use crate::records::external_index::{ExternalIndexStore, SourceState};
use crate::records::{
    DreamBeginAdmission, DreamEvidenceStore, GroupedInsomniaCompletion, HistoryPruneManifest,
    ImportEpochStatus, ImportEpochStore, OwnerMaintenanceStore,
};
use reliquary_archive::{Archive, Node};
use reliquary_container::{
    Container, ContainerError, ContainerIdentity, FileKind, WriterLeaseProof,
};
use reliquary_ego_store::{
    EgoAnchor, EgoAnchorId, EgoError, EgoIdentity, EgoIdentityId, EgoPersonality, EgoStore,
};
use reliquary_freshness_store::{FreshnessMemoryId, FreshnessRecord, FreshnessStore};
use reliquary_graph::{
    CommunitySnapshot, CommunityStore, GraphAdmission, GraphError, GraphStore, NodeRef, Relation,
};
use reliquary_insomnia_store::{
    AcceptedCompletionState, AcceptedEpisodeOrder, CompletionCheck, EpisodeId as WorkEpisodeId,
    InsomniaError, InsomniaLeaseToken, InsomniaPriority, InsomniaStore, InsomniaWork,
    MemoryId as WorkMemoryId,
};
use reliquary_memory_core::{MemoryError, MemoryHistory, MemoryId, MemoryStore};
use reliquary_perception_store::{
    Entity, EntityId, MemoryEntityMentionKey, MemoryEntityResolution, PerceptionError,
    PerceptionStore,
};
use reliquary_vector_store::{
    ArchiveVectorError, ArchiveVectorInfo, ArchiveVectorStore, CompatibilityProfile,
    CompatibilityProfileId, CompatibilityProfileStore, GenerationStoreError, MemoryVectorError,
    MemoryVectorInfo, MemoryVectorStore, PackedVectorError, PackedVectorInfo, PackedVectorStore,
    ProfileStoreError, VectorGeneration, VectorGenerationId, VectorGenerationStore,
};
use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum OwnerSessionError {
    WrongFileKind {
        expected: FileKind,
        actual: FileKind,
    },
    RelOnlyRecordInPhylactery(&'static str),
    Container(ContainerError),
    Memory(MemoryError),
    Perception(PerceptionError),
    Graph(GraphError),
    Ego(EgoError),
    Insomnia(InsomniaError),
    PackedVector(PackedVectorError),
    Profile(ProfileStoreError),
    ArchiveVector(ArchiveVectorError),
    MemoryVector(MemoryVectorError),
    Generation(GenerationStoreError),
    Archive(reliquary_archive::ArchiveError),
    Record(String),
}

impl fmt::Display for OwnerSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFileKind { expected, actual } => {
                write!(
                    f,
                    "owner session kind mismatch: expected {expected:?}, found {actual:?}"
                )
            }
            Self::RelOnlyRecordInPhylactery(domain) => {
                write!(f, "REL-only {domain} record found in Phylactery")
            }
            Self::Container(e) => e.fmt(f),
            Self::Memory(e) => e.fmt(f),
            Self::Perception(e) => e.fmt(f),
            Self::Graph(e) => e.fmt(f),
            Self::Ego(e) => e.fmt(f),
            Self::Insomnia(e) => e.fmt(f),
            Self::PackedVector(e) => e.fmt(f),
            Self::Profile(e) => e.fmt(f),
            Self::ArchiveVector(e) => e.fmt(f),
            Self::MemoryVector(e) => e.fmt(f),
            Self::Generation(e) => e.fmt(f),
            Self::Archive(e) => e.fmt(f),
            Self::Record(e) => f.write_str(e),
        }
    }
}
impl std::error::Error for OwnerSessionError {}
impl From<reliquary_archive::ArchiveError> for OwnerSessionError {
    fn from(e: reliquary_archive::ArchiveError) -> Self {
        Self::Archive(e)
    }
}
impl From<ContainerError> for OwnerSessionError {
    fn from(e: ContainerError) -> Self {
        Self::Container(e)
    }
}
impl From<MemoryError> for OwnerSessionError {
    fn from(e: MemoryError) -> Self {
        Self::Memory(e)
    }
}
impl From<PerceptionError> for OwnerSessionError {
    fn from(e: PerceptionError) -> Self {
        Self::Perception(e)
    }
}
impl From<GraphError> for OwnerSessionError {
    fn from(e: GraphError) -> Self {
        Self::Graph(e)
    }
}
impl From<EgoError> for OwnerSessionError {
    fn from(e: EgoError) -> Self {
        Self::Ego(e)
    }
}
impl From<InsomniaError> for OwnerSessionError {
    fn from(e: InsomniaError) -> Self {
        Self::Insomnia(e)
    }
}
impl From<PackedVectorError> for OwnerSessionError {
    fn from(e: PackedVectorError) -> Self {
        Self::PackedVector(e)
    }
}
impl From<ProfileStoreError> for OwnerSessionError {
    fn from(e: ProfileStoreError) -> Self {
        Self::Profile(e)
    }
}
impl From<ArchiveVectorError> for OwnerSessionError {
    fn from(e: ArchiveVectorError) -> Self {
        Self::ArchiveVector(e)
    }
}
impl From<MemoryVectorError> for OwnerSessionError {
    fn from(e: MemoryVectorError) -> Self {
        Self::MemoryVector(e)
    }
}
impl From<GenerationStoreError> for OwnerSessionError {
    fn from(e: GenerationStoreError) -> Self {
        Self::Generation(e)
    }
}

/// Shared owner-local REL/PHY domains, kept private behind typed reads.
struct CommonState {
    memories: MemoryStore,
    perception: PerceptionStore,
    graph: GraphStore,
    communities: CommunityStore,
    ego: EgoStore,
    packed_vectors: PackedVectorStore,
    profiles: CompatibilityProfileStore,
    memory_vectors: MemoryVectorStore,
    maintenance: OwnerMaintenanceStore,
    dreams: DreamEvidenceStore,
}

/// Read-only Reliquary owner aggregate. Archive and turn authority exist only
/// here; private fields prevent exposing a public bag of stores.
pub struct RelSession {
    owner_id: [u8; 16],
    archive: Archive,
    common: CommonState,
    freshness: FreshnessStore,
    insomnia: InsomniaStore,
    imports: ImportEpochStore,
    archive_vectors: ArchiveVectorStore,
    generations: VectorGenerationStore,
    external_index: ExternalIndexStore,
    grouped_completions: BTreeMap<(WorkEpisodeId, u32), GroupedInsomniaCompletion>,
    write_poisoned: bool,
    container_identity: ContainerIdentity,
    container_path: PathBuf,
    container_file_identity: (u64, u64),
    writer_lease_proof: Option<WriterLeaseProof>,
    #[cfg(test)]
    fail_after_grouped_sync: bool,
}

/// Owner-session-owned writer lease and mutable REL projection. The container
/// is deliberately private: callers can request the concrete grouped commit,
/// but cannot use it as a general mutable storage handle.
pub struct RelWriter {
    container: Container,
    session: RelSession,
}

impl RelWriter {
    pub fn open_path(
        path: impl AsRef<Path>,
        graph_admission: GraphAdmission,
        accepted_episodes: impl IntoIterator<Item = AcceptedEpisodeOrder>,
        dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<Self, OwnerSessionError> {
        let accepted_episodes = accepted_episodes.into_iter().collect::<Vec<_>>();
        let dream_admissions = dream_admissions.into_iter().collect::<Vec<_>>();
        let container = Container::open_writer_as(path, FileKind::Reliquary)?;
        let session = RelSession::open(
            &container,
            &graph_admission,
            accepted_episodes.clone(),
            dream_admissions.clone(),
        )?;
        Ok(Self { container, session })
    }

    pub fn owner_id(&self) -> [u8; 16] {
        self.session.owner_id()
    }
    pub fn read_session(&self) -> &RelSession {
        &self.session
    }

    pub fn next_global_version(&self) -> Result<u64, OwnerSessionError> {
        self.container
            .latest_version()
            .checked_add(1)
            .ok_or_else(|| OwnerSessionError::Record("physical version exhausted".into()))
    }

    /// Accept a live source turn and advance its branch through the owner’s
    /// Archive projection. Archive operations are idempotent, so a retry after
    /// a queue journal failure can safely confirm the already synced turn.
    pub fn accept_turn_and_advance_branch(
        &mut self,
        input: reliquary_archive::TurnInput,
        branch: reliquary_archive::Branch,
    ) -> Result<reliquary_archive::Node, OwnerSessionError> {
        self.validate_turn_branch(&input, &branch)?;
        let already_on_branch = if let Some(current) = self
            .session
            .archive
            .branch(&branch.conversation_id, &branch.id)
        {
            let ancestry = self
                .session
                .archive
                .historical_turns(&branch.conversation_id, &current.leaf_node_id)?;
            let found = ancestry.iter().any(|turn| turn.node_id == input.id);
            if found {
                self.validate_existing_turn(&input)?;
            }
            found
        } else {
            false
        };
        let accepted = self
            .session
            .archive
            .accept_turn(&mut self.container, input)?;
        if already_on_branch {
            return Ok(accepted.value);
        }
        self.session
            .archive
            .put_branch(&mut self.container, branch)?;
        Ok(accepted.value)
    }

    /// Check that an accepted turn would advance this owner's current branch
    /// before any Archive frame is appended. The caller must retain the same
    /// writer lock from validation through `accept_turn_and_advance_branch`.
    pub fn validate_turn_branch(
        &self,
        input: &reliquary_archive::TurnInput,
        branch: &reliquary_archive::Branch,
    ) -> Result<(), OwnerSessionError> {
        if input.conversation_id != branch.conversation_id || input.id != branch.leaf_node_id {
            return Err(OwnerSessionError::Archive(
                reliquary_archive::ArchiveError::InvalidBranchRevision,
            ));
        }
        let Some(current) = self
            .session
            .archive
            .branch(&branch.conversation_id, &branch.id)
        else {
            return Ok(());
        };
        if self
            .session
            .archive
            .node(&input.conversation_id, &input.id)
            .is_some()
        {
            self.validate_existing_turn(input)?;
        }
        let current_ancestry = self
            .session
            .archive
            .historical_turns(&branch.conversation_id, &current.leaf_node_id)?;
        if current_ancestry
            .iter()
            .any(|turn| turn.node_id == input.id)
        {
            return Ok(());
        }
        let Some(parent_id) = input.parent_id.as_deref() else {
            return Err(OwnerSessionError::Archive(
                reliquary_archive::ArchiveError::InvalidBranchRevision,
            ));
        };
        let ancestry = self
            .session
            .archive
            .historical_turns(&input.conversation_id, parent_id)?;
        if ancestry
            .iter()
            .any(|turn| turn.node_id == current.leaf_node_id)
        {
            Ok(())
        } else {
            Err(OwnerSessionError::Archive(
                reliquary_archive::ArchiveError::InvalidBranchRevision,
            ))
        }
    }

    fn validate_existing_turn(
        &self,
        input: &reliquary_archive::TurnInput,
    ) -> Result<(), OwnerSessionError> {
        let Some(node) = self
            .session
            .archive
            .node(&input.conversation_id, &input.id)
        else {
            return Ok(());
        };
        let content_matches = self
            .session
            .archive
            .content_for_node(&input.conversation_id, &input.id)
            == Some(input.content.as_str());
        if node.parent_id != input.parent_id
            || node.role != input.role
            || node.principal_id != input.principal_id
            || node.timestamp_ns != input.timestamp_ns
            || !content_matches
        {
            return Err(OwnerSessionError::Archive(
                reliquary_archive::ArchiveError::ConflictingNode,
            ));
        }
        Ok(())
    }

    /// Queue and lease work for an admitted Archive Episode. The Episode's
    /// scheduling identity is re-derived from this owner's Archive projection;
    /// caller-supplied order data cannot redirect the claim.
    pub fn queue_and_claim_insomnia_episode(
        &mut self,
        episode_id: reliquary_archive::EpisodeId,
        worker: &str,
        now_ns: i64,
        lease_duration_ns: i64,
    ) -> Result<InsomniaWork, OwnerSessionError> {
        let episode = self
            .session
            .archive
            .episode(episode_id)
            .ok_or(OwnerSessionError::Insomnia(InsomniaError::UnacceptedEpisode))?;
        let work_id = WorkEpisodeId(episode_id.0);
        let order = self
            .session
            .insomnia
            .accepted_order(work_id)
            .cloned()
            .ok_or(OwnerSessionError::Insomnia(InsomniaError::UnacceptedEpisode))?;
        if order.source_through_ns != episode.source_through_ns
            || order.conversation_id != episode.conversation_id
            || order.start_node_id != episode.start_node_id
        {
            return Err(OwnerSessionError::Insomnia(InsomniaError::UnacceptedEpisode));
        }
        self.session
            .insomnia
            .queue(&mut self.container, order, InsomniaPriority::Live, now_ns)?;
        let work = self.session.insomnia.claim_episode(
            &mut self.container,
            work_id,
            worker,
            now_ns,
            lease_duration_ns,
        )?;
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        Ok(work)
    }

    pub fn commit_grouped_insomnia_completion(
        &mut self,
        token: InsomniaLeaseToken,
        completion: GroupedInsomniaCompletion,
        now_ns: i64,
    ) -> Result<CompletionCheck, OwnerSessionError> {
        self.session.commit_grouped_insomnia_completion(
            &mut self.container,
            token,
            completion,
            now_ns,
        )
    }
}

/// Read-only Phylactery owner aggregate. It has Memory, Graph, Entity, Ego,
/// retention/Dream evidence, and vector reads, with no Archive or REL activity API.
pub struct PhySession {
    owner_id: [u8; 16],
    common: CommonState,
}

fn open_common(
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
    let graph = GraphStore::open(container, graph_admission)?;
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

macro_rules! common_reads {
    ($session:ty, $field:ident) => {
        impl $session {
            pub fn memory_history(
                &self,
                id: MemoryId,
            ) -> Result<Option<MemoryHistory>, OwnerSessionError> {
                match self.$field.memories.history(id) {
                    Ok(history) => Ok(Some(history)),
                    Err(MemoryError::MissingMemory) => Ok(None),
                    Err(e) => Err(e.into()),
                }
            }
            pub fn entity(&self, id: EntityId) -> Option<&Entity> {
                self.$field.perception.entity(id)
            }
            pub fn entities(&self) -> Vec<Entity> {
                self.$field.perception.entities().cloned().collect()
            }
            pub fn mention(&self, key: MemoryEntityMentionKey) -> Option<&MemoryEntityResolution> {
                self.$field.perception.mention(key)
            }
            pub fn graph_relation(
                &self,
                source: NodeRef,
                target: NodeRef,
                kind: reliquary_graph::RelationKind,
            ) -> Option<&Relation> {
                self.$field.graph.relation(source, target, kind)
            }
            pub fn graph_version(&self) -> u64 {
                self.$field.graph.graph_version()
            }
            pub fn community_snapshot(&self) -> Option<&CommunitySnapshot> {
                self.$field.communities.current()
            }
            pub fn ego_identity(&self, id: EgoIdentityId) -> Option<&EgoIdentity> {
                self.$field.ego.identity(id)
            }
            pub fn ego_personality(&self) -> Option<&EgoPersonality> {
                self.$field.ego.personality()
            }
            pub fn ego_anchor(&self, id: EgoAnchorId) -> Option<&EgoAnchor> {
                self.$field.ego.anchor(id)
            }
            pub fn retention_manifest(&self, digest: &[u8; 32]) -> Option<&HistoryPruneManifest> {
                self.$field.maintenance.get(digest)
            }
            pub fn dream_begin(
                &self,
                pass_id: &str,
            ) -> Option<&crate::records::dream::dream_begin_codec::BeginRecord> {
                self.$field.dreams.begin(pass_id)
            }
            pub fn packed_vector_info(
                &self,
                id: reliquary_vector_store::PackedVectorId,
            ) -> Option<PackedVectorInfo> {
                self.$field.packed_vectors.info(id)
            }
            pub fn compatibility_profile(
                &self,
                id: CompatibilityProfileId,
            ) -> Option<&CompatibilityProfile> {
                self.$field.profiles.get(id)
            }
            pub fn active_compatibility_profile(
                &self,
                id: CompatibilityProfileId,
            ) -> Option<&CompatibilityProfile> {
                self.$field.profiles.get_active(id)
            }
            pub fn memory_vector_infos(&self) -> Vec<MemoryVectorInfo> {
                self.$field.memory_vectors.infos()
            }
        }
    };
}

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
        let grouped_completions = validate_grouped_replay_admission(container, &archive)?;
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
        let freshness = FreshnessStore::open_with_grouped_births(container, |payload| {
            let completion = GroupedInsomniaCompletion::decode(payload)?;
            if !completion.external_memory_refs.is_empty() {
                return Err("routed grouped completion requires owner authentication".into());
            }
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
        })
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
        let archive_vectors = ArchiveVectorStore::open(container, &common.packed_vectors)?;
        let generations =
            VectorGenerationStore::open(container, &common.profiles, &archive_vectors)?;
        let external_index =
            ExternalIndexStore::open(container).map_err(OwnerSessionError::Record)?;
        let container_path = canonical_container_path(container)?;
        let container_file_identity = container.physical_file_identity()?;
        let writer_lease_proof = container.writer_lease_proof();
        Ok(Self {
            owner_id: container.owner_id(),
            archive,
            common,
            freshness,
            insomnia,
            imports,
            archive_vectors,
            generations,
            external_index,
            grouped_completions,
            write_poisoned: false,
            container_identity: container.identity(),
            container_path,
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

    pub fn owner_id(&self) -> [u8; 16] {
        self.owner_id
    }

    pub fn archive_node(&self, conversation: &str, node: &str) -> Option<&Node> {
        self.archive.node(conversation, node)
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
    pub fn commit_grouped_insomnia_completion(
        &mut self,
        container: &mut Container,
        token: InsomniaLeaseToken,
        completion: GroupedInsomniaCompletion,
        now_ns: i64,
    ) -> Result<CompletionCheck, OwnerSessionError> {
        if self.write_poisoned {
            return Err(OwnerSessionError::Record(
                "owner session requires reopen after an uncertain grouped write".into(),
            ));
        }
        if container.identity().file_kind != FileKind::Reliquary {
            return Err(OwnerSessionError::WrongFileKind {
                expected: FileKind::Reliquary,
                actual: container.identity().file_kind,
            });
        }
        if container.owner_id() != self.owner_id {
            return Err(OwnerSessionError::Record(
                "grouped completion writer belongs to another owner".into(),
            ));
        }
        if container.identity() != self.container_identity
            || canonical_container_path(container)? != self.container_path
            || container.physical_file_identity()? != self.container_file_identity
        {
            return Err(OwnerSessionError::Record(
                "grouped completion writer is not bound to this owner session file".into(),
            ));
        }
        let expected_writer_proof = self.writer_lease_proof.ok_or_else(|| {
            OwnerSessionError::Record(
                "grouped completion requires a session opened with a writer lease".into(),
            )
        })?;
        let current_writer_proof = container.writer_lease_proof().ok_or_else(|| {
            OwnerSessionError::Record(
                "grouped completion requires the writer lease held at session open".into(),
            )
        })?;
        if current_writer_proof != expected_writer_proof {
            return Err(OwnerSessionError::Record(
                "owner session is stale; reopen before grouped writes".into(),
            ));
        }
        let episode_id = WorkEpisodeId(completion.episode_id);
        let key = (episode_id, completion.attempt);
        if let Some(accepted) = self.grouped_completions.get(&key) {
            return if accepted == &completion {
                Ok(CompletionCheck::AlreadyAccepted)
            } else {
                Err(OwnerSessionError::Insomnia(InsomniaError::ReceiptConflict))
            };
        }
        let archive_episode = self
            .archive
            .episode(reliquary_archive::EpisodeId(completion.episode_id))
            .ok_or_else(|| {
                OwnerSessionError::Record("grouped completion Episode is not admitted".into())
            })?;
        let accepted_order =
            self.insomnia
                .accepted_order(episode_id)
                .ok_or(OwnerSessionError::Insomnia(
                    InsomniaError::UnacceptedEpisode,
                ))?;
        if accepted_order.source_through_ns != archive_episode.source_through_ns
            || accepted_order.conversation_id != archive_episode.conversation_id
            || accepted_order.start_node_id != archive_episode.start_node_id
        {
            return Err(OwnerSessionError::Record(
                "grouped completion Episode order differs from Archive evidence".into(),
            ));
        }
        completion
            .validate_replay_episode(completion.episode_id)
            .map_err(OwnerSessionError::Record)?;
        if !completion.routing_metadata.is_empty() {
            return Err(OwnerSessionError::Record(
                "grouped routing metadata commit is unsupported".into(),
            ));
        }
        if completion.global_version_start
            != container
                .latest_version()
                .checked_add(1)
                .ok_or_else(|| OwnerSessionError::Record("physical version exhausted".into()))?
        {
            return Err(OwnerSessionError::Record(
                "grouped completion has a stale physical version range".into(),
            ));
        }
        if !completion.memory_records.is_empty()
            && completion.freshness_birth_turn != Some(self.archive.rel_turn_count())
        {
            return Err(OwnerSessionError::Record(
                "grouped Memory births require the current accepted Freshness turn".into(),
            ));
        }

        let receipt = grouped_completion_receipt(&completion);
        if completion
            .memory_ids
            .iter()
            .any(|id| !self.common.memories.contains_memory(MemoryId(*id)))
        {
            let birth_ids = completion
                .memory_records
                .iter()
                .filter_map(|bytes| reliquary_memory_core::decode_grouped_creation(bytes).ok())
                .map(|record| record.id.0)
                .collect::<std::collections::BTreeSet<_>>();
            if completion
                .memory_ids
                .iter()
                .any(|id| !birth_ids.contains(id))
            {
                return Err(OwnerSessionError::Record(
                    "grouped completion references an absent owner Memory".into(),
                ));
            }
        }
        let bodies = completion
            .bodies
            .iter()
            .map(|body| (body.id, body.bytes.clone()))
            .collect::<Vec<_>>();
        self.common
            .memories
            .preflight_grouped_completion(bodies.clone(), completion.memory_records.clone())?;
        if let Some(turn) = completion.freshness_birth_turn {
            self.freshness
                .preflight_grouped_births(
                    self.owner_id,
                    turn,
                    completion
                        .freshness_birth_memory_ids
                        .iter()
                        .copied()
                        .map(FreshnessMemoryId)
                        .collect(),
                )
                .map_err(OwnerSessionError::Record)?;
        }
        if !self
            .insomnia
            .preflight_grouped_completion(container, token, &receipt, now_ns)?
        {
            return Err(OwnerSessionError::Insomnia(InsomniaError::ReceiptConflict));
        }
        let payload = completion.encode().map_err(OwnerSessionError::Record)?;

        if let Err(error) = container.append(&payload) {
            self.write_poisoned = true;
            return Err(error.into());
        }
        if let Err(error) = container.commit_embedded_version_range_at(
            completion.global_version_start,
            completion.memory_records.len(),
            Some(completion.transaction_time_ns),
        ) {
            self.write_poisoned = true;
            return Err(error.into());
        }
        if let Err(error) = container.sync() {
            self.write_poisoned = true;
            return Err(error.into());
        }
        #[cfg(test)]
        if self.fail_after_grouped_sync {
            self.fail_after_grouped_sync = false;
            self.write_poisoned = true;
            return Err(OwnerSessionError::Record(
                "injected failure after grouped sync".into(),
            ));
        }

        if let Err(error) = self
            .common
            .memories
            .apply_grouped_completion(bodies, completion.memory_records.clone())
        {
            self.write_poisoned = true;
            return Err(error.into());
        }
        if let Some(turn) = completion.freshness_birth_turn {
            if let Err(error) = self.freshness.apply_grouped_births(
                self.owner_id,
                turn,
                completion
                    .freshness_birth_memory_ids
                    .iter()
                    .copied()
                    .map(FreshnessMemoryId)
                    .collect(),
            ) {
                self.write_poisoned = true;
                return Err(OwnerSessionError::Record(error));
            }
        }
        match self.insomnia.apply_grouped_completion(container, receipt) {
            Ok(CompletionCheck::Apply) => {}
            Ok(CompletionCheck::AlreadyAccepted) => {
                self.write_poisoned = true;
                return Err(OwnerSessionError::Insomnia(InsomniaError::ReceiptConflict));
            }
            Err(error) => {
                self.write_poisoned = true;
                return Err(error.into());
            }
        }
        self.grouped_completions.insert(key, completion);
        let Some(next_proof) = container.writer_lease_proof() else {
            self.write_poisoned = true;
            return Err(OwnerSessionError::Record(
                "grouped writer lease disappeared after sync".into(),
            ));
        };
        self.writer_lease_proof = Some(next_proof);
        Ok(CompletionCheck::Apply)
    }

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

    #[cfg(test)]
    pub(crate) fn inject_grouped_projection_failure(&mut self) {
        self.fail_after_grouped_sync = true;
    }
}
common_reads!(RelSession, common);

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
}

impl PhySession {
    /// Replay PHY owner domains. This deliberately has no Archive, imported
    /// turn, or REL-only external-source API.
    pub fn open(
        container: &Container,
        graph_admission: &GraphAdmission,
        dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<Self, OwnerSessionError> {
        require_kind(container, FileKind::Phylactery)?;
        reject_rel_only_records(container)?;
        let common = open_common(container, graph_admission, dream_admissions, false)?;
        Ok(Self {
            owner_id: container.owner_id(),
            common,
        })
    }

    pub fn open_path(
        path: impl AsRef<Path>,
        graph_admission: &GraphAdmission,
        dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<Self, OwnerSessionError> {
        let container = Container::open_read_only_as(path, FileKind::Phylactery)?;
        Self::open(&container, graph_admission, dream_admissions)
    }

    pub fn owner_id(&self) -> [u8; 16] {
        self.owner_id
    }
}
common_reads!(PhySession, common);

fn grouped_completion_receipt(completion: &GroupedInsomniaCompletion) -> AcceptedCompletionState {
    AcceptedCompletionState {
        episode_id: WorkEpisodeId(completion.episode_id),
        attempt: completion.attempt,
        started_at_ns: completion.started_at_ns,
        completed_at_ns: completion.completed_at_ns,
        extractor_model: completion.extractor_model.clone(),
        extractor_version: completion.extractor_version.clone(),
        memory_ids: completion
            .memory_ids
            .iter()
            .copied()
            .map(WorkMemoryId)
            .collect(),
        external_memory_refs: completion
            .external_memory_refs
            .iter()
            .map(|reference| reliquary_insomnia_store::ExternalMemoryRef {
                owner_id: reference.owner_id.clone(),
                memory_id: WorkMemoryId(reference.memory_id),
            })
            .collect(),
        rejected_count: completion.rejected_count,
    }
}

fn canonical_container_path(container: &Container) -> Result<PathBuf, OwnerSessionError> {
    std::fs::canonicalize(container.path()).map_err(|error| {
        OwnerSessionError::Record(format!("cannot identify owner session container: {error}"))
    })
}

fn require_kind(container: &Container, expected: FileKind) -> Result<(), OwnerSessionError> {
    let actual = container.identity().file_kind;
    if actual == expected {
        Ok(())
    } else {
        Err(OwnerSessionError::WrongFileKind { expected, actual })
    }
}

fn validate_grouped_replay_admission(
    container: &Container,
    archive: &Archive,
) -> Result<BTreeMap<(WorkEpisodeId, u32), GroupedInsomniaCompletion>, OwnerSessionError> {
    let mut grouped = BTreeMap::new();
    for frame in container.read_frames()? {
        if !frame.payload.starts_with(b"RELINSC7") {
            continue;
        }
        let completion =
            GroupedInsomniaCompletion::decode(&frame.payload).map_err(OwnerSessionError::Record)?;
        let episode_id = reliquary_archive::EpisodeId(completion.episode_id);
        if archive.episode(episode_id).is_none() {
            return Err(OwnerSessionError::Record(
                "grouped completion Episode is not admitted by this Archive".into(),
            ));
        }
        completion
            .validate_replay_episode(completion.episode_id)
            .map_err(OwnerSessionError::Record)?;
        if !completion.routing_metadata.is_empty() {
            return Err(OwnerSessionError::Record(
                "grouped routing metadata replay is unsupported".into(),
            ));
        }
        let key = (WorkEpisodeId(completion.episode_id), completion.attempt);
        if grouped.insert(key, completion).is_some() {
            return Err(OwnerSessionError::Record(
                "duplicate grouped completion attempt".into(),
            ));
        }
    }
    Ok(grouped)
}

fn reject_rel_only_records(container: &Container) -> Result<(), OwnerSessionError> {
    const REL_ONLY: [(&[u8; 8], &str); 19] = [
        (b"RELTURN2", "Archive turn"),
        (b"RELFILE1", "Archive file"),
        (b"RELBRCH1", "Archive branch"),
        (b"RELEPIS1", "Archive Episode"),
        (b"RELECHO1", "Archive Echo"),
        (b"RELFRS02", "Freshness journal"),
        (b"RELFRP01", "Freshness policy"),
        (b"RELIEPS1", "import epoch start"),
        (b"RELIEPC1", "import epoch completion"),
        (b"RELINSW1", "Insomnia work"),
        (b"RELINSR1", "Insomnia receipt"),
        (b"RELINSC7", "grouped Insomnia completion"),
        (b"RELAVFM1", "Archive vector format"),
        (b"RELAVEC1", "Archive vector"),
        (b"RELVGFM1", "vector generation format"),
        (b"RELVGEN1", "vector generation"),
        (b"RELVGRC1", "vector generation revision"),
        (b"RELXIDX2", "external source index"),
        (b"RELXMEM2", "external Memory reference"),
    ];
    use std::io::Read;
    let mut cursor = container.physical_cursor()?;
    while let Some(mut frame) = cursor.next_frame()? {
        let mut prefix = [0u8; 8];
        let mut length = 0;
        while length < prefix.len() {
            let count = frame
                .read(&mut prefix[length..])
                .map_err(ContainerError::Io)?;
            if count == 0 {
                break;
            }
            length += count;
        }
        if length == prefix.len() {
            for (marker, domain) in REL_ONLY {
                if &prefix == marker {
                    return Err(OwnerSessionError::RelOnlyRecordInPhylactery(domain));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::{ContainerIdentity, ReliquaryScope};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".owner-session-test-tmp");
            fs::create_dir_all(&root).unwrap();
            Self(root.join(format!(
                "owner-session-{}-{}.rel",
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
    fn initialize_vectors(container: &mut Container) {
        PackedVectorStore::default().initialize(container).unwrap();
        CompatibilityProfileStore::default()
            .initialize(container)
            .unwrap();
        ArchiveVectorStore::default().initialize(container).unwrap();
        MemoryVectorStore::default().initialize(container).unwrap();
        VectorGenerationStore::empty()
            .initialize(container)
            .unwrap();
        container.sync().unwrap();
    }

    #[test]
    fn rel_writer_owns_the_exclusive_container_lease() {
        let path = Temp::new();
        let mut container = Container::create(
            &path.0,
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            [9; 16],
        )
        .unwrap();
        initialize_vectors(&mut container);
        drop(container);

        let admission = GraphAdmission::for_replay([9; 16], [], [], [], [], []);
        let writer = RelWriter::open_path(&path.0, admission, [], []).unwrap();
        assert_eq!(writer.owner_id(), [9; 16]);
        assert!(Container::open_writer(&path.0).is_err());
        drop(writer);

        let reopened = Container::open_writer(&path.0).unwrap();
        drop(reopened);
    }

    #[test]
    fn accepted_turn_and_branch_are_idempotent_after_writer_reopen() {
        let path = Temp::new();
        let owner = [19; 16];
        let mut container = Container::create(
            &path.0,
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            owner,
        )
        .unwrap();
        initialize_vectors(&mut container);
        drop(container);

        let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
        let turn = || reliquary_archive::TurnInput {
            id: "submission-1".to_owned(),
            conversation_id: "conversation-1".to_owned(),
            parent_id: None,
            role: "user".to_owned(),
            principal_id: Some("alice".to_owned()),
            timestamp_ns: 42,
            content: "durable content".to_owned(),
            attachments: Vec::new(),
            project_attachments: Vec::new(),
        };
        let branch = || reliquary_archive::Branch {
            id: "main".to_owned(),
            conversation_id: "conversation-1".to_owned(),
            leaf_node_id: "submission-1".to_owned(),
            canonical: true,
        };

        let mut writer = RelWriter::open_path(&path.0, admission.clone(), [], []).unwrap();
        writer
            .accept_turn_and_advance_branch(turn(), branch())
            .unwrap();
        drop(writer);

        let mut recovered = RelWriter::open_path(&path.0, admission, [], []).unwrap();
        recovered
            .accept_turn_and_advance_branch(turn(), branch())
            .unwrap();
        assert_eq!(recovered.session.archive.node_count(), 1);
        assert_eq!(
            recovered
                .session
                .archive
                .branch("conversation-1", "main")
                .unwrap()
                .leaf_node_id,
            "submission-1"
        );
    }

    #[test]
    fn rel_and_phy_replay_through_distinct_typed_aggregates() {
        let rel_path = Temp::new();
        let mut owner = [0; 16];
        owner[0] = 1;
        let mut rel = Container::create(
            &rel_path.0,
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            owner,
        )
        .unwrap();
        initialize_vectors(&mut rel);
        let mut archive = Archive::empty();
        archive
            .accept_turn(
                &mut rel,
                reliquary_archive::TurnInput {
                    id: "assistant-1".into(),
                    conversation_id: "conversation-1".into(),
                    parent_id: None,
                    role: "assistant".into(),
                    principal_id: None,
                    timestamp_ns: 12,
                    content: "replayed turn".into(),
                    attachments: vec![],
                    project_attachments: vec![],
                },
            )
            .unwrap();
        rel.sync().unwrap();
        let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
        let rel_session = RelSession::open(&rel, &graph, [], []).unwrap();
        assert_eq!(rel_session.owner_id(), owner);
        assert_eq!(
            rel_session
                .archive_node("conversation-1", "assistant-1")
                .unwrap()
                .role,
            "assistant"
        );
        assert_eq!(rel_session.accepted_turn_sources().len(), 1);
        assert!(matches!(
            PhySession::open(&rel, &graph, []),
            Err(OwnerSessionError::WrongFileKind { .. })
        ));

        let phy_path = Temp::new();
        let mut phy =
            Container::create(&phy_path.0, ContainerIdentity::phylactery(), owner).unwrap();
        PackedVectorStore::default().initialize(&mut phy).unwrap();
        CompatibilityProfileStore::default()
            .initialize(&mut phy)
            .unwrap();
        MemoryVectorStore::default().initialize(&mut phy).unwrap();
        phy.sync().unwrap();
        let phy_session = PhySession::open(&phy, &graph, []).unwrap();
        assert_eq!(phy_session.owner_id(), owner);
        assert_eq!(phy_session.graph_version(), 0);
        assert!(
            phy_session
                .memory_history(MemoryId([8; 32]))
                .unwrap()
                .is_none()
        );
        assert!(phy_session.entity(EntityId([8; 32])).is_none());
        assert!(phy_session.memory_vector_infos().is_empty());
        assert!(matches!(
            RelSession::open(&phy, &graph, [], []),
            Err(OwnerSessionError::WrongFileKind { .. })
        ));
    }

    #[test]
    fn phy_rejects_rel_only_record_families_without_mutation() {
        for marker in [
            b"RELTURN2",
            b"RELFILE1",
            b"RELBRCH1",
            b"RELEPIS1",
            b"RELECHO1",
            b"RELFRS02",
            b"RELFRP01",
            b"RELIEPS1",
            b"RELIEPC1",
            b"RELINSW1",
            b"RELINSR1",
            b"RELAVFM1",
            b"RELAVEC1",
            b"RELVGFM1",
            b"RELVGEN1",
            b"RELVGRC1",
            b"RELXIDX2",
        ] {
            let path = Temp::new();
            let owner = [7; 16];
            let mut phy =
                Container::create(&path.0, ContainerIdentity::phylactery(), owner).unwrap();
            phy.append(marker).unwrap();
            phy.sync().unwrap();
            drop(phy);
            let before = fs::read(&path.0).unwrap();
            let phy = Container::open_read_only(&path.0).unwrap();
            let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
            assert!(matches!(
                PhySession::open(&phy, &admission, []),
                Err(OwnerSessionError::RelOnlyRecordInPhylactery(_))
            ));
            drop(phy);
            assert_eq!(fs::read(&path.0).unwrap(), before);
        }
    }

    #[test]
    fn graph_replay_requires_owner_scoped_closure_admission() {
        let path = Temp::new();
        let mut owner = [0; 16];
        owner[0] = 2;
        let mut container = Container::create(
            &path.0,
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            owner,
        )
        .unwrap();
        initialize_vectors(&mut container);
        let foreign = GraphAdmission::for_replay([9; 16], [], [], [], [], []);
        assert!(matches!(
            RelSession::open(&container, &foreign, [], []),
            Err(OwnerSessionError::Graph(_))
        ));
    }
}
