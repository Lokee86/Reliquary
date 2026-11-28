//! Shared result authorization for one host-admitted assistant turn.

use super::{
    ActivePhyRecall, ArchiveSearchOptions, RetrievalMemoryReference, RetrievalMode,
    RetrievalPreview, RetrievalSession,
};
use crate::{
    ConfiguredRuntime, HermesCopiedSourceTurn, InspectionCut, Instance, MemoryId,
    ObservationCitation, OwnerId, OwnerKind, ProductError, ProductErrorKind, TemporalBasis,
    TemporalFilter,
};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const ARCHIVE_RESULT_LIMIT: usize = 10;

/// A read-only retrieval controller bound to an authenticated Instance and owner scope.
/// Hosts accept a result after their transcript disclosure checks succeed.
pub struct ChatRetrieval {
    instance: Instance,
    session: RetrievalSession,
    returned: Option<Selection>,
    pending: Option<Selection>,
    archive: BTreeMap<String, ArchiveReference>,
    pending_archive: BTreeMap<String, ArchiveReference>,
}

struct ArchiveReference {
    cut: InspectionCut,
    source_archive_version: u64,
    conversation_id: String,
    start_node: String,
    end_node: String,
}

#[derive(Debug, serde::Serialize)]
pub struct ChatArchiveResult {
    pub temporal_filter: TemporalFilter,
    pub hits: Vec<ChatArchiveHit>,
}

#[derive(Debug, serde::Serialize)]
pub struct ChatArchiveHit {
    pub reference: String,
    pub score: f64,
}

struct Selection {
    search_id: String,
    result_id: String,
    cut: MemoryInspectionCut,
    ids: BTreeSet<String>,
    memories: BTreeSet<RetrievalMemoryReference>,
    observations: BTreeSet<ObservationCitation>,
    observation_version: Option<u64>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct MemoryInspectionCut {
    rel: Option<InspectionCut>,
    phy: Option<InspectionCut>,
}

#[derive(Debug, serde::Serialize)]
pub struct ChatMemoryResult {
    pub search_id: String,
    pub result_id: String,
    pub preview: Option<RetrievalPreview>,
}

impl Instance {
    pub fn begin_chat_retrieval(
        &self,
        owner: OwnerId,
        turn_id: &str,
    ) -> Result<ChatRetrieval, ProductError> {
        let session = self.begin_retrieval(owner, turn_id, RetrievalMode::Inspect)?;
        Ok(self.chat_retrieval(session))
    }

    pub fn begin_scoped_chat_retrieval(
        &self,
        rel: Option<OwnerId>,
        phy: ActivePhyRecall,
        turn_id: &str,
    ) -> Result<ChatRetrieval, ProductError> {
        let session = self.begin_scoped_retrieval(rel, phy, turn_id, RetrievalMode::Inspect)?;
        Ok(self.chat_retrieval(session))
    }

    fn chat_retrieval(&self, session: RetrievalSession) -> ChatRetrieval {
        ChatRetrieval {
            instance: self.clone(),
            session,
            returned: None,
            pending: None,
            archive: BTreeMap::new(),
            pending_archive: BTreeMap::new(),
        }
    }
}

impl ChatRetrieval {
    /// Search the bound REL. The host confirms the results it discloses.
    pub fn search_chat_archive(
        &mut self,
        query: &str,
        basis: TemporalBasis,
        request_time_ns: i64,
        calendar_zone: &str,
        runtime: &ConfiguredRuntime,
    ) -> Result<ChatArchiveResult, ProductError> {
        let cut = self.archive_cut()?;
        self.pending_archive.clear();
        let result = self.instance.search_archive_for_query(
            self.session.owner(),
            query,
            basis,
            ArchiveSearchOptions {
                limit: Some(ARCHIVE_RESULT_LIMIT),
                embedding_route: None,
            },
            runtime,
            request_time_ns,
            calendar_zone,
        )?;
        if self.archive_cut()? != cut {
            return Err(stale_selection());
        }
        let mut hits = Vec::new();
        let mut ranges = BTreeSet::new();
        for (hit, time) in result.search.hits.iter().zip(&result.temporal_evidence) {
            if hit.fragment_id != time.fragment_id {
                return Err(stale_selection());
            }
            let eligible = if time.partial {
                time.turns
                    .iter()
                    .map(|turn| (turn.node_id.as_str(), turn.node_id.as_str()))
                    .collect()
            } else {
                vec![(hit.start_node_id.as_str(), hit.end_node_id.as_str())]
            };
            for (start, end) in eligible {
                if hits.len() == ARCHIVE_RESULT_LIMIT {
                    break;
                }
                if !ranges.insert((hit.conversation_id.as_str(), start, end)) {
                    continue;
                }
                let reference = Uuid::new_v4().simple().to_string();
                self.pending_archive.insert(
                    reference.clone(),
                    ArchiveReference {
                        cut,
                        source_archive_version: result.search.source_archive_version,
                        conversation_id: hit.conversation_id.clone(),
                        start_node: start.to_owned(),
                        end_node: end.to_owned(),
                    },
                );
                hits.push(ChatArchiveHit {
                    reference,
                    score: hit.score,
                });
            }
        }
        Ok(ChatArchiveResult {
            temporal_filter: result.temporal_filter,
            hits,
        })
    }

