use crate::records::external_index::{ExternalIndexStore, SourceState};
use crate::records::external_memory::ExternalMemoryStore;
use crate::records::external_reference::ExternalReferenceStore;
use crate::records::{
    DreamBeginAdmission, DreamEvidenceStore, ExternalReferenceSnapshotManifest,
    ExternalReferenceSnapshotReceipt, GroupedInsomniaCompletion, HistoryPruneManifest,
    ImportEpochStatus, ImportEpochStore, OwnerMaintenanceStore,
};
use reliquary_archive::{Archive, EchoStore, Node};
use reliquary_container::{
    Container, ContainerError, ContainerIdentity, FileKind, WriterLeaseProof,
};
use reliquary_ego_store::{
    EgoAnchor, EgoAnchorId, EgoError, EgoIdentity, EgoIdentityId, EgoPersonality, EgoStore,
};
use reliquary_freshness_store::{
    FreshnessEventProof, FreshnessMemoryId, FreshnessRecord, FreshnessStore, FreshnessUseReceipt,
};
use reliquary_graph::{
    CommunitySnapshot, CommunityStore, GraphAdmission, GraphError, GraphStore, NodeRef, Relation,
};
use reliquary_insomnia_store::{
    AcceptedCompletionState, AcceptedEpisodeOrder, CompletionCheck, EpisodeId as WorkEpisodeId,
    InsomniaError, InsomniaLeaseToken, InsomniaPriority, InsomniaStore, InsomniaWork,
    MemoryId as WorkMemoryId,
};
use reliquary_memory_core::{
    MemoryBodyId, MemoryDraft, MemoryError, MemoryHistory, MemoryId, MemoryRevision, MemoryStore,
    PayloadState, TerminalTombstone,
};
use reliquary_perception_store::{
    Entity, EntityId, EntityResolutionReason, EntityTombstone, MemoryEntityMentionKey,
    MemoryEntityResolution, MemoryEntityResolutionStatus, MemoryTextField, PerceptionError,
    PerceptionStore, Relationship, RelationshipDraft, RelationshipEntityRef, RelationshipId,
    RelationshipMemoryRef, RelationshipParticipant, RelationshipTombstone,
};
use reliquary_vector_store::{
    ArchiveVectorError, ArchiveVectorInfo, ArchiveVectorStore, CompatibilityProfile,
    CompatibilityProfileId, CompatibilityProfileStore, GenerationStoreError, MemoryVectorError,
    MemoryVectorInfo, MemoryVectorStore, PackedVectorError, PackedVectorInfo, PackedVectorStore,
    ProfileStoreError, VectorGeneration, VectorGenerationId, VectorGenerationStore,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::{Path, PathBuf},
};

use self::fresh_history_rewrite::owner_history_generation_from_container;

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
    StaleDreamCut,
    ManagementMissing,
    ManagementLimit,
    ManagementStale,
    ManagementEvidence,
    ManagementConflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerFailureKind {
    Io,
    UnsupportedFormat,
    WrongFileKind,
    Admission,
    WriterConflict,
    Corruption,
}

