use crate::records::external_index::{ExternalIndexStore, SourceState};
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
use reliquary_freshness_store::{FreshnessMemoryId, FreshnessRecord, FreshnessStore};
use reliquary_graph::{
    CommunitySnapshot, CommunityStore, GraphAdmission, GraphError, GraphStore, NodeRef, Relation,
};
use reliquary_insomnia_store::{
    AcceptedCompletionState, AcceptedEpisodeOrder, CompletionCheck, EpisodeId as WorkEpisodeId,
    InsomniaError, InsomniaLeaseToken, InsomniaPriority, InsomniaStore, InsomniaWork,
    MemoryId as WorkMemoryId,
};
use reliquary_memory_core::{
    MemoryDraft, MemoryError, MemoryHistory, MemoryId, MemoryRevision, MemoryStore,
};
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
    StaleDreamCut,
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

/// Read-only Phylactery owner aggregate. It has Memory, Graph, Entity, Ego,
/// retention/Dream evidence, and vector reads, with no Archive or REL activity API.
pub struct PhySession {
    owner_id: [u8; 16],
    common: CommonState,
}

/// Narrow PHY writer for user-owned Memories routed from an authorized REL.
/// The Gateway owns the grant check; this writer exposes no general Container
/// mutation or Archive capability.
pub struct PhyWriter {
    container: Container,
    owner_id: [u8; 16],
    common: CommonState,
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
            pub fn entity(&self, id: EntityId) -> Option<&Entity> {
                self.$field.perception.entity(id)
            }
            pub fn entity_revision(&self, id: EntityId, revision: u64) -> Option<&Entity> {
                self.$field.perception.entity_revision(id, revision)
            }
            pub fn entities(&self) -> Vec<Entity> {
                self.$field.perception.entities().cloned().collect()
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

#[cfg(test)]
mod tests;

mod archive_evidence;
mod create;
pub(crate) mod dream_pairs;
pub(crate) mod dream_prefix;
mod external_reference;
mod phy_writer;
mod rel_session;
mod rel_writer;

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