    /// Inspect a candidate before the trusted host commits its disclosure.
    pub fn preview_archive_reference(
        &self,
        reference: &str,
    ) -> Result<Vec<HermesCopiedSourceTurn>, ProductError> {
        self.read_reference(self.pending_archive.get(reference))
    }

    pub fn accept_archive_results(&mut self, references: &[String]) -> Result<(), ProductError> {
        let cut = self.archive_cut()?;
        self.archive.retain(|_, claim| claim.cut == cut);
        if references.len() + self.archive.len() > 128
            || references.iter().collect::<BTreeSet<_>>().len() != references.len()
            || references.iter().any(|reference| {
                self.pending_archive
                    .get(reference)
                    .is_none_or(|claim| claim.cut != cut)
            })
        {
            return Err(stale_selection());
        }
        for reference in references {
            let claim = self
                .pending_archive
                .remove(reference)
                .expect("validated Archive reference");
            self.archive.insert(reference.clone(), claim);
        }
        self.pending_archive.clear();
        Ok(())
    }

    pub fn has_archive_references(&self) -> bool {
        self.archive_cut()
            .is_ok_and(|cut| self.archive.values().any(|claim| claim.cut == cut))
    }

    /// Resolve a disclosed reference against the bound owner and original cut.
    pub fn read_archive_reference(
        &self,
        reference: &str,
    ) -> Result<Vec<HermesCopiedSourceTurn>, ProductError> {
        self.read_reference(self.archive.get(reference))
    }

    fn read_reference(
        &self,
        reference: Option<&ArchiveReference>,
    ) -> Result<Vec<HermesCopiedSourceTurn>, ProductError> {
        let cut = self.archive_cut()?;
        let claim = reference
            .filter(|reference| reference.cut == cut)
            .ok_or_else(stale_selection)?;
        let rows = self.instance.read_hermes_archive_range(
            self.session.owner(),
            &claim.conversation_id,
            &claim.start_node,
            &claim.end_node,
            claim.source_archive_version,
        )?;
        if self.archive_cut()? != cut {
            return Err(stale_selection());
        }
        rows.ok_or_else(stale_selection)
    }

    pub fn query_memory(
        &mut self,
        query: &str,
        runtime: &ConfiguredRuntime,
    ) -> Result<ChatMemoryResult, ProductError> {
        self.query(query, runtime, None)
    }

    /// Query the scoped ordinary population with one shared result limit.
    pub fn query_memory_limited(
        &mut self,
        query: &str,
        runtime: &ConfiguredRuntime,
        limit: usize,
    ) -> Result<ChatMemoryResult, ProductError> {
        self.query(query, runtime, Some(limit))
    }

    fn query(
        &mut self,
        query: &str,
        runtime: &ConfiguredRuntime,
        limit: Option<usize>,
    ) -> Result<ChatMemoryResult, ProductError> {
        self.returned = None;
        self.pending = None;
        let cut = self.cut()?;
        let outcome = if let Some(limit) = limit {
            self.instance
                .memory_query_limited(&self.session, query, runtime, limit)?
        } else {
            self.instance.memory_query(&self.session, query, runtime)?
        };
        self.prepare(Uuid::new_v4().simple().to_string(), cut, outcome.preview)
    }

    pub fn refine_memory(
        &mut self,
        search_id: &str,
        selected_ids: &[String],
        query: &str,
        runtime: &ConfiguredRuntime,
    ) -> Result<ChatMemoryResult, ProductError> {
        let cut = self.validate_followup(search_id, selected_ids, false)?;
        self.pending = None;
        let outcome = self.instance.refine_query(&self.session, query, runtime)?;
        self.prepare(search_id.to_owned(), cut, outcome.preview)
    }

