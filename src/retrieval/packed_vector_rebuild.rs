use crate::packed_vector_codec::{decode_format, decode_object, decode_pruned_marker};
use crate::packed_vector_store::{PackedVectorStore, packed_vector_id};
use crate::{ObjectRef, PackedVectorError, PackedVectorInfo};

pub(crate) struct PackedVectorOpenState {
    store: PackedVectorStore,
    format_seen: bool,
}

impl PackedVectorOpenState {
    pub(crate) fn new() -> Self {
        Self {
            store: PackedVectorStore::default(),
            format_seen: false,
        }
    }

    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
    ) -> Result<(), PackedVectorError> {
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
        if packed_vector_id(decoded.schema, decoded.bytes) != decoded.id {
            return Err(PackedVectorError::HashCollision);
        }
        self.store.insert_rebuilt(
            PackedVectorInfo {
                id: decoded.id,
                schema: decoded.schema,
                count: decoded.count,
                byte_len: u64::try_from(decoded.bytes.len())
                    .map_err(|_| PackedVectorError::SizeOverflow)?,
            },
            chunk,
        )?;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<PackedVectorStore, PackedVectorError> {
        if !self.format_seen {
            return Err(PackedVectorError::MissingFormat);
        }
        Ok(self.store)
    }
}
