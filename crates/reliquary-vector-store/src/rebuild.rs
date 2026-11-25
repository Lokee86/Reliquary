use crate::PackedVectorError;
use crate::codec::{decode_format, decode_object, decode_pruned_marker};
use crate::store::PackedVectorStore;
use reliquary_container::{Container, ObjectRef};

pub struct PackedVectorOpenState {
    pub(crate) store: PackedVectorStore,
    format_seen: bool,
}

impl PackedVectorOpenState {
    pub fn new() -> Self {
        Self {
            store: PackedVectorStore::default(),
            format_seen: false,
        }
    }

    pub(crate) fn new_leased(container: &Container) -> Result<Self, PackedVectorError> {
        let mut state = Self::new();
        state.store.bind_read_lease(container)?;
        Ok(state)
    }

    pub fn ingest(&mut self, chunk: ObjectRef, payload: &[u8]) -> Result<(), PackedVectorError> {
        if decode_format(payload)? {
            if self.format_seen {
                return Err(PackedVectorError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        if let Some(marker) = decode_pruned_marker(payload)? {
            self.store
                .insert_pruned(marker.info, chunk, marker.manifest_digest)?;
            return Ok(());
        }
        let Some(decoded) = decode_object(payload)? else {
            return Ok(());
        };
        self.store.ingest_object(decoded, chunk, payload)
    }

    pub fn finish(self) -> Result<PackedVectorStore, PackedVectorError> {
        if !self.format_seen {
            return Err(PackedVectorError::MissingFormat);
        }
        Ok(self.store)
    }
}
