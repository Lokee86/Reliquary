//! Turn-scoped agentic Memory retrieval state. The embedding host owns its lifetime.
use crate::MemoryRef;
use crate::agentic_memory_retrieval_model::AgenticMemoryError;
use std::collections::BTreeSet;

/// One requesting-agent turn. Preparation never records delivery.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AgenticMemoryController {
    pub(crate) current_query: Option<String>,
    pub(crate) searched_ids: BTreeSet<MemoryRef>,
    pub(crate) selected_ids: BTreeSet<MemoryRef>,
}

/// An operation-local snapshot. Dropping it discards all tentative transitions.
/// This is not a controller registry or a pending-delivery store.
pub(crate) struct StagedAgenticMemoryOperation {
    expected: AgenticMemoryController,
    next: AgenticMemoryController,
}

impl AgenticMemoryController {
    pub(crate) fn stage_query(
        &self,
        query: &str,
    ) -> Result<StagedAgenticMemoryOperation, AgenticMemoryError> {
        let query = validate_query(query)?;
        let mut next = self.clone();
        next.current_query = Some(query);
        next.searched_ids.clear();
        Ok(StagedAgenticMemoryOperation {
            expected: self.clone(),
            next,
        })
    }

    pub(crate) fn stage_refinement(
        &self,
        revised_query: &str,
    ) -> Result<StagedAgenticMemoryOperation, AgenticMemoryError> {
        self.require_query()?;
        let mut next = self.clone();
        next.current_query = Some(validate_query(revised_query)?);
        Ok(StagedAgenticMemoryOperation {
            expected: self.clone(),
            next,
        })
    }

    pub(crate) fn stage_traversal(
        &self,
    ) -> Result<StagedAgenticMemoryOperation, AgenticMemoryError> {
        self.require_query()?;
        Ok(StagedAgenticMemoryOperation {
            expected: self.clone(),
            next: self.clone(),
        })
    }

    pub(crate) fn declined_ids(&self) -> BTreeSet<MemoryRef> {
        self.searched_ids
            .difference(&self.selected_ids)
            .cloned()
            .collect()
    }

    fn require_query(&self) -> Result<&str, AgenticMemoryError> {
        self.current_query()
            .ok_or(AgenticMemoryError::NoActiveQuery)
    }
}

impl StagedAgenticMemoryOperation {
    pub(crate) fn query(&self) -> &str {
        self.next
            .current_query
            .as_deref()
            .expect("staged operation has a query")
    }

    pub(crate) fn record_evaluated(&mut self, ids: impl IntoIterator<Item = MemoryRef>) {
        self.next.searched_ids.extend(ids);
    }

    pub(crate) fn commit(
        self,
        controller: &mut AgenticMemoryController,
    ) -> Result<(), AgenticMemoryError> {
        if *controller != self.expected {
            return Err(AgenticMemoryError::StaleOperation);
        }
        // SelectedIDs is never changed by any preparation operation.
        debug_assert_eq!(self.next.selected_ids, self.expected.selected_ids);
        *controller = self.next;
        Ok(())
    }
}

pub(crate) fn validate_query(query: &str) -> Result<String, AgenticMemoryError> {
    let query = query.trim();
    if query.is_empty() || query.len() > crate::MAX_MEMORY_SEARCH_QUERY_BYTES {
        return Err(AgenticMemoryError::InvalidQuery);
    }
    Ok(query.to_owned())
}

impl AgenticMemoryController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_query(&self) -> Option<&str> {
        self.current_query.as_deref()
    }

    pub fn searched_ids(&self) -> &BTreeSet<MemoryRef> {
        &self.searched_ids
    }

    pub fn selected_ids(&self) -> &BTreeSet<MemoryRef> {
        &self.selected_ids
    }
}
