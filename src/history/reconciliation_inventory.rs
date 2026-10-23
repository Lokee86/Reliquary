//! Narrow immutable source reads used by divergent Import reconciliation.
//!
//! This view owns Container-dependent materialization. Import receives only
//! source facts and keeps the reconciliation policy and normalization rules.

use crate::container::ObjectRef;
use crate::owner_lifecycle::{HistoryReadAccess, HistoryReadFailure, HistorySource, OwnerError};
use crate::{Archive, MemoryError, memory_store::MemoryStore};

pub(crate) struct ReconciliationRead<'a> {
    access: HistoryReadAccess<'a>,
    archive: &'a Archive,
    memories: &'a MemoryStore,
}

impl<'a> ReconciliationRead<'a> {
    pub(crate) fn new(
        access: HistoryReadAccess<'a>,
        archive: &'a Archive,
        memories: &'a MemoryStore,
    ) -> Self {
        Self {
            access,
            archive,
            memories,
        }
    }

    /// Preserve the existing first full-source physical scan checkpoint.
    pub(crate) fn materialized_resolution_changes(
        &self,
    ) -> Result<
        std::collections::BTreeMap<
            ObjectRef,
            crate::entity_resolution_codec::DecodedEntityResolution,
        >,
        MemoryError,
    > {
        self.access.materialized_resolution_changes()
    }

    pub(crate) fn snapshot(&self) -> Result<HistorySource<'a>, HistoryReadFailure> {
        self.access.snapshot()
    }

    pub(crate) fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.access.owner_uuid()
    }

    pub(crate) fn physical_objects(
        &self,
        source: &HistorySource<'_>,
    ) -> Result<Vec<ObjectRef>, crate::ContainerError> {
        let mut cursor = source.cursor().map_err(|error| error.0)?;
        let mut objects = Vec::new();
        while let Some(frame) = cursor.next_frame().map_err(|error| error.0)? {
            objects.push(frame.object);
        }
        Ok(objects)
    }

    pub(crate) fn read_object(
        &self,
        source: &HistorySource<'_>,
        object: ObjectRef,
    ) -> Result<Vec<u8>, crate::ContainerError> {
        source.read_object(object).map_err(|error| error.0)
    }

    pub(crate) fn archive_publication(
        &self,
        source: &HistorySource<'_>,
        version: crate::ArchiveRecordVersion,
    ) -> Result<crate::archive_codec::ArchiveRecord, crate::ArchiveError> {
        source.archive_publication(self.archive, version)
    }

    pub(crate) fn materialized_dream_begin(
        &self,
        source: &HistorySource<'_>,
        object: ObjectRef,
        payload: &[u8],
    ) -> Result<Option<crate::InitialDreamPass>, OwnerError> {
        source.materialized_dream_begin(self.archive, self.memories, object, payload)
    }

    pub(crate) fn hydrate_grouped_completion(
        &self,
        source: &HistorySource<'_>,
        completion: &mut crate::insomnia::completion::InsomniaCompletion,
    ) -> Result<(), OwnerError> {
        source
            .hydrate_grouped_completion(self.memories, completion)
            .map_err(OwnerError::from)
    }

    pub(crate) fn memory_record_at_publication(
        &self,
        global_version: u64,
        memory_version: u64,
    ) -> Option<crate::memory_model::MemoryRecord> {
        self.memories
            .record_at_publication(global_version, memory_version)
            .cloned()
    }
}