    pub fn traverse_memory(
        &mut self,
        search_id: &str,
        result_id: &str,
        seed_ids: &[String],
        runtime: &ConfiguredRuntime,
    ) -> Result<ChatMemoryResult, ProductError> {
        if self
            .returned
            .as_ref()
            .is_none_or(|returned| returned.result_id != result_id)
        {
            return Err(stale_selection());
        }
        let cut = self.validate_followup(search_id, seed_ids, true)?;
        self.pending = None;
        let outcome = if self.session.phy_owner().is_some() {
            let returned = self.returned.as_ref().ok_or_else(stale_selection)?;
            let anchors = seed_ids
                .iter()
                .map(|id| {
                    returned
                        .memories
                        .iter()
                        .find(|reference| scoped_reference_id(reference) == *id)
                        .cloned()
                        .ok_or_else(stale_selection)
                })
                .collect::<Result<Vec<_>, _>>()?;
            self.instance
                .traverse_scoped_graph(&self.session, &anchors, runtime)?
        } else {
            let anchors = seed_ids
                .iter()
                .map(|id| parse_memory_id(id))
                .collect::<Result<Vec<_>, _>>()?;
            self.instance
                .traverse_graph(&self.session, &anchors, runtime)?
        };
        self.prepare(search_id.to_owned(), cut, outcome.preview)
    }

    /// Authorize only the IDs actually disclosed to the model by the trusted host.
    pub fn accept_memory_result(
        &mut self,
        result_id: &str,
        disclosed_ids: &[String],
    ) -> Result<(), ProductError> {
        self.accept_typed_memory_result(result_id, disclosed_ids, &[])
    }

    /// Authorize only the Memory IDs and exact Observation revisions disclosed
    /// by the trusted host. Observation references never become graph seeds.
    pub fn accept_typed_memory_result(
        &mut self,
        result_id: &str,
        disclosed_ids: &[String],
        disclosed_observations: &[ObservationCitation],
    ) -> Result<(), ProductError> {
        if self.session.phy_owner().is_some() {
            return Err(ProductError::invalid(
                "scoped retrieval requires owner-qualified result references",
            ));
        }
        let cut = self.cut()?;
        let pending = self
            .pending
            .as_ref()
            .filter(|selection| selection.result_id == result_id)
            .ok_or_else(stale_selection)?;
        if cut != pending.cut {
            return Err(stale_selection());
        }
        validate_ids(disclosed_ids, &pending.ids, false)?;
        if !disclosed_observations.is_empty() {
            let unique: BTreeSet<_> = disclosed_observations.iter().copied().collect();
            if unique.len() != disclosed_observations.len()
                || !unique.is_subset(&pending.observations)
            {
                return Err(ProductError::invalid(
                    "Observation citations must come from the authorized result",
                ));
            }
            self.instance.inner.accept_inspected_observations(
                &self.session.inner,
                pending.observation_version.ok_or_else(stale_selection)?,
                disclosed_observations,
            )?;
        }
        let mut selection = self.pending.take().expect("validated pending result");
        selection.ids.retain(|id| disclosed_ids.contains(id));
        self.returned = Some(selection);
        Ok(())
    }

    /// Authorize the exact owner-qualified references disclosed by the host.
    pub fn accept_scoped_memory_result(
        &mut self,
        result_id: &str,
        memories: &[RetrievalMemoryReference],
        observations: &[ObservationCitation],
    ) -> Result<(), ProductError> {
        let cut = self.cut()?;
        let pending = self
            .pending
            .as_ref()
            .filter(|selection| selection.result_id == result_id && selection.cut == cut)
            .ok_or_else(stale_selection)?;
        let unique_memories = memories.iter().cloned().collect::<BTreeSet<_>>();
        let unique_observations = observations.iter().copied().collect::<BTreeSet<_>>();
        if unique_memories.len() != memories.len()
            || unique_observations.len() != observations.len()
            || !unique_memories.is_subset(&pending.memories)
            || !unique_observations.is_subset(&pending.observations)
        {
            return Err(ProductError::invalid(
                "result references must come from the authorized result",
            ));
        }
        self.instance.inner.accept_inspected_scoped_selection(
            &self.session.inner,
            memories,
            observations,
        )?;
        let mut selection = self.pending.take().expect("validated pending result");
        selection.ids = memories.iter().map(scoped_reference_id).collect();
        selection.memories = unique_memories;
        selection.observations = unique_observations;
        self.returned = Some(selection);
        Ok(())
    }

