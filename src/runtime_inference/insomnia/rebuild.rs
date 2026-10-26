use super::codec::{InsomniaRecord, decode_format, decode_record};
use crate::InsomniaError;
use reliquary_container::ObjectRef;

pub(crate) struct InsomniaOpenState {
    store: super::store::InsomniaStore,
    format_seen: bool,
}

impl InsomniaOpenState {
    pub(crate) fn new() -> Self {
        Self {
            store: super::store::InsomniaStore::empty(),
            format_seen: false,
        }
    }

    pub(crate) fn ingest(
        &mut self,
        _chunk: ObjectRef,
        payload: &[u8],
    ) -> Result<(), InsomniaError> {
        if let Some(completion) =
            super::completion::decode_completion(payload).map_err(InsomniaError::CorruptRecord)?
        {
            self.store.apply_completion(&completion)?;
            return Ok(());
        }
        if decode_format(payload)? {
            if self.format_seen {
                return Err(InsomniaError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        if let Some(record) = decode_record(payload)? {
            match record {
                InsomniaRecord::Work(work) => self.store.apply_work(work),
                InsomniaRecord::Attempt(attempt) => self.store.apply_attempt(attempt),
            }
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<super::store::InsomniaStore, InsomniaError> {
        if !self.format_seen {
            return Err(InsomniaError::MissingFormat);
        }
        Ok(self.store)
    }
}
