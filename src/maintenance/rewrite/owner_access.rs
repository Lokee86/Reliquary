//! Operation-specific adapters over concrete lifecycle owners for rewriting.
use crate::archive_vector_store::validate_mapping_shape;
use crate::owner_lifecycle::{OwnerError, OwnerPhylacteryError, PhylacteryOwner, ReliquaryOwner};
use crate::vector_generation_validation::{validate_generation_reference, vector_generation_id};
use crate::{
    ArchiveVectorError, ArchiveVectorId, ArchiveVectorSet, CompatibilityProfile,
    CompatibilityProfileId, FragmentId, Memory, MemoryBodyId, MemoryError, MemoryId, MemoryStats,
    MemoryVectorError, MemoryVectorId, MemoryVectorLocation, MemoryVectorSet, PackedVectorError,
    PackedVectorId, PackedVectors, VectorGeneration, VectorGenerationError,
};
use crate::{FileKind, Fragment};
use std::collections::HashSet;

pub(crate) trait RewriteOwnerAccess {
    fn memory(&mut self, id: MemoryId) -> Result<Memory, MemoryError>;
    fn memory_revision(&mut self, id: MemoryId, revision: u64) -> Result<Memory, MemoryError>;
    fn memory_ids(&self) -> Vec<MemoryId>;
    fn memory_stats(&self) -> MemoryStats;
    fn tombstone_memory(
        &mut self,
        id: MemoryId,
        expected_revision: u64,
    ) -> Result<crate::owner_lifecycle::TombstoneOutcome, MemoryError>;
    fn put_memory_routing_metadata(
        &mut self,
        metadata: crate::MemoryRoutingMetadata,
    ) -> Result<bool, MemoryError>;
    fn sync(&self) -> Result<(), OwnerError>;
    fn compatibility_profiles(&self) -> Vec<CompatibilityProfile>;
    fn compatibility_profile_is_retired(&self, id: CompatibilityProfileId) -> bool;
    fn put_packed_vectors(
        &mut self,
        packed: PackedVectors,
    ) -> Result<PackedVectorId, PackedVectorError>;
    fn packed_vectors(&mut self, id: PackedVectorId) -> Result<PackedVectors, PackedVectorError>;
    fn put_memory_vectors(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        packed_vector_id: PackedVectorId,
        memory_body_ids: Vec<MemoryBodyId>,
    ) -> Result<MemoryVectorId, MemoryVectorError>;
    fn memory_vectors(&mut self, id: MemoryVectorId) -> Result<MemoryVectorSet, MemoryVectorError>;
    fn memory_vector_location(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
        memory_body_id: MemoryBodyId,
    ) -> Option<MemoryVectorLocation>;
    fn history_generation(&self) -> crate::HistoryGenerationId;
}
impl RewriteOwnerAccess for ReliquaryOwner {
    fn memory(&mut self, id: MemoryId) -> Result<Memory, MemoryError> {
        self.memories.memory(&mut self.container, id)
    }
    fn memory_revision(&mut self, id: MemoryId, revision: u64) -> Result<Memory, MemoryError> {
        self.memories
            .memory_revision(&mut self.container, id, revision)
    }
    fn memory_ids(&self) -> Vec<MemoryId> {
        self.memories.current_ids()
    }
    fn memory_stats(&self) -> MemoryStats {
        self.memories.stats()
    }
    fn tombstone_memory(
        &mut self,
        id: MemoryId,
        expected_revision: u64,
    ) -> Result<crate::owner_lifecycle::TombstoneOutcome, MemoryError> {
        let result = self
            .memories
            .tombstone(&mut self.container, id, expected_revision)?;
        self.graph
            .exclude_from_current(crate::SemanticNodeRef::memory(id))
            .map_err(|_| MemoryError::InvalidField("Memory graph projection"))?;
        Ok(result)
    }
    fn put_memory_routing_metadata(
        &mut self,
        metadata: crate::MemoryRoutingMetadata,
    ) -> Result<bool, MemoryError> {
        self.memories
            .put_routing_metadata(&mut self.container, metadata)
    }
    fn sync(&self) -> Result<(), OwnerError> {
        self.container.sync()?;
        Ok(())
    }
    fn compatibility_profiles(&self) -> Vec<CompatibilityProfile> {
        self.compatibility_profiles.profiles()
    }
    fn compatibility_profile_is_retired(&self, id: CompatibilityProfileId) -> bool {
        self.compatibility_profiles.is_retired(id)
    }
    fn put_packed_vectors(
        &mut self,
        packed: PackedVectors,
    ) -> Result<PackedVectorId, PackedVectorError> {
        self.packed_vectors.put(&mut self.container, packed)
    }
    fn packed_vectors(&mut self, id: PackedVectorId) -> Result<PackedVectors, PackedVectorError> {
        self.packed_vectors.get(&mut self.container, id)
    }
    fn put_memory_vectors(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        packed_vector_id: PackedVectorId,
        memory_body_ids: Vec<MemoryBodyId>,
    ) -> Result<MemoryVectorId, MemoryVectorError> {
        self.compatibility_profiles
            .get_active(compatibility_profile_id)
            .ok_or(MemoryVectorError::MissingProfile)?;
        crate::memory_vector_store::validate_mapping_shape(
            &self.compatibility_profiles,
            &self.packed_vectors,
            compatibility_profile_id,
            packed_vector_id,
            &memory_body_ids,
        )?;
        let mut seen = std::collections::HashSet::new();
        for body in &memory_body_ids {
            if !self.memories.contains_body(*body) {
                return Err(MemoryVectorError::MissingMemoryBody);
            }
            if !seen.insert(*body) {
                return Err(MemoryVectorError::DuplicateMemoryBody);
            }
        }
        self.memory_vectors.put(
            &mut self.container,
            &self.compatibility_profiles,
            &self.packed_vectors,
            compatibility_profile_id,
            packed_vector_id,
            memory_body_ids,
        )
    }
    fn memory_vectors(&mut self, id: MemoryVectorId) -> Result<MemoryVectorSet, MemoryVectorError> {
        self.memory_vectors.get(&mut self.container, id)
    }
    fn memory_vector_location(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
        memory_body_id: MemoryBodyId,
    ) -> Option<MemoryVectorLocation> {
        self.memory_vectors
            .location(compatibility_profile_id, memory_body_id)
    }
    fn history_generation(&self) -> crate::HistoryGenerationId {
        self.rewrite_lineage.generation()
    }
}
impl RewriteOwnerAccess for PhylacteryOwner {
    fn memory(&mut self, id: MemoryId) -> Result<Memory, MemoryError> {
        self.memories.memory(&mut self.container, id)
    }
    fn memory_revision(&mut self, id: MemoryId, revision: u64) -> Result<Memory, MemoryError> {
        self.memories
            .memory_revision(&mut self.container, id, revision)
    }
    fn memory_ids(&self) -> Vec<MemoryId> {
        self.memories.current_ids()
    }
    fn memory_stats(&self) -> MemoryStats {
        self.memories.stats()
    }
    fn tombstone_memory(
        &mut self,
        id: MemoryId,
        expected_revision: u64,
    ) -> Result<crate::owner_lifecycle::TombstoneOutcome, MemoryError> {
        let result = self
            .memories
            .tombstone(&mut self.container, id, expected_revision)?;
        self.graph
            .exclude_from_current(crate::SemanticNodeRef::memory(id))
            .map_err(|_| MemoryError::InvalidField("Memory graph projection"))?;
        Ok(result)
    }
    fn put_memory_routing_metadata(
        &mut self,
        metadata: crate::MemoryRoutingMetadata,
    ) -> Result<bool, MemoryError> {
        self.memories
            .put_routing_metadata(&mut self.container, metadata)
    }
    fn sync(&self) -> Result<(), OwnerError> {
        self.container.sync()?;
        Ok(())
    }
    fn compatibility_profiles(&self) -> Vec<CompatibilityProfile> {
        self.compatibility_profiles.profiles()
    }
    fn compatibility_profile_is_retired(&self, id: CompatibilityProfileId) -> bool {
        self.compatibility_profiles.is_retired(id)
    }
    fn put_packed_vectors(
        &mut self,
        packed: PackedVectors,
    ) -> Result<PackedVectorId, PackedVectorError> {
        self.packed_vectors.put(&mut self.container, packed)
    }
    fn packed_vectors(&mut self, id: PackedVectorId) -> Result<PackedVectors, PackedVectorError> {
        self.packed_vectors.get(&mut self.container, id)
    }
    fn put_memory_vectors(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        packed_vector_id: PackedVectorId,
        memory_body_ids: Vec<MemoryBodyId>,
    ) -> Result<MemoryVectorId, MemoryVectorError> {
        self.compatibility_profiles
            .get_active(compatibility_profile_id)
            .ok_or(MemoryVectorError::MissingProfile)?;
        crate::memory_vector_store::validate_mapping_shape(
            &self.compatibility_profiles,
            &self.packed_vectors,
            compatibility_profile_id,
            packed_vector_id,
            &memory_body_ids,
        )?;
        let mut seen = std::collections::HashSet::new();
        for body in &memory_body_ids {
            if !self.memories.contains_body(*body) {
                return Err(MemoryVectorError::MissingMemoryBody);
            }
            if !seen.insert(*body) {
                return Err(MemoryVectorError::DuplicateMemoryBody);
            }
        }
        self.memory_vectors.put(
            &mut self.container,
            &self.compatibility_profiles,
            &self.packed_vectors,
            compatibility_profile_id,
            packed_vector_id,
            memory_body_ids,
        )
    }
    fn memory_vectors(&mut self, id: MemoryVectorId) -> Result<MemoryVectorSet, MemoryVectorError> {
        self.memory_vectors.get(&mut self.container, id)
    }
    fn memory_vector_location(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
        memory_body_id: MemoryBodyId,
    ) -> Option<MemoryVectorLocation> {
        self.memory_vectors
            .location(compatibility_profile_id, memory_body_id)
    }
    fn history_generation(&self) -> crate::HistoryGenerationId {
        self.rewrite_lineage.generation()
    }
}
pub(crate) trait ReliquaryRewriteAccess {
    fn archive_version(&self) -> u64;
    fn rel_metadata(&self) -> crate::RelMetadata;
    fn set_rel_metadata(
        &mut self,
        type_label: Option<String>,
        dependencies: Vec<String>,
    ) -> Result<bool, OwnerError>;
    fn fragments(&self) -> Vec<Fragment>;
    fn freshness_policy(&self) -> crate::freshness::FreshnessPolicy;
    fn put_archive_vectors(
        &mut self,
        packed_vector_id: PackedVectorId,
        fragment_ids: Vec<FragmentId>,
    ) -> Result<ArchiveVectorId, ArchiveVectorError>;
    fn archive_vectors(
        &mut self,
        id: ArchiveVectorId,
    ) -> Result<ArchiveVectorSet, ArchiveVectorError>;
    fn current_vector_generation(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
    ) -> Option<VectorGeneration>;
    fn has_complete_current_coverage(&self, generation: &VectorGeneration) -> bool;
    fn rel_turn_count(&self) -> u64;
    fn activity_position_for_turn(&self, conversation_id: &str, node_id: &str) -> Option<u64>;
    fn publish_vector_generation_without_live_semantic(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        archive_vector_id: crate::ArchiveVectorId,
        source_archive_version: u64,
    ) -> Result<VectorGeneration, VectorGenerationError>;
    fn enroll_legacy_freshness_baseline(&mut self, id: MemoryId) -> Result<bool, OwnerError>;
}
impl ReliquaryRewriteAccess for ReliquaryOwner {
    fn archive_version(&self) -> u64 {
        self.archive.archive_version()
    }
    fn rel_metadata(&self) -> crate::RelMetadata {
        if let Some(metadata) = self.rel_metadata.current() {
            return metadata.clone();
        }
        crate::RelMetadata {
            type_label: self
                .container
                .identity()
                .and_then(|identity| identity.scope)
                .map(|scope| match scope {
                    crate::ReliquaryScopeKind::Organization => "Organization".to_owned(),
                    crate::ReliquaryScopeKind::Project => "Project".to_owned(),
                    crate::ReliquaryScopeKind::Connection => "Connection".to_owned(),
                }),
            dependencies: Vec::new(),
        }
    }
    fn set_rel_metadata(
        &mut self,
        type_label: Option<String>,
        mut dependencies: Vec<String>,
    ) -> Result<bool, OwnerError> {
        let type_label = type_label.and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        });
        dependencies = dependencies
            .into_iter()
            .map(|value| value.trim().to_owned())
            .collect();
        dependencies.sort();
        dependencies.dedup();
        if let Some(owner_id) = self.container.owner_id()
            && dependencies
                .iter()
                .any(|dependency| dependency == &owner_id)
        {
            return Err(OwnerError::RelMetadata(
                "a REL cannot depend on itself".into(),
            ));
        }
        let metadata = crate::RelMetadata {
            type_label,
            dependencies,
        };
        self.rel_metadata
            .put(&mut self.container, metadata)
            .map_err(OwnerError::RelMetadata)
    }
    fn fragments(&self) -> Vec<Fragment> {
        self.archive.fragments()
    }
    fn freshness_policy(&self) -> crate::freshness::FreshnessPolicy {
        self.freshness.policy()
    }
    fn put_archive_vectors(
        &mut self,
        packed_vector_id: PackedVectorId,
        fragment_ids: Vec<FragmentId>,
    ) -> Result<ArchiveVectorId, ArchiveVectorError> {
        validate_mapping_shape(&self.packed_vectors, packed_vector_id, &fragment_ids)?;
        let mut seen = HashSet::with_capacity(fragment_ids.len());
        let mut max_fragment_archive_version = 0;
        for fragment_id in &fragment_ids {
            let version = self
                .archive
                .fragment_archive_version(*fragment_id)
                .ok_or(ArchiveVectorError::MissingFragment)?;
            if !seen.insert(*fragment_id) {
                return Err(ArchiveVectorError::DuplicateFragment);
            }
            max_fragment_archive_version = max_fragment_archive_version.max(version);
        }
        self.archive_vectors.put(
            &mut self.container,
            &self.packed_vectors,
            max_fragment_archive_version,
            packed_vector_id,
            fragment_ids,
        )
    }
    fn archive_vectors(
        &mut self,
        id: ArchiveVectorId,
    ) -> Result<ArchiveVectorSet, ArchiveVectorError> {
        self.archive_vectors.get(&mut self.container, id)
    }
    fn current_vector_generation(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
    ) -> Option<VectorGeneration> {
        if self.owner_uuid().is_none() || self.import_epochs.open().is_some() {
            return None;
        }
        self.compatibility_profiles
            .get_active(compatibility_profile_id)?;
        let generation = self
            .vector_generations
            .current_generation(compatibility_profile_id)?;
        self.has_complete_current_coverage(&generation)
            .then_some(generation)
    }
    fn has_complete_current_coverage(&self, generation: &VectorGeneration) -> bool {
        generation.source_archive_version == self.archive.archive_version()
            && self.archive_vectors.has_exact_fragment_coverage(
                generation.archive_vector_id,
                &self
                    .archive
                    .fragments()
                    .iter()
                    .map(|fragment| fragment.id)
                    .collect::<Vec<_>>(),
            )
    }
    fn rel_turn_count(&self) -> u64 {
        self.archive.rel_turn_count()
    }
    fn activity_position_for_turn(&self, conversation_id: &str, node_id: &str) -> Option<u64> {
        self.archive
            .activity_position_for_turn(conversation_id, node_id)
    }
    fn publish_vector_generation_without_live_semantic(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        archive_vector_id: crate::ArchiveVectorId,
        source_archive_version: u64,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        self.compatibility_profiles
            .get_active(compatibility_profile_id)
            .ok_or(VectorGenerationError::MissingProfile)?;
        let candidate = VectorGeneration {
            id: vector_generation_id(
                compatibility_profile_id,
                archive_vector_id,
                source_archive_version,
            ),
            compatibility_profile_id,
            archive_vector_id,
            source_archive_version,
            global_version: 0,
            vector_version: 0,
        };
        if candidate.source_archive_version == 0
            || candidate.source_archive_version > self.archive.archive_version()
        {
            return Err(VectorGenerationError::SourceArchiveVersion);
        }
        validate_generation_reference(
            self.archive.archive_version(),
            &self.packed_vectors,
            &self.archive_vectors,
            &self.compatibility_profiles,
            &candidate,
        )?;
        let generation = self.vector_generations.publish(
            &mut self.container,
            compatibility_profile_id,
            archive_vector_id,
            source_archive_version,
        )?;
        Ok(generation)
    }
    fn enroll_legacy_freshness_baseline(&mut self, id: MemoryId) -> Result<bool, OwnerError> {
        if !self.memories.contains_memory(id) {
            return Err(OwnerError::Freshness(format!(
                "unknown owner-local Memory {:?}",
                id
            )));
        }
        if self.dream_freshness.is_publication_birth(id)
            || self.dream_freshness.pending_initialization(id).is_some()
        {
            return Err(OwnerError::Freshness(
                "a proven first publication cannot be re-enrolled as a legacy baseline".into(),
            ));
        }
        if self.freshness.records().contains_key(&id) {
            return Ok(false);
        }
        let memory = self.memory(id)?;
        if memory.archived || !matches!(memory.lifecycle_state.as_str(), "knowledge" | "canonical")
        {
            return Err(OwnerError::Freshness(
                "legacy baseline requires a nonarchived settled Memory".into(),
            ));
        }
        let owner = self.owner_uuid().ok_or_else(|| {
            OwnerError::Freshness("legacy owner requires explicit owner migration first".into())
        })?;
        let turn = self.archive.rel_turn_count();
        let mut record = crate::FreshnessRecord::created();
        record
            .admit(turn)
            .map_err(|error| OwnerError::Freshness(error.to_string()))?;
        self.freshness
            .persist_policy_if_needed(&mut self.container, owner)
            .map_err(OwnerError::Freshness)?;
        let payload = self.freshness.encode_initialize(owner, id, turn, record);
        self.container.append(&payload)?;
        self.container.sync()?;
        self.freshness
            .ingest_appended(&payload)
            .map_err(OwnerError::Freshness)?;
        crate::FreshnessDueDispatcher::default()
            .refresh_with_transition(id, &record, turn, &self.freshness.policy())
            .map_err(|error| OwnerError::Freshness(error.to_string()))?;
        Ok(true)
    }
}
pub(crate) trait PhylacteryRewriteAccess {
    fn profile(&self) -> crate::PhylacteryProfile;
    fn set_profile(&mut self, display_name: Option<String>) -> Result<bool, OwnerPhylacteryError>;
}
impl PhylacteryRewriteAccess for PhylacteryOwner {
    fn profile(&self) -> crate::PhylacteryProfile {
        self.profile.current()
    }
    fn set_profile(&mut self, display_name: Option<String>) -> Result<bool, OwnerPhylacteryError> {
        let normalize = |value: Option<String>| {
            value.and_then(|value| {
                let trimmed = value.trim().to_owned();
                (!trimmed.is_empty()).then_some(trimmed)
            })
        };
        self.profile
            .put(
                &mut self.container,
                crate::PhylacteryProfile {
                    display_name: normalize(display_name),
                },
            )
            .map_err(OwnerPhylacteryError::Profile)
    }
}

