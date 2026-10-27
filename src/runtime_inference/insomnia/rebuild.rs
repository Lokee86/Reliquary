use crate::{Archive, Cva, CvaError, InsomniaError};
use reliquary_container::ObjectRef;

pub(crate) struct InsomniaFrameValidation {
    format_seen: bool,
}

impl InsomniaFrameValidation {
    pub(crate) fn new() -> Self {
        Self { format_seen: false }
    }

    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), CvaError> {
        if super::completion::decode_completion(payload)
            .map_err(InsomniaError::CorruptRecord)?
            .is_some()
        {
            return Ok(());
        }
        if super::product_adapter::InsomniaStore::is_format(payload)? {
            if self.format_seen {
                return Err(InsomniaError::ConflictingFormat.into());
            }
            self.format_seen = true;
        } else {
            super::product_adapter::InsomniaStore::is_owner_record(payload)?;
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<(), CvaError> {
        if self.format_seen {
            Ok(())
        } else {
            Err(InsomniaError::MissingFormat.into())
        }
    }
}

pub(crate) struct InsomniaOpenState {
    store: super::product_adapter::InsomniaStore,
    format_seen: bool,
}

impl InsomniaOpenState {
    pub(crate) fn new() -> Self {
        Self {
            store: super::product_adapter::InsomniaStore::empty(),
            format_seen: false,
        }
    }

    /// Ingests the second physical pass, after Archive and Memory have accepted
    /// the complete prefix. Completion projection is applied only after all
    /// higher-owner checks pass for that envelope.
    pub(crate) fn ingest_authorized(
        &mut self,
        _chunk: ObjectRef,
        payload: &[u8],
        archive: &Archive,
        memories: &crate::memory_store::MemoryStore,
    ) -> Result<(), CvaError> {
        if let Some(completion) =
            super::completion::decode_completion(payload).map_err(InsomniaError::CorruptRecord)?
        {
            if archive.historical_episode(completion.episode_id).is_none() {
                return Err(InsomniaError::MissingEpisode.into());
            }
            if completion
                .memory_ids
                .iter()
                .any(|id| !memories.contains_historical_memory(*id))
            {
                return Err(InsomniaError::InvalidTransition.into());
            }
            Cva::validate_insomnia_birth_cohort(archive, memories, &completion)?;
            self.store.apply_completion(&completion)?;
            return Ok(());
        }
        if super::product_adapter::InsomniaStore::is_format(payload)? {
            if self.format_seen {
                return Err(InsomniaError::ConflictingFormat.into());
            }
            self.format_seen = true;
            return Ok(());
        }
        self.store.apply_encoded_record(payload)?;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<super::product_adapter::InsomniaStore, InsomniaError> {
        if !self.format_seen {
            return Err(InsomniaError::MissingFormat);
        }
        Ok(self.store)
    }
}
