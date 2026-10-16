//! Host-owned state for exactly one requesting-agent turn.
use crate::MemoryRef;
use std::collections::BTreeSet;

/// Runtime state only. The embedding host creates this value per turn.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AgenticMemoryController {
    pub(crate) current_query: Option<String>,
    pub(crate) searched_ids: BTreeSet<MemoryRef>,
    pub(crate) selected_ids: BTreeSet<MemoryRef>,
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
