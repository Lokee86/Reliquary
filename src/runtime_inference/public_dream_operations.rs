//! Canonical Dream invocation and accepted owner-prefix access.
impl crate::RelExecutionOwner {
    pub(crate) fn native_dream_population_at_prefix(
        &mut self,
        end: u64,
        cut: u64,
        source: crate::MemoryId,
    ) -> Result<std::collections::BTreeSet<crate::MemoryId>, crate::CvaError> {
        self.owner.dream_population_at_prefix(end, cut, source)
    }
}

use crate::dream_owner_candidates::{
    finish_dream_candidates, finish_dream_memory_context, prepare_dream_candidates,
    prepare_dream_memory_context,
};
use crate::dream_publication_execution::{
    DreamSourceTime, publish_dream_pair as publish_from_parts,
};
use crate::dream_temporal_policy::analyze_memory_temporal;
use crate::memory_source_time::memory_source_timestamp_ns;
use crate::memory_source_time_admission::reliquary_memory_source_timestamp_ns;
use crate::{
    CompatibilityProfileId, DreamCandidateConfig, DreamCandidateError, DreamCandidateSet,
    DreamMemoryContext, DreamPairClassification, DreamPairVerification, DreamPublicationError,
    DreamPublicationOutcome, DreamTemporalAnalysis, DreamVerificationPolicy, Memory, MemoryError,
    MemoryId,
};

