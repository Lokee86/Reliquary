//! Scoped completion publication into an exclusively opened offline REL.
use super::OwnerError;
use crate::container::{Container, FileKind};
use crate::imports::epoch_store::{ImportCompletionPublication, ImportEpochStore};

/// Temporary REL import capability. It holds only the owner-file writer and
/// can consume an opaque completion admitted by the Import coordinator.
pub(crate) struct ImportSession<'a> {
    container: &'a mut Container,
}

impl<'a> ImportSession<'a> {
    /// The caller must hold the ordinary exclusive REL open. Phylactery and
    /// legacy scoped/non-REL containers cannot acquire the Import writer.
    pub(crate) fn offline_rel(container: &'a mut Container) -> Result<Self, OwnerError> {
        if container.identity().map(|identity| identity.file_kind) != Some(FileKind::Reliquary)
            || container.owner_uuid().is_none()
        {
            return Err(OwnerError::InvalidContainerIdentity(
                "offline Import requires a versioned Reliquary owner",
            ));
        }
        container.set_owner_write_scope(true);
        Ok(Self { container })
    }

    /// Provide bounded read access through this same exclusive owner session.
    /// The borrow ends before publication mutates the physical owner file.
    pub(crate) fn history_access(&self) -> crate::owner_lifecycle::HistoryReadAccess<'_> {
        crate::owner_lifecycle::HistoryReadAccess::new(self.container)
    }

    /// Persist the admission fence once the recovered open epoch is accepted.
    pub(crate) fn hold_open_import_fence(&mut self) {
        self.container.set_write_admission_closed(true);
    }

    /// A recovered completion retry synchronizes first, then restores the
    /// fence from accepted epoch state.
    pub(crate) fn sync_completed_retry(
        &mut self,
        epochs: &ImportEpochStore,
    ) -> Result<(), OwnerError> {
        self.container.sync()?;
        self.container
            .set_write_admission_closed(epochs.open().is_some());
        Ok(())
    }

    /// Append, synchronize, and only then release the staged completed epoch
    /// and its write fence. An append/sync error leaves admission closed.
    pub(crate) fn publish_completion(
        &mut self,
        staged_epochs: &mut ImportEpochStore,
        publication: ImportCompletionPublication,
    ) -> Result<(), OwnerError> {
        let append = staged_epochs.append_complete_publication(self.container, publication);
        // The store's append operation acknowledges its local transition. The
        // lifecycle boundary restores the durable fence until sync succeeds.
        self.container.set_write_admission_closed(true);
        append?;
        self.container.sync()?;
        self.container.set_write_admission_closed(false);
        Ok(())
    }
}

impl Drop for ImportSession<'_> {
    fn drop(&mut self) {
        self.container.set_owner_write_scope(false);
    }
}
