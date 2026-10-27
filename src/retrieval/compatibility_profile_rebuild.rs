use crate::CompatibilityProfileError;
use reliquary_container::ObjectRef;

pub(crate) struct CompatibilityProfileOpenState {
    inner: reliquary_vector_store::CompatibilityProfileOpenState,
}

impl CompatibilityProfileOpenState {
    pub(crate) fn new() -> Self {
        Self {
            inner: reliquary_vector_store::CompatibilityProfileOpenState::new(),
        }
    }
    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
    ) -> Result<(), CompatibilityProfileError> {
        self.inner.ingest(chunk, payload).map_err(Into::into)
    }
    pub(crate) fn finish(
        self,
    ) -> Result<
        super::compatibility_profile_store::CompatibilityProfileStore,
        CompatibilityProfileError,
    > {
        self.inner
            .finish()
            .map(super::compatibility_profile_store::CompatibilityProfileStore::from_store)
            .map_err(Into::into)
    }
}