pub(crate) fn open_rel(container: crate::Container) -> Result<ReliquaryOwner, OwnerError> {
    super::super::reliquary_prune::reopen_reliquary(container).map(|(owner, _due)| owner)
}
pub(crate) fn open_phy(
    container: crate::Container,
) -> Result<PhylacteryOwner, OwnerPhylacteryError> {
    super::super::phylactery_copy::reopen(container)
}
pub(crate) fn create_rel(
    path: &std::path::Path,
    owner_uuid: [u8; 16],
    type_label: Option<String>,
) -> Result<ReliquaryOwner, OwnerError> {
    let container = crate::Container::create_with_identity_and_uuid(
        path,
        crate::ContainerIdentity {
            file_kind: FileKind::Reliquary,
            scope: None,
        },
        owner_uuid,
    )?;
    let mut owner = crate::owner_lifecycle::initialize_reliquary(container)?;
    if type_label.is_some() {
        owner.set_rel_metadata(type_label, Vec::new())?;
    }
    Ok(owner)
}
pub(crate) fn create_phy(
    path: &std::path::Path,
    owner_uuid: [u8; 16],
) -> Result<PhylacteryOwner, OwnerPhylacteryError> {
    let container = crate::Container::create_with_identity_and_uuid(
        path,
        crate::ContainerIdentity {
            file_kind: FileKind::Phylactery,
            scope: None,
        },
        owner_uuid,
    )?;
    crate::owner_lifecycle::initialize_phylactery(container)
}
pub(crate) fn plan_rel(
    owner: &ReliquaryOwner,
    request: &super::model::RewriteRequest,
) -> Result<super::model::RewritePlan, super::RewriteError> {
    super::plan_source::ReliquaryRewritePlanSource {
        container: &owner.container,
        memories: &owner.memories,
        compatibility_profiles: &owner.compatibility_profiles,
        memory_vectors: &owner.memory_vectors,
        rewrite_lineage: &owner.rewrite_lineage,
        entities: &owner.entities,
        relationships: &owner.relationships,
        graph: &owner.graph,
        archive: &owner.archive,
        freshness: &owner.freshness,
    }
    .plan_rewrite(request)
}
pub(crate) fn semantic_closure_rel(
    owner: &ReliquaryOwner,
    retained: &std::collections::BTreeSet<crate::MemoryId>,
) -> Result<super::closure::SemanticClosure, super::RewriteError> {
    super::plan_source::ReliquaryRewritePlanSource {
        container: &owner.container,
        memories: &owner.memories,
        compatibility_profiles: &owner.compatibility_profiles,
        memory_vectors: &owner.memory_vectors,
        rewrite_lineage: &owner.rewrite_lineage,
        entities: &owner.entities,
        relationships: &owner.relationships,
        graph: &owner.graph,
        archive: &owner.archive,
        freshness: &owner.freshness,
    }
    .semantic_closure(retained)
}
pub(crate) fn copy_semantic_state_rel(
    source: &ReliquaryOwner,
    destination: &mut ReliquaryOwner,
    closure: &super::closure::SemanticClosure,
    include_synthesis: bool,
) -> Result<(), super::RewriteError> {
    let source_id = source.container.owner_id();
    let mut output = super::semantic_emit::SemanticRewriteDestination {
        container: &mut destination.container,
        memories: &destination.memories,
        entities: &mut destination.entities,
        relationships: &mut destination.relationships,
        graph: &mut destination.graph,
        ego: &mut destination.ego,
    };
    super::semantic_emit::copy_semantic_state_rel(
        source_id.as_deref(),
        &source.ego,
        &mut output,
        closure,
        include_synthesis,
    )
}
pub(crate) fn plan_phy(
    owner: &PhylacteryOwner,
    request: &super::model::RewriteRequest,
) -> Result<super::model::RewritePlan, super::RewriteError> {
    super::plan_source::PhylacteryRewritePlanSource {
        container: &owner.container,
        memories: &owner.memories,
        compatibility_profiles: &owner.compatibility_profiles,
        memory_vectors: &owner.memory_vectors,
        rewrite_lineage: &owner.rewrite_lineage,
        entities: &owner.entities,
        relationships: &owner.relationships,
        graph: &owner.graph,
    }
    .plan_rewrite(request)
}
pub(crate) fn semantic_closure_phy(
    owner: &PhylacteryOwner,
    retained: &std::collections::BTreeSet<crate::MemoryId>,
) -> Result<super::closure::SemanticClosure, super::RewriteError> {
    super::plan_source::PhylacteryRewritePlanSource {
        container: &owner.container,
        memories: &owner.memories,
        compatibility_profiles: &owner.compatibility_profiles,
        memory_vectors: &owner.memory_vectors,
        rewrite_lineage: &owner.rewrite_lineage,
        entities: &owner.entities,
        relationships: &owner.relationships,
        graph: &owner.graph,
    }
    .semantic_closure(retained)
}
pub(crate) fn copy_semantic_state_phy(
    source: &PhylacteryOwner,
    destination: &mut PhylacteryOwner,
    closure: &super::closure::SemanticClosure,
) -> Result<(), super::RewriteError> {
    let source_id = source.container.owner_id();
    let mut output = super::semantic_emit::SemanticRewriteDestination {
        container: &mut destination.container,
        memories: &destination.memories,
        entities: &mut destination.entities,
        relationships: &mut destination.relationships,
        graph: &mut destination.graph,
        ego: &mut destination.ego,
    };
    super::semantic_emit::copy_semantic_state_phy(
        source_id.as_deref(),
        &source.ego,
        &mut output,
        closure,
    )
}