impl OwnerSessionError {
    pub fn failure_kind(&self) -> OwnerFailureKind {
        match self {
            Self::WrongFileKind { .. } | Self::Container(ContainerError::WrongFileKind { .. }) => {
                OwnerFailureKind::WrongFileKind
            }
            Self::Container(ContainerError::Io(_)) => OwnerFailureKind::Io,
            Self::Container(
                ContainerError::InvalidSignature
                | ContainerError::UnsupportedFormatSignature
                | ContainerError::UnsupportedVersion(_),
            ) => OwnerFailureKind::UnsupportedFormat,
            Self::Container(
                ContainerError::WriterConflict | ContainerError::WriteRecoveryRequired,
            ) => OwnerFailureKind::WriterConflict,
            Self::Graph(GraphError::WrongOwner)
            | Self::StaleDreamCut
            | Self::Insomnia(InsomniaError::UnacceptedEpisode) => OwnerFailureKind::Admission,
            _ => OwnerFailureKind::Corruption,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerScope {
    Unspecified,
    Organization,
    Project,
    Connection,
}

impl OwnerScope {
    pub(crate) fn into_container_scope(self) -> reliquary_container::ReliquaryScope {
        match self {
            Self::Unspecified => reliquary_container::ReliquaryScope::Unspecified,
            Self::Organization => reliquary_container::ReliquaryScope::Organization,
            Self::Project => reliquary_container::ReliquaryScope::Project,
            Self::Connection => reliquary_container::ReliquaryScope::Connection,
        }
    }
}

impl From<reliquary_container::ReliquaryScope> for OwnerScope {
    fn from(value: reliquary_container::ReliquaryScope) -> Self {
        match value {
            reliquary_container::ReliquaryScope::Unspecified => Self::Unspecified,
            reliquary_container::ReliquaryScope::Organization => Self::Organization,
            reliquary_container::ReliquaryScope::Project => Self::Project,
            reliquary_container::ReliquaryScope::Connection => Self::Connection,
        }
    }
}

impl fmt::Display for OwnerSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManagementMissing => f.write_str("owner inspection item is absent"),
            Self::ManagementLimit => f.write_str("owner inspection limit exceeded"),
            Self::ManagementStale => f.write_str("semantic mutation revision is stale"),
            Self::ManagementEvidence => f.write_str("semantic mutation evidence is invalid"),
            Self::ManagementConflict => f.write_str("semantic mutation request identity conflicts"),
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
            Self::StaleDreamCut => f.write_str("Dream owner cut changed before publication"),
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
    user_memory_requests: BTreeMap<String, crate::records::user_memory::UserMemoryRecord>,
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
    dream_pairs: dream_pairs::DreamPairs,
    dream_read_index: dream_read_index::DreamReadIndex,
}

/// Read-only Reliquary owner aggregate. Archive and turn authority exist only
/// here; private fields prevent exposing a public bag of stores.
pub struct RelSession {
    owner_id: [u8; 16],
    archive: Archive,
    echo: EchoStore,
    common: CommonState,
    freshness: FreshnessStore,
    insomnia: InsomniaStore,
    imports: ImportEpochStore,
    archive_vectors: ArchiveVectorStore,
    generations: VectorGenerationStore,
    external_index: ExternalIndexStore,
    external_references: ExternalReferenceStore,
    external_memories: ExternalMemoryStore,
    grouped_completions: BTreeMap<(WorkEpisodeId, u32), GroupedInsomniaCompletion>,
    native_import_grouped_epochs: BTreeMap<(WorkEpisodeId, u32), String>,
    write_poisoned: bool,
    container_identity: ContainerIdentity,
    container_path: PathBuf,
    history_generation: [u8; 32],
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

/// Immutable read cut for retrieval. Reading this value never reinforces
/// Freshness; only an explicit accepted-use write can do that.
#[derive(Clone, Debug, PartialEq)]
pub struct RelRetrievalSnapshot {
    pub owner_id: [u8; 16],
    pub rel_turn: u64,
    pub memory_version: u64,
    pub graph_version: u64,
    pub memories: Vec<RelRetrievalMemory>,
    pub active_relations: Vec<Relation>,
    pub community: Option<CommunitySnapshot>,
    pub vectors: BTreeMap<reliquary_memory_core::MemoryBodyId, Vec<f32>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RelRetrievalMemory {
    pub revision: MemoryRevision,
    pub freshness: Option<FreshnessRecord>,
}

/// Owner-validated work input returned after the durable lease claim commits.
/// Callers may run model inference on this snapshot only after the writer lock
/// is released; completion must still present the lease token.
#[derive(Clone, Debug)]
pub struct RelInsomniaClaimInput {
    pub owner_id: [u8; 16],
    pub work: InsomniaWork,
    pub episode: reliquary_archive::Episode,
    pub turns: Vec<reliquary_archive::ResolvedTurn>,
    pub started_at_ns: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeImportMemoryAvailability {
    Supported { position: u64 },
    Unsupported,
    CreditExcluded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeImportHistoricalEvent {
    pub source: MemoryId,
    pub position: u64,
    pub roots: BTreeMap<MemoryId, i16>,
    pub credit_excluded: BTreeSet<MemoryId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeImportRetainedEvent {
    pub recipient: MemoryId,
    pub position: u64,
    pub event_id: [u8; 32],
    pub amount: i16,
}

/// Immutable owner proof for Application's pure historical Freshness scorer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeImportFreshnessInput {
    pub owner_id: [u8; 16],
    pub import_id: String,
    pub import_digest: [u8; 32],
    pub base_cut: u64,
    pub settlement_cut: u64,
    pub evidence_digest: [u8; 32],
    pub initial_records: BTreeMap<MemoryId, FreshnessRecord>,
    pub historical_events: Vec<NativeImportHistoricalEvent>,
    pub retained_events: Vec<NativeImportRetainedEvent>,
    pub adjacency: BTreeMap<MemoryId, BTreeSet<MemoryId>>,
    pub memberships: BTreeMap<MemoryId, reliquary_graph::CommunityId>,
    pub authorized: BTreeSet<MemoryId>,
    pub availability: BTreeMap<MemoryId, NativeImportMemoryAvailability>,
}

/// Read-only Phylactery owner aggregate. It has Memory, Graph, Entity, Ego,
/// retention/Dream evidence, and vector reads, with no Archive or REL activity API.
pub struct PhySession {
    owner_id: [u8; 16],
    common: CommonState,
    history_generation: [u8; 32],
    container_file_identity: (u64, u64),
}

/// Narrow PHY writer for user-owned Memories routed from an authorized REL.
/// The Gateway owns the grant check; this writer exposes no general Container
/// mutation or Archive capability.
pub struct PhyWriter {
    container: Container,
    owner_id: [u8; 16],
    common: CommonState,
    history_generation: [u8; 32],
    container_file_identity: (u64, u64),
    writer_lease_proof: Option<WriterLeaseProof>,
    poisoned: bool,
}

/// Unforgeable source and payload proof for one user-owned Memory routed from
/// an opened REL to a specific PHY writer. Only `RelSession` can mint it after
/// checking the exact Archive turn, principal and quoted evidence. The proof
/// authenticates owner data, not a principal's Gateway grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutedUserMemoryProof {
    source_owner_id: [u8; 16],
    destination_owner_id: [u8; 16],
    source_episode_id: [u8; 32],
    draft: MemoryDraft,
}

/// Authenticated receipt for a User Memory durably published to a PHY.
/// Only `PhyWriter` can mint this after its append and sync have succeeded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutedUserMemoryReceipt {
    source_owner_id: [u8; 16],
    source_episode_id: [u8; 32],
    destination_owner_id: [u8; 16],
    memory_id: MemoryId,
    published_draft: MemoryDraft,
}

impl RoutedUserMemoryReceipt {
    /// Return the owner-qualified reference suitable for the REL completion
    /// carrier. The REL commit still authenticates this receipt separately.
    pub fn external_ref(&self) -> crate::records::GroupedExternalMemoryRef {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut owner_id = String::with_capacity(32);
        for byte in self.destination_owner_id {
            owner_id.push(HEX[usize::from(byte >> 4)] as char);
            owner_id.push(HEX[usize::from(byte & 0x0f)] as char);
        }
        crate::records::GroupedExternalMemoryRef {
            owner_id,
            memory_id: self.memory_id.0,
        }
    }

    /// Check the full immutable routed draft accepted by PHY. Identity alone
    /// is insufficient because Memory wording can change without changing
    /// its stable semantic key.
    pub fn matches_draft(&self, draft: &MemoryDraft) -> bool {
        &self.published_draft == draft
    }
}

/// The exact owner-local cut used by a semantic graph read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticGraphCut {
    pub owner_id: [u8; 16],
    pub generation: [u8; 32],
    pub archive: Option<u64>,
    pub memory: u64,
    pub perception: u64,
    pub relationship: u64,
    pub graph: u64,
    pub community: Option<u64>,
}

/// Renderer-facing Memory metadata. Body content is never included.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticGraphMemory {
    pub id: MemoryId,
    pub revision: u64,
    pub title: Option<String>,
    pub origin: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticGraphMemoryPage {
    pub memories: Vec<SemanticGraphMemory>,
    pub next: Option<MemoryId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticGraphRelationPage {
    pub relations: Vec<Relation>,
    pub next: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticGraphCommunity {
    pub id: reliquary_graph::CommunityId,
    pub name: Option<String>,
    pub member_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticGraphCommunityPage {
    pub communities: Vec<SemanticGraphCommunity>,
    pub next: Option<reliquary_graph::CommunityId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticGraphCommunityMemberPage {
    pub members: Vec<MemoryId>,
    pub next: Option<MemoryId>,
}
/// Immutable, bounded-page view of one admitted owner graph cut.
/// It exposes no storage handles and remains coherent for its borrow lifetime.
pub struct SemanticGraphSnapshot<'a> {
    common: &'a CommonState,
    cut: SemanticGraphCut,
}

impl SemanticGraphSnapshot<'_> {
    pub fn cut(&self) -> SemanticGraphCut {
        self.cut
    }

    pub fn memory_page(
        &self,
        after: Option<MemoryId>,
        limit: usize,
    ) -> Result<SemanticGraphMemoryPage, OwnerSessionError> {
        if !(1..=128).contains(&limit) {
            return Err(OwnerSessionError::Record(
                "semantic graph page limit must be between 1 and 128".into(),
            ));
        }
        let ids = self
            .common
            .memories
            .memory_ids_page(after, (limit + 1).min(128));
        let has_more = ids.len() > limit
            || (ids.len() == limit
                && self
                    .common
                    .memories
                    .memory_ids_page(ids.last().copied(), 1)
                    .len()
                    > 0);
        let memories = ids
            .into_iter()
            .take(limit)
            .map(|id| {
                let (record, body) = self.common.memories.revision_projection(id, None)?;
                Ok(SemanticGraphMemory {
                    id,
                    revision: record.revision,
                    title: body.map(|body| body.title.clone()),
                    origin: record.authority_kind.clone(),
                })
            })
            .collect::<Result<Vec<_>, MemoryError>>()?;
        let next = has_more.then(|| memories.last().expect("nonempty page when has_more").id);
        Ok(SemanticGraphMemoryPage { memories, next })
    }

    pub fn relation_page(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<SemanticGraphRelationPage, OwnerSessionError> {
        if !(1..=128).contains(&limit) {
            return Err(OwnerSessionError::Record(
                "semantic graph page limit must be between 1 and 128".into(),
            ));
        }
        let relations = self
            .common
            .graph
            .active_relations()
            .skip(offset)
            .take(limit + 1)
            .copied()
            .collect::<Vec<_>>();
        let next = (relations.len() > limit).then_some(offset.saturating_add(limit));
        Ok(SemanticGraphRelationPage {
            relations: relations.into_iter().take(limit).collect(),
            next,
        })
    }

    pub fn community_page(
        &self,
        after: Option<reliquary_graph::CommunityId>,
        limit: usize,
    ) -> Result<SemanticGraphCommunityPage, OwnerSessionError> {
        if !(1..=128).contains(&limit) {
            return Err(OwnerSessionError::Record(
                "semantic graph page limit must be between 1 and 128".into(),
            ));
        }
        let Some(snapshot) = self.common.communities.current() else {
            return Ok(SemanticGraphCommunityPage {
                communities: Vec::new(),
                next: None,
            });
        };
        let start = after.map_or(0, |id| {
            snapshot
                .communities
                .partition_point(|community| community.id <= id)
        });
        let rows = snapshot
            .communities
            .iter()
            .skip(start)
            .take(limit + 1)
            .collect::<Vec<_>>();
        let next = (rows.len() > limit).then(|| rows[limit - 1].id);
        let communities = rows
            .into_iter()
            .take(limit)
            .map(|community| SemanticGraphCommunity {
                id: community.id,
                name: self
                    .common
                    .communities
                    .user_name(community.id)
                    .map(str::to_owned),
                member_count: community.members.len(),
            })
            .collect();
        Ok(SemanticGraphCommunityPage { communities, next })
    }

    pub fn community_member_page(
        &self,
        community_id: reliquary_graph::CommunityId,
        after: Option<MemoryId>,
        limit: usize,
    ) -> Result<SemanticGraphCommunityMemberPage, OwnerSessionError> {
        if !(1..=128).contains(&limit) {
            return Err(OwnerSessionError::Record(
                "semantic graph page limit must be between 1 and 128".into(),
            ));
        }
        let Some(community) = self.common.communities.current().and_then(|snapshot| {
            snapshot
                .communities
                .iter()
                .find(|community| community.id == community_id)
        }) else {
            return Err(OwnerSessionError::ManagementMissing);
        };
        let start = after.map_or(0, |id| {
            community.members.partition_point(|member| *member <= id.0)
        });
        let rows = community
            .members
            .iter()
            .skip(start)
            .take(limit + 1)
            .copied()
            .collect::<Vec<_>>();
        let next = (rows.len() > limit).then(|| MemoryId(rows[limit - 1]));
        Ok(SemanticGraphCommunityMemberPage {
            members: rows.into_iter().take(limit).map(MemoryId).collect(),
            next,
        })
    }
}
macro_rules! common_reads {
    ($session:ty, $field:ident) => {
        impl $session {
            /// Enumerate retained Memory identities in stable owner-local order.
            /// The returned clock lets callers reject a mixed multi-page read.
            pub fn memory_ids_page(
                &self,
                after: Option<MemoryId>,
                limit: usize,
            ) -> Result<(u64, Vec<MemoryId>), OwnerSessionError> {
                if !(1..=128).contains(&limit) {
                    return Err(OwnerSessionError::Record(
                        "Memory page limit must be between 1 and 128".into(),
                    ));
                }
                let ids = self.$field.memories.memory_ids_page(after, limit);
                Ok((self.$field.memories.memory_version(), ids))
            }
            pub fn memory_version(&self) -> u64 {
                self.$field.memories.memory_version()
            }
            pub fn current_memory(&self,id:MemoryId)->Result<MemoryRevision,OwnerSessionError>{Ok(self.$field.memories.current(id)?)}
            pub fn dream_candidate_pool(&self,source:MemoryId,profile:CompatibilityProfileId,row:&[f32])->Result<DreamCandidatePool,OwnerSessionError>{
                if self.$field.dream_read_index.version!=self.$field.memories.memory_version(){return Err(OwnerSessionError::StaleDreamCut);}
                let current=self.$field.memories.record_at_cut(source,self.$field.memories.memory_version()).ok_or(OwnerSessionError::StaleDreamCut)?;
                if !self.$field.dream_read_index.active.contains(&source){return Err(OwnerSessionError::StaleDreamCut);}
                Ok(self.$field.dream_read_index.query(source,self.$field.memories.first_memory_version(source).ok_or(OwnerSessionError::StaleDreamCut)?,current.source_time_ns,profile,row))
            }
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
            /// Return one Memory's retained revisions visible at its owner-local
            /// publication cut. Payload availability remains the store's current
            /// retained-body fact; a later prune does not recreate old bytes.
            pub fn memory_history_at_cut(
                &self,
                id: MemoryId,
                memory_version: u64,
            ) -> Result<Option<MemoryHistory>, OwnerSessionError> {
                if memory_version > self.$field.memories.memory_version() {
                    return Err(OwnerSessionError::Record(
                        "Memory history cut exceeds the accepted owner version".into(),
                    ));
                }
                let Some(mut history) = self.memory_history(id)? else {
                    return Ok(None);
                };
                history
                    .revisions
                    .retain(|revision| revision.record.memory_version <= memory_version);
                if history.revisions.is_empty() {
                    return Ok(None);
                }
                if history
                    .tombstone
                    .is_some_and(|tombstone| tombstone.memory_version > memory_version)
                {
                    history.tombstone = None;
                }
                Ok(Some(history))
            }
            pub fn entity(&self, id: EntityId) -> Option<&Entity> {
                self.$field.perception.entity(id)
            }
            pub fn entity_revision(&self, id: EntityId, revision: u64) -> Option<&Entity> {
                self.$field.perception.entity_revision(id, revision)
            }
            pub fn entity_tombstone(&self, id: EntityId) -> Option<reliquary_perception_store::EntityTombstone> {
                self.$field.perception.entity_tombstone(id)
            }
            pub fn entity_description_prune(&self, id: EntityId, revision: u64) -> Option<reliquary_perception_store::EntityDescriptionPrune> {
                self.$field.perception.description_prune(id, revision)
            }
            pub fn entities(&self) -> Vec<Entity> {
                self.$field.perception.entities().cloned().collect()
            }
            pub fn relationship_revision(&self, id: RelationshipId, revision: u64) -> Option<&Relationship> {
                self.$field.perception.relationship_revision(id, revision)
            }
            pub fn relationship_tombstone(&self, id: RelationshipId) -> Option<RelationshipTombstone> {
                self.$field.perception.relationship_tombstone(id)
            }
            pub fn relationship_description_prune(&self, id: RelationshipId, revision: u64) -> Option<reliquary_perception_store::RelationshipDescriptionPrune> {
                self.$field.perception.relationship_description_prune(id, revision)
            }
            pub fn mention(&self, key: MemoryEntityMentionKey) -> Option<&MemoryEntityResolution> {
                self.$field.perception.mention(key)
            }
            pub fn mention_revision(
                &self,
                key: MemoryEntityMentionKey,
                revision: u32,
            ) -> Option<&MemoryEntityResolution> {
                self.$field.perception.mention_revision(key, revision)
            }
            pub fn entity_resolutions(&self) -> Vec<MemoryEntityResolution> {
                self.$field.perception.mentions().cloned().collect()
            }
            /// Expand selected Memories through the owner-local semantic closure
            /// used by fresh-history reconstruction.
            pub fn fresh_history_memory_closure(
                &self,
                retained: impl IntoIterator<Item = MemoryId>,
            ) -> Result<Vec<MemoryId>, OwnerSessionError> {
                let retained = retained.into_iter().collect::<BTreeSet<_>>();
                fresh_history_rewrite::semantic_memory_closure(
                    &self.$field,
                    self.owner_id,
                    retained,
                )
                .map_err(OwnerSessionError::Record)
            }
            pub fn graph_relations(&self) -> Vec<Relation> {
                self.$field.graph.active_relations().copied().collect()
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
            pub fn dream_duplicate_context(&self,seeds:&[MemoryId])->Result<(Vec<reliquary_memory_core::MemoryRecord>,Vec<Relation>),OwnerSessionError> {
                let (ids,relations)=self.$field.graph.duplicate_context(&seeds.iter().map(|id|id.0).collect::<Vec<_>>(),128)?;
                let mut records=Vec::new();
                for id in ids {records.push(self.$field.memories.record_at_cut(MemoryId(id),self.$field.memories.memory_version()).ok_or(OwnerSessionError::StaleDreamCut)?.clone());}
                Ok((records,relations))
            }
            pub fn initial_dream_frontier(&self,pass:&str)->Option<crate::InitialDreamFrontier>{self.$field.dreams.initial_frontier(pass)}
            pub fn dream_begin(
                &self,
                pass_id: &str,
            ) -> Option<&crate::records::dream::dream_begin_codec::BeginRecord> {
                self.$field.dreams.begin(pass_id)
            }
            pub fn dream_pair_evaluated(&self, left: MemoryId, right: MemoryId) -> bool {
                self.$field.dream_pairs.contains(left, right)
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
            /// Read one profile-qualified Memory-body vector from this owner.
            pub fn memory_vector_row(
                &self,
                profile: CompatibilityProfileId,
                body: reliquary_memory_core::MemoryBodyId,
            ) -> Result<Option<Vec<f32>>, OwnerSessionError> {
                if self.$field.profiles.get_active(profile).is_none() {
                    return Err(OwnerSessionError::Record(
                        "embedding profile is unavailable".into(),
                    ));
                }
                let Some(location) = self
                    .$field
                    .memory_vectors
                    .location(profile, reliquary_vector_store::MemoryBodyId(body.0))
                else {
                    return Ok(None);
                };
                let mapping = self.$field.memory_vectors.info(location.set_id).ok_or(MemoryVectorError::MissingObject)?;
                let row = self
                    .$field
                    .packed_vectors
                    .read_f32_row(mapping.packed_vector_id, location.row)?;
                Ok(Some(row))
            }
        }
    };
}

impl RelSession {
    /// Bind bounded semantic graph pages to the same immutable owner cut.
    pub fn semantic_graph_snapshot(&self) -> SemanticGraphSnapshot<'_> {
        let (memory, perception, relationship, graph) = self.management_read().versions();
        let community = self
            .common
            .communities
            .current()
            .map(|snapshot| snapshot.generation);
        SemanticGraphSnapshot {
            common: &self.common,
            cut: SemanticGraphCut {
                owner_id: self.owner_id,
                generation: self.history_generation,
                archive: Some(self.archive_version()),
                memory,
                perception,
                relationship,
                graph,
                community,
            },
        }
    }
}
common_reads!(RelSession, common);

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
        retired_profile_prune::validate_common_profile_prune(
            &common.profiles,
            &common.packed_vectors,
            &common.memory_vectors,
            &common.maintenance,
        )?;
        Ok(Self {
            owner_id: container.owner_id(),
            common,
            history_generation: owner_history_generation_from_container(container)
                .map_err(OwnerSessionError::Record)?,
            container_file_identity: container.physical_file_identity()?,
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

    /// Open and fully admit one current-format PHY from durable owner evidence.
    pub fn open_path_current(path: impl AsRef<Path>) -> Result<Self, OwnerSessionError> {
        let container = Container::open_read_only_as(path, FileKind::Phylactery)?;
        require_kind(&container, FileKind::Phylactery)?;
        reject_rel_only_records(&container)?;
        let common = open_common_for_owner(&container, container.owner_id(), [], false)?;
        Ok(Self {
            owner_id: container.owner_id(),
            common,
            history_generation: owner_history_generation_from_container(&container)
                .map_err(OwnerSessionError::Record)?,
            container_file_identity: container.physical_file_identity()?,
        })
    }

    pub fn owner_id(&self) -> [u8; 16] {
        self.owner_id
    }

    pub fn scope(&self) -> OwnerScope {
        OwnerScope::Unspecified
    }
}
impl PhySession {
    /// Bind bounded semantic graph pages to the same immutable owner cut.
    pub fn semantic_graph_snapshot(&self) -> SemanticGraphSnapshot<'_> {
        let read = self.management_read();
        let (memory, perception, relationship, graph) = read.versions();
        let community = self
            .common
            .communities
            .current()
            .map(|snapshot| snapshot.generation);
        SemanticGraphSnapshot {
            common: &self.common,
            cut: SemanticGraphCut {
                owner_id: self.owner_id,
                generation: self.history_generation,
                archive: None,
                memory,
                perception,
                relationship,
                graph,
                community,
            },
        }
    }
}
common_reads!(PhySession, common);
common_reads!(PhyWriter, common);

#[cfg(test)]
mod tests;

mod archive_evidence;
mod archive_prune;
mod archive_vector_publication;
mod create;
pub(crate) mod dream_pairs;
pub(crate) mod dream_prefix;
mod entity_prune;
mod external_memory;
mod external_reference;
mod fresh_history_rewrite;
mod freshness_use;
#[cfg(test)]
#[path = "session/gm_parity_regressions.rs"]
mod gm_parity_regressions;
mod historical_proof;
mod maintenance_prune;
mod perception_lifecycle;
mod relationship_prune;
mod semantic_reconciliation;
pub use entity_prune::{EntityPruneReport, prune_terminal_entity_descriptions_copy};
pub use fresh_history_rewrite::{
    FreshHistoryArchive, FreshHistoryFreshness, FreshHistoryIdentity, FreshHistoryLineage,
    FreshHistoryRequest, FreshHistoryRewriteReport, fresh_history_lineage,
    owner_history_generation, rewrite_memory_history_copy,
};
pub use historical_proof::{
    HistoricalOwnerCut, HistoricalOwnerProof, HistoricalProofError, RelHistoricalCut,
};
pub use perception_lifecycle::{
    CreateRelationshipRequest, EntityRetirementRequest, MemoryRetirementRequest,
    RelationshipParticipantEvidence, RelationshipRetirementRequest, WithdrawEntityMentionRequest,
};
pub use relationship_prune::{
    RelationshipPruneReport, prune_terminal_relationship_descriptions_copy,
};
pub use semantic_reconciliation::{
    RelHistoryComparison, RelHistoryRelation, RelReconciliationError, RelReconciliationReport,
    compare_rel_histories, reconcile_divergent_rel_copy,
};
mod maintenance_prune_proof;
mod profile_repair;
mod retired_profile_prune;
mod storage_reclamation;
pub use archive_prune::{ArchivePruneReport, prune_terminal_archive_copy};
pub use profile_repair::{
    ProfileRepairError, ProfileRepairPlan, ProfileRepairReport, execute_profile_repair,
    plan_profile_repair,
};
pub use retired_profile_prune::{RetiredProfilePruneReport, prune_retired_profile_vectors_copy};
pub use storage_reclamation::{StorageReclamationReport, reclaim_storage_copy};
mod native_import;
pub use native_import::InsomniaEpisodeStatus;
mod live_processing;
mod native_import_checkpoints;
pub use live_processing::LiveExtractionOutput;
pub use maintenance_prune::{
    PrunedOwnerCopyReport, promote_terminal_memory_copy, prune_terminal_memory_copy,
};
pub use native_import_checkpoints::CompletedImportExtraction;
mod vector_publication;
pub use archive_vector_publication::{
    ArchiveVectorInput, ArchiveVectorRow, MAX_ARCHIVE_VECTOR_BATCH_ROWS,
    MAX_ARCHIVE_VECTOR_RESPONSE_BYTES,
};
pub use vector_publication::{MemoryVectorInput, MemoryVectorRow};
mod management;
mod management_association;
pub use management_association::MemoryAssociationCut;
#[path = "session/user_memory.rs"]
mod user_memory;
pub use user_memory::{UserMemoryCorrection, UserMemoryCorrectionResult};
mod phy_writer;
mod rel_session;
mod rel_writer;
pub use management::{
    ManagementRead, RelConversationSummary, RelImportedSource, RelSourceTurn, RelTranscriptTurn,
};

mod replay;
mod validation;
use replay::*;
use validation::*;
mod dream_empty;
mod dream_initial;
mod dream_initial_replay;
mod dream_population;
pub use dream_initial::InitialDreamSnapshot;
mod dream_group;
mod dream_no_link;
mod dream_supersession;
mod grouped_commit;

pub(crate) mod dream_batch_model;
pub use dream_batch_model::{DreamBatchCut, DreamLifecyclePublication, DreamPairPublication};

mod dream_batch;

mod dream_frontier;

mod dream_read_index;
pub use dream_read_index::DreamCandidatePool;

impl CommonState {
    fn refresh_dream_index(&mut self, ids: &[MemoryId]) -> Result<(), OwnerSessionError> {
        self.dream_read_index.refresh(
            &self.memories,
            &self.packed_vectors,
            &self.profiles,
            &self.memory_vectors,
            ids,
        )
    }
}

mod replay_prefix;

mod dream_prefix_effect;

mod entity_mention;
pub use entity_mention::EntityMentionCommit;

mod entity_mention_inputs;
pub use entity_mention_inputs::{EntityCandidateMemory, EntityMentionInputs};