    /// Recheck exact owner-qualified results before model disclosure.
    pub fn validate_scoped_memory_result(
        &self,
        result_id: &str,
        memories: &[RetrievalMemoryReference],
        observations: &[ObservationCitation],
    ) -> Result<(), ProductError> {
        let cut = self.cut()?;
        let selection = self
            .pending
            .as_ref()
            .or(self.returned.as_ref())
            .filter(|selection| selection.result_id == result_id && selection.cut == cut)
            .ok_or_else(stale_selection)?;
        if memories
            .iter()
            .any(|reference| !selection.memories.contains(reference))
            || observations
                .iter()
                .any(|citation| !selection.observations.contains(citation))
        {
            return Err(stale_selection());
        }
        self.instance.inner.validate_inspected_scoped_selection(
            &self.session.inner,
            memories,
            observations,
        )?;
        Ok(())
    }

    fn archive_cut(&self) -> Result<InspectionCut, ProductError> {
        self.instance.validate_retrieval(&self.session)?;
        let owner = self.session.rel_owner().ok_or_else(stale_selection)?;
        self.instance
            .semantic_graph_inspection_cut(owner, OwnerKind::Rel)
    }

    fn cut(&self) -> Result<MemoryInspectionCut, ProductError> {
        self.instance.validate_retrieval(&self.session)?;
        Ok(MemoryInspectionCut {
            rel: self
                .session
                .rel_owner()
                .map(|owner| {
                    self.instance
                        .semantic_graph_inspection_cut(owner, OwnerKind::Rel)
                })
                .transpose()?,
            phy: self
                .session
                .phy_owner()
                .map(|owner| {
                    self.instance
                        .semantic_graph_inspection_cut(owner, OwnerKind::Phylactery)
                })
                .transpose()?,
        })
    }

    fn validate_followup(
        &self,
        search_id: &str,
        ids: &[String],
        require_ids: bool,
    ) -> Result<MemoryInspectionCut, ProductError> {
        let cut = self.cut()?;
        let returned = self
            .returned
            .as_ref()
            .filter(|selection| selection.search_id == search_id && selection.cut == cut)
            .ok_or_else(stale_selection)?;
        if self.pending.is_some() {
            return Err(stale_selection());
        }
        validate_ids(ids, &returned.ids, require_ids)?;
        Ok(cut)
    }

    fn prepare(
        &mut self,
        search_id: String,
        cut: MemoryInspectionCut,
        preview: Option<RetrievalPreview>,
    ) -> Result<ChatMemoryResult, ProductError> {
        if self.cut()? != cut {
            return Err(stale_selection());
        }
        let result_id = Uuid::new_v4().simple().to_string();
        let memories = preview
            .iter()
            .flat_map(|preview| &preview.memories)
            .map(|memory| memory.reference())
            .collect::<BTreeSet<_>>();
        let ids = memories
            .iter()
            .map(|memory| {
                if self.session.phy_owner().is_some() {
                    scoped_reference_id(memory)
                } else {
                    memory.memory_id.clone()
                }
            })
            .collect();
        let observations = preview
            .iter()
            .flat_map(|preview| &preview.observations)
            .map(|observation| observation.claim.citation)
            .collect();
        let observation_version = preview
            .as_ref()
            .and_then(|preview| preview.cut.observation_version);
        self.pending = Some(Selection {
            search_id: search_id.clone(),
            result_id: result_id.clone(),
            cut,
            ids,
            memories,
            observations,
            observation_version,
        });
        Ok(ChatMemoryResult {
            search_id,
            result_id,
            preview,
        })
    }
}

impl Drop for ChatRetrieval {
    fn drop(&mut self) {
        let _ = self.instance.cancel_retrieval(&self.session);
    }
}

fn validate_ids(
    ids: &[String],
    permitted: &BTreeSet<String>,
    required: bool,
) -> Result<(), ProductError> {
    let unique: BTreeSet<_> = ids.iter().collect();
    if (required && ids.is_empty())
        || ids.len() > 128
        || unique.len() != ids.len()
        || ids.iter().any(|id| !permitted.contains(id))
    {
        return Err(ProductError::invalid(
            "Memory IDs must come from the authorized result",
        ));
    }
    Ok(())
}

fn parse_memory_id(id: &str) -> Result<MemoryId, ProductError> {
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ProductError::invalid("invalid Memory ID"));
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&id[index * 2..index * 2 + 2], 16)
            .map_err(|_| ProductError::invalid("invalid Memory ID"))?;
    }
    Ok(MemoryId(bytes))
}

fn stale_selection() -> ProductError {
    ProductError::new(
        ProductErrorKind::Stale,
        "The retrieval result is unavailable in this turn or cut",
    )
}

fn scoped_reference_id(reference: &RetrievalMemoryReference) -> String {
    format!("{}:{}", reference.owner_id, reference.memory_id)
}
