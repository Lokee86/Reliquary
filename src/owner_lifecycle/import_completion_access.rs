//! Physical access scoped to one Import completion operation.
use super::{HistoryReadAccess, ImportSession, OwnerError};
use crate::Container;

/// Operation-specific REL access for Import recovery and completion. The
/// container stays private; callers can request immutable history or a scoped
/// Import session, but cannot retrieve a writer or install callbacks.
pub(crate) struct ImportCompletionAccess<'a> {
    container: &'a mut Container,
}

impl<'a> ImportCompletionAccess<'a> {
    pub(crate) fn new(container: &'a mut Container) -> Self {
        Self { container }
    }

    pub(crate) fn history_access(&self) -> HistoryReadAccess<'_> {
        HistoryReadAccess::new(self.container)
    }

    pub(crate) fn session(&mut self) -> Result<ImportSession<'_>, OwnerError> {
        ImportSession::offline_rel(self.container)
    }
}
