use crate::compatibility_profile_codec::{decode_format, decode_profile, decode_retirement};
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::{CompatibilityProfileError, ObjectRef};

pub(crate) struct CompatibilityProfileOpenState {
    store: CompatibilityProfileStore,
    format_seen: bool,
}

impl CompatibilityProfileOpenState {
    pub(crate) fn new() -> Self {
        Self {
            store: CompatibilityProfileStore::default(),
            format_seen: false,
        }
    }

    pub(crate) fn ingest(
        &mut self,
        _chunk: ObjectRef,
        payload: &[u8],
    ) -> Result<(), CompatibilityProfileError> {
        if decode_format(payload)? {
            if self.format_seen {
                return Err(CompatibilityProfileError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        if let Some(profile) = decode_profile(payload)? {
            self.store.insert_rebuilt(profile)?;
            return Ok(());
        }
        if let Some(id) = decode_retirement(payload)? {
            self.store.retire_rebuilt(id)?;
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<CompatibilityProfileStore, CompatibilityProfileError> {
        if !self.format_seen {
            return Err(CompatibilityProfileError::MissingFormat);
        }
        Ok(self.store)
    }
}
