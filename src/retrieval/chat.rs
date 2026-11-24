//! Shared result authorization for one host-admitted assistant turn.

use super::{ArchiveSearchOptions, RetrievalMode, RetrievalPreview, RetrievalSession};
use crate::{
    ConfiguredRuntime, HermesCopiedSourceTurn, InspectionCut, Instance, MemoryId, OwnerId,
    OwnerKind, ProductError, ProductErrorKind, TemporalBasis, TemporalFilter,
};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const ARCHIVE_RESULT_LIMIT: usize = 10;

/// A read-only retrieval controller bound to the authenticated Instance and REL.
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
    cut: InspectionCut,
    ids: BTreeSet<String>,
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
        Ok(ChatRetrieval {
            instance: self.clone(),
            session: self.begin_retrieval(owner, turn_id, RetrievalMode::Inspect)?,
            returned: None,
            pending: None,
            archive: BTreeMap::new(),
            pending_archive: BTreeMap::new(),
        })
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
        let cut = self.cut()?;
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
        if self.cut()? != cut {
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
        let cut = self.cut()?;
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
        self.cut()
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
        let cut = self.cut()?;
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
        if self.cut()? != cut {
            return Err(stale_selection());
        }
        rows.ok_or_else(stale_selection)
    }

    pub fn query_memory(
        &mut self,
        query: &str,
        runtime: &ConfiguredRuntime,
    ) -> Result<ChatMemoryResult, ProductError> {
        self.returned = None;
        self.pending = None;
        let cut = self.cut()?;
        let outcome = self.instance.memory_query(&self.session, query, runtime)?;
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
        let anchors = seed_ids
            .iter()
            .map(|id| parse_memory_id(id))
            .collect::<Result<Vec<_>, _>>()?;
        let outcome = self
            .instance
            .traverse_graph(&self.session, &anchors, runtime)?;
        self.prepare(search_id.to_owned(), cut, outcome.preview)
    }

    /// Authorize only the IDs actually disclosed to the model by the trusted host.
    pub fn accept_memory_result(
        &mut self,
        result_id: &str,
        disclosed_ids: &[String],
    ) -> Result<(), ProductError> {
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
        let mut selection = self.pending.take().expect("validated pending result");
        selection.ids.retain(|id| disclosed_ids.contains(id));
        self.returned = Some(selection);
        Ok(())
    }

    fn cut(&self) -> Result<InspectionCut, ProductError> {
        self.instance
            .semantic_graph_inspection_cut(self.session.owner(), OwnerKind::Rel)
    }

    fn validate_followup(
        &self,
        search_id: &str,
        ids: &[String],
        require_ids: bool,
    ) -> Result<InspectionCut, ProductError> {
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
        cut: InspectionCut,
        preview: Option<RetrievalPreview>,
    ) -> Result<ChatMemoryResult, ProductError> {
        if self.cut()? != cut {
            return Err(stale_selection());
        }
        let result_id = Uuid::new_v4().simple().to_string();
        let ids = preview
            .iter()
            .flat_map(|preview| &preview.memories)
            .map(|memory| memory.memory_id.clone())
            .collect();
        self.pending = Some(Selection {
            search_id: search_id.clone(),
            result_id: result_id.clone(),
            cut,
            ids,
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
