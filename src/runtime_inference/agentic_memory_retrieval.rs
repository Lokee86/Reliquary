//! Turn-scoped agentic Memory retrieval state. The embedding host owns its lifetime.
use crate::MemoryRef;
use crate::agentic_memory_retrieval_model::{AgenticMemoryError, AgenticMemoryOperation};
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
    operation: AgenticMemoryOperation,
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
            operation: AgenticMemoryOperation::Query,
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
            operation: AgenticMemoryOperation::Refinement,
        })
    }

    pub(crate) fn stage_traversal(
        &self,
    ) -> Result<StagedAgenticMemoryOperation, AgenticMemoryError> {
        self.require_query()?;
        Ok(StagedAgenticMemoryOperation {
            expected: self.clone(),
            next: self.clone(),
            operation: AgenticMemoryOperation::Traversal,
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

    pub(crate) fn record_evaluated(
        &mut self,
        ids: impl IntoIterator<Item = MemoryRef>,
    ) -> Result<(), AgenticMemoryError> {
        let ids = ids.into_iter().collect::<Vec<_>>();
        if ids.iter().any(|id| {
            self.expected.selected_ids.contains(id)
                || match self.operation {
                    AgenticMemoryOperation::Query => false,
                    AgenticMemoryOperation::Refinement => !self.expected.searched_ids.contains(id),
                    AgenticMemoryOperation::Traversal => self.expected.searched_ids.contains(id),
                }
        }) {
            return Err(AgenticMemoryError::InvalidEvaluation);
        }
        self.next.searched_ids.extend(ids);
        Ok(())
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

#[cfg(test)]
#[path = "agentic_memory_retrieval_tests.rs"]
mod agentic_memory_retrieval_tests;