impl crate::RelExecutionOwner {
    pub(crate) fn native_dream_memory_context(
        &mut self,
        id: MemoryId,
    ) -> Result<DreamMemoryContext, DreamCandidateError> {
        let facts = self.owner.dream_operation_facts();
        let work = prepare_dream_memory_context(&facts.memories, facts.graph, id)?;
        let source_time = reliquary_memory_source_timestamp_ns(&self.archive, &work.memory);
        Ok(finish_dream_memory_context(work, source_time))
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_dream_candidates(
        &mut self,
        profile: CompatibilityProfileId,
        id: MemoryId,
        config: DreamCandidateConfig,
    ) -> Result<DreamCandidateSet, DreamCandidateError> {
        let facts = self.owner.dream_operation_facts();
        let work = prepare_dream_candidates(
            &facts.memories,
            facts.graph,
            facts.pairs,
            &crate::memory_vector_reader::MemoryVectorReader::new(facts.vectors),
            profile,
            id,
            config,
        )?;
        let source_times = work
            .source_time_memories()
            .map(|memory| {
                (
                    memory.id,
                    reliquary_memory_source_timestamp_ns(&self.archive, memory),
                )
            })
            .collect();
        Ok(finish_dream_candidates(work, &source_times))
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_dream_temporal_analysis(
        &mut self,
        id: MemoryId,
    ) -> Result<DreamTemporalAnalysis, MemoryError> {
        let memory = self.owner.memories.memory(&mut self.owner.container, id)?;
        let body = self.memories.current_body_id(id)?;
        Ok(analyze_memory_temporal(
            &memory,
            body,
            reliquary_memory_source_timestamp_ns(&self.archive, &memory),
        ))
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_publish_dream_pair(
        &mut self,
        classification: &DreamPairClassification,
        verification: Option<&DreamPairVerification>,
        policy: DreamVerificationPolicy,
        expected_graph_version: u64,
    ) -> Result<DreamPublicationOutcome, DreamPublicationError> {
        let archive = &self.owner.archive;
        publish_from_parts(
            crate::owner_lifecycle::DreamGraphPublication::new(
                &mut self.owner.container,
                &self.owner.memories,
                &mut self.owner.graph,
            ),
            &mut self.duplicate_index,
            classification,
            verification,
            policy,
            expected_graph_version,
            DreamSourceTime::Reliquary(archive),
        )
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_dream_was_processed(&self, id: MemoryId) -> bool {
        self.owner
            .dream_processing_state_native(id)
            .is_some_and(|state| state.checkpoint_at_ns.is_some())
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_dream_eligible_epoch(
        &mut self,
        id: MemoryId,
        now_ns: i64,
    ) -> Result<Option<u64>, MemoryError> {
        let memory = self.memory(id)?;
        crate::dream_cooldown::eligible_dream_epoch(
            &memory,
            memory_source_timestamp_ns(&memory),
            self.owner.dream_processing_state_native(id),
            now_ns,
        )
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_mark_dream_processed(
        &mut self,
        id: MemoryId,
        epoch: u64,
        processed_at_ns: i64,
    ) -> Result<bool, MemoryError> {
        self.owner.mark_dream_processed_native(
            id,
            crate::dream_cooldown::dream_processing_state(epoch, processed_at_ns),
        )
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_dream_was_processed(&self, id: MemoryId) -> bool {
        self.owner
            .dream_processing_state_native(id)
            .is_some_and(|state| state.checkpoint_at_ns.is_some())
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_dream_memory_context(
        &mut self,
        memory_id: MemoryId,
    ) -> Result<DreamMemoryContext, DreamCandidateError> {
        let facts = self.owner.dream_operation_facts();
        let work = prepare_dream_memory_context(&facts.memories, facts.graph, memory_id)?;
        let source_time = memory_source_timestamp_ns(&work.memory);
        Ok(finish_dream_memory_context(work, source_time))
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_dream_candidates(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        source_id: MemoryId,
        config: DreamCandidateConfig,
    ) -> Result<DreamCandidateSet, DreamCandidateError> {
        let facts = self.owner.dream_operation_facts();
        let work = prepare_dream_candidates(
            &facts.memories,
            facts.graph,
            facts.pairs,
            &crate::memory_vector_reader::MemoryVectorReader::new(facts.vectors),
            compatibility_profile_id,
            source_id,
            config,
        )?;
        let source_times = work
            .source_time_memories()
            .map(|memory| (memory.id, memory_source_timestamp_ns(memory)))
            .collect();
        Ok(finish_dream_candidates(work, &source_times))
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_dream_temporal_analysis(
        &mut self,
        memory_id: MemoryId,
    ) -> Result<DreamTemporalAnalysis, MemoryError> {
        let memory = self
            .owner
            .memories
            .memory(&mut self.owner.container, memory_id)?;
        let body_id = self.memories.current_body_id(memory_id)?;
        Ok(analyze_memory_temporal(
            &memory,
            body_id,
            memory_source_timestamp_ns(&memory),
        ))
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_mark_dream_pair_evaluated(
        &mut self,
        left: MemoryId,
        right: MemoryId,
    ) -> Result<bool, MemoryError> {
        self.owner.mark_dream_pair_native(left, right)
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_dream_pair_records(&self) -> Vec<(MemoryId, MemoryId)> {
        self.owner.dream_pair_records_native()
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_mark_dream_pair_evaluated(
        &mut self,
        left: MemoryId,
        right: MemoryId,
    ) -> Result<bool, MemoryError> {
        self.owner.mark_dream_pair_native(left, right)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_dream_pair_records(&self) -> Vec<(MemoryId, MemoryId)> {
        self.owner.dream_pair_records_native()
    }
}

use crate::dream_canonical::{
    DreamOwnerFacts, canonical_superseders, corroborated_representative,
    duplicate_component_has_active_peer, explicit_authority_promotes,
};
use crate::memory_store::MemoryReadView;
use crate::{
    DreamLifecycleError, DreamLifecycleResult, GraphRelation, GraphRelationKind, MemoryDraft,
};
use std::collections::{HashMap, HashSet};
pub(crate) fn native_dream_owner_facts(
    cva: &crate::RelExecutionOwner,
    ids: impl IntoIterator<Item = MemoryId>,
    cohort_source: Option<MemoryId>,
) -> DreamOwnerFacts {
    let ids: HashSet<_> = ids.into_iter().collect();
    let births: HashSet<_> = ids
        .iter()
        .copied()
        .filter(|id| cva.owner.dream_is_publication_birth(*id))
        .collect();
    let mut anchors = HashMap::new();
    let mut representative_keys = HashMap::new();
    let memories = MemoryReadView::new(&cva.memories, &cva.container);
    for id in &ids {
        if let Ok(memory) = memories.memory(*id) {
            let anchor = memory.source_node_id.clone().and_then(|node| {
                memory
                    .source_episode_id
                    .and_then(|episode| cva.archive.episode(episode))
                    .map(|episode| (episode.conversation_id.clone(), node))
            });
            anchors.insert(*id, anchor);
            representative_keys.insert(
                *id,
                crate::history::dream_representative::representative_key(&cva.archive, &memory),
            );
        }
    }
    let mut cohorts = HashMap::new();
    if let Some(source_id) = cohort_source.filter(|_| !births.is_empty()) {
        let history = crate::owner_lifecycle::HistorySource::new(&cva.container)
            .map_err(|error| crate::CvaError::Container(error.0).to_string());
        for id in &births {
            let result = match &history {
                Ok(history) => {
                    crate::history::same_publication_birth_cohort(history, source_id, *id)
                        .map_err(|error| error.to_string())
                }
                Err(error) => Err(error.clone()),
            };
            cohorts.insert((source_id, *id), result);
        }
    }
    DreamOwnerFacts {
        publication_births: births,
        cohorts,
        anchors,
        representative_keys,
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_reconcile_dream_lifecycle(
        &mut self,
        source_id: MemoryId,
    ) -> Result<DreamLifecycleResult, DreamLifecycleError> {
        let relations = self.runtime_graph_relations();
        let mut revised = Vec::new();
        let mut archived = Vec::new();
        let mut canonicalized = {
            let parts = MemoryReadView::new(&self.memories, &self.container);
            canonical_superseders(&parts, source_id, &relations)?
        };

        let mut superseded_targets: Vec<_> = relations
            .iter()
            .filter(|relation| {
                relation.kind == GraphRelationKind::Supersedes
                    && (relation.source == source_id || relation.target == source_id)
            })
            .map(|relation| relation.target)
            .collect();
        superseded_targets.sort_by_key(|id| id.0);
        superseded_targets.dedup();

        for target in superseded_targets {
            let superseded_by = unique_superseder(&relations, target);
            if let Some(memory) =
                self.native_revise_dream_lifecycle(target, "archived", true, superseded_by)?
            {
                archived.push(target);
                revised.push(memory);
            }
        }

        let source = self.runtime_memory(source_id)?;
        if !source.archived
            && source.lifecycle_state == "extracted"
            && {
                let parts = MemoryReadView::new(&self.memories, &self.container);
                duplicate_component_has_active_peer(&parts, source_id, &relations)?
            }
            && let Some(memory) = self.native_revise_dream_lifecycle(
                source_id,
                "archived",
                true,
                source.superseded_by,
            )?
        {
            archived.push(source_id);
            revised.push(memory);
        }

        let source = self.runtime_memory(source_id)?;
        if !source.archived && explicit_authority_promotes(&source) {
            canonicalized.push(source_id);
        }
        if let Some(representative) = {
            let parts = MemoryReadView::new(&self.memories, &self.container);
            let facts = native_dream_owner_facts(
                self,
                crate::graph_relation_sets::duplicate_component(source_id, &relations),
                None,
            );
            corroborated_representative(&parts, &facts, source_id, &relations)?
        } {
            canonicalized.push(representative);
        }
        canonicalized.sort_by_key(|id| id.0);
        canonicalized.dedup();

        let mut promoted = Vec::new();
        for id in canonicalized {
            let current = self.runtime_memory(id)?;
            if current.archived || current.lifecycle_state == "canonical" {
                continue;
            }
            if let Some(memory) =
                self.native_revise_dream_lifecycle(id, "canonical", false, current.superseded_by)?
            {
                revised.push(memory);
                promoted.push(id);
            }
        }

        let source = self.runtime_memory(source_id)?;
        let promoted_to_knowledge = if !source.archived && source.lifecycle_state == "extracted" {
            if let Some(memory) = self.native_revise_dream_lifecycle(
                source_id,
                "knowledge",
                false,
                source.superseded_by,
            )? {
                revised.push(memory);
                true
            } else {
                false
            }
        } else {
            false
        };

        archived.sort_by_key(|id| id.0);
        archived.dedup();
        promoted.sort_by_key(|id| id.0);
        Ok(DreamLifecycleResult {
            source: self.runtime_memory(source_id)?,
            revised,
            archived,
            canonicalized: promoted,
            promoted_to_knowledge,
        })
    }

    fn native_revise_dream_lifecycle(
        &mut self,
        id: MemoryId,
        lifecycle_state: &str,
        archived: bool,
        superseded_by: Option<MemoryId>,
    ) -> Result<Option<Memory>, DreamLifecycleError> {
        let current = self.runtime_memory(id)?;
        if current.lifecycle_state == lifecycle_state
            && current.archived == archived
            && current.superseded_by == superseded_by
        {
            return Ok(None);
        }
        let graph_version = self.runtime_graph_version();
        let draft = lifecycle_draft(
            &current,
            lifecycle_state,
            archived,
            superseded_by,
            graph_version,
        );
        let (memory, changed) =
            self.runtime_publish_knowledge_memory(Some(id), current.revision, draft)?;
        Ok(changed.then_some(memory))
    }
}

fn unique_superseder(relations: &[GraphRelation], target: MemoryId) -> Option<MemoryId> {
    let mut sources: Vec<_> = relations
        .iter()
        .filter(|relation| {
            relation.kind == GraphRelationKind::Supersedes && relation.target == target
        })
        .map(|relation| relation.source)
        .collect();
    sources.sort_by_key(|id| id.0);
    sources.dedup();
    match sources.as_slice() {
        [source] => Some(*source),
        _ => None,
    }
}

fn lifecycle_draft(
    memory: &Memory,
    lifecycle_state: &str,
    archived: bool,
    superseded_by: Option<MemoryId>,
    graph_version: u64,
) -> MemoryDraft {
    MemoryDraft {
        category: memory.category.clone(),
        memory_type: memory.memory_type.clone(),
        authority_kind: memory.authority_kind.clone(),
        temporal_status: memory.temporal_status.clone(),
        title: memory.title.clone(),
        content: memory.content.clone(),
        scope: memory.scope.clone(),
        lifecycle_state: lifecycle_state.into(),
        archived,
        superseded_by,
        parent_id: memory.parent_id,
        source_node_id: memory.source_node_id.clone(),
        content_source_conversation_id: memory.content_source_conversation_id.clone(),
        content_source_node_id: memory.content_source_node_id.clone(),
        grounding_source_conversation_id: memory.grounding_source_conversation_id.clone(),
        grounding_source_node_id: memory.grounding_source_node_id.clone(),
        source_episode_id: memory.source_episode_id,
        source_time_ns: memory.source_time_ns,
        mutation_id: lifecycle_mutation_id(
            memory.id,
            lifecycle_state,
            superseded_by,
            graph_version,
        ),
        created_at_ns: memory.created_at_ns,
        updated_at_ns: memory.updated_at_ns.saturating_add(1),
    }
}

fn lifecycle_mutation_id(
    id: MemoryId,
    lifecycle_state: &str,
    superseded_by: Option<MemoryId>,
    graph_version: u64,
) -> String {
    let id = bytes_hex(&id.0);
    let superseder = superseded_by
        .map(|value| bytes_hex(&value.0))
        .unwrap_or_else(|| "none".into());
    format!("dream-lifecycle:{id}:{graph_version}:{lifecycle_state}:{superseder}")
}

fn bytes_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_reconcile_dream_lifecycle(
        &mut self,
        source_id: MemoryId,
    ) -> Result<DreamLifecycleResult, DreamLifecycleError> {
        let (mut memories, graph) = self.owner.dream_lifecycle_write_parts();
        crate::dream_owner_publisher::reconcile_phy_lifecycle(&mut memories, graph, source_id)
    }

    pub(crate) fn native_publish_dream_pair(
        &mut self,
        classification: &DreamPairClassification,
        verification: Option<&DreamPairVerification>,
        policy: DreamVerificationPolicy,
        expected_graph_version: u64,
    ) -> Result<DreamPublicationOutcome, DreamPublicationError> {
        publish_from_parts(
            crate::owner_lifecycle::DreamGraphPublication::new(
                &mut self.owner.container,
                &self.owner.memories,
                &mut self.owner.graph,
            ),
            &mut self.duplicate_index,
            classification,
            verification,
            policy,
            expected_graph_version,
            DreamSourceTime::Phylactery,
        )
    }
}
