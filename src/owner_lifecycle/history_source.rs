//! Read-only lifecycle access to one validated physical history snapshot.
//!
//! This capability carries no writer, facade, callback, or Container accessor.

use crate::container::{PhysicalCursor, PhysicalFrame};
use crate::freshness_storage::FreshnessStore;
use crate::{Container, ContainerError, ObjectRef};
use std::fmt;

/// Physical source failure retained across the lifecycle/owner boundary.
#[derive(Debug)]
pub(crate) struct HistoryReadFailure(pub(crate) ContainerError);

impl fmt::Display for HistoryReadFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl std::error::Error for HistoryReadFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl From<ContainerError> for HistoryReadFailure {
    fn from(error: ContainerError) -> Self {
        Self(error)
    }
}

/// A bounded physical frame and the global clock observed after accepting it.
pub(crate) struct HistoryFrame {
    pub(crate) object: ObjectRef,
    pub(crate) payload: Vec<u8>,
    pub(crate) latest_global: u64,
    pub(crate) end: u64,
}

/// Container-neutral facts from an accepted publication envelope.
#[derive(Clone, Copy)]
pub(crate) struct PublicationEvidence<'a> {
    pub(crate) global_version: u64,
    pub(crate) body: &'a [u8],
}

impl HistoryFrame {
    /// Physical grouped-publication coordinates, with the existing owner error
    /// mapping. Container alone parses and authenticates this clock header.
    pub(crate) fn external_version_range(
        &self,
    ) -> Result<Option<(u64, u32, i64)>, crate::MemoryError> {
        crate::container::grouped_clock::external_range(&self.payload)
            .map_err(|_| crate::MemoryError::CorruptRecord("external publication"))
    }

    /// Decode only the Container publication envelope; owner bodies stay opaque.
    pub(crate) fn publication(&self) -> Result<Option<PublicationEvidence<'_>>, String> {
        crate::container::publication_envelope::decode(&self.payload)
            .map(|value| {
                value.map(|envelope| PublicationEvidence {
                    global_version: envelope.global_version,
                    body: envelope.body,
                })
            })
            .map_err(|error| error.to_string())
    }
}

/// Immutable owner read access whose construction performs no I/O. A proof
/// captures its bounded snapshot only when its original read checkpoint is
/// reached, so logical preflight errors keep their precedence.
pub(crate) struct HistoryReadAccess<'a> {
    container: &'a Container,
}

impl<'a> HistoryReadAccess<'a> {
    pub(crate) fn new(container: &'a Container) -> Self {
        Self { container }
    }

    pub(crate) fn owner_id(&self) -> Option<String> {
        self.container.owner_id()
    }
    pub(crate) fn episode_turns(
        &self,
        archive: &crate::Archive,
        id: crate::EpisodeId,
    ) -> Result<Vec<crate::ResolvedTurn>, crate::ArchiveError> {
        archive.episode_turns(self.container, id)
    }

    pub(crate) fn retained_completed_dream_passes(
        &self,
        archive: &crate::Archive,
        memories: &crate::memory_store::MemoryStore,
    ) -> Result<
        std::collections::BTreeMap<crate::MemoryId, crate::InitialDreamPass>,
        super::OwnerError,
    > {
        super::dream_replay::retained_completed_passes(self.container, archive, memories)
    }

    pub(crate) fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.container.owner_uuid()
    }

    /// Preserve the operation's path-metadata preflight before a held-source snapshot.
    pub(crate) fn path_length(&self) -> std::io::Result<u64> {
        std::fs::metadata(self.container.path()).map(|metadata| metadata.len())
    }

    pub(crate) fn latest_global_version(&self) -> u64 {
        self.container.latest_version()
    }

    pub(crate) fn snapshot(&self) -> Result<HistorySource<'a>, HistoryReadFailure> {
        HistorySource::new(self.container)
    }

    pub(crate) fn current_memory(
        &self,
        memories: &crate::memory_store::MemoryStore,
        id: crate::MemoryId,
    ) -> Result<crate::Memory, crate::MemoryError> {
        memories.memory(self.container, id)
    }

    pub(crate) fn memory_revision(
        &self,
        memories: &crate::memory_store::MemoryStore,
        id: crate::MemoryId,
        revision: u64,
    ) -> Result<crate::Memory, crate::MemoryError> {
        memories.memory_revision(self.container, id, revision)
    }
}

/// Read-only access to an immutable Container prefix.
pub(crate) struct HistorySource<'a> {
    container: &'a Container,
    physical_end: u64,
    header_len: u64,
    latest_version: u64,
    owner_uuid: Option<[u8; 16]>,
}

impl<'a> HistorySource<'a> {
    /// Capture a source after validating its held-file identity and current size.
    pub(crate) fn new(container: &'a Container) -> Result<Self, HistoryReadFailure> {
        container.validate_path_identity()?;
        let physical_end = container.physical_length()?;
        let source = Self {
            container,
            physical_end,
            header_len: container.header_len,
            latest_version: container.latest_version(),
            owner_uuid: container.owner_uuid(),
        };
        source.validate()?;
        Ok(source)
    }

    pub(crate) fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.owner_uuid
    }

    pub(crate) fn latest_version(&self) -> u64 {
        self.latest_version
    }

    pub(crate) fn transaction_time_ns(&self, version: u64) -> Option<i64> {
        (version <= self.latest_version)
            .then(|| self.container.transaction_time_ns(version))
            .flatten()
    }

    pub(crate) fn prefix_end(&self) -> u64 {
        self.physical_end
    }

    pub(crate) fn header_len(&self) -> u64 {
        self.header_len
    }

    /// Confirm the path still names the held file and its physical end is stable.
    pub(crate) fn validate(&self) -> Result<(), HistoryReadFailure> {
        self.container.validate_path_identity()?;
        let current_end = self.container.physical_length()?;
        if current_end != self.physical_end {
            return Err(ContainerError::UnstableSnapshot.into());
        }
        Ok(())
    }

    pub(crate) fn cursor(&self) -> Result<HistoryCursor<'_>, HistoryReadFailure> {
        self.prefix_cursor(self.physical_end)
    }

    pub(crate) fn prefix_cursor(&self, end: u64) -> Result<HistoryCursor<'_>, HistoryReadFailure> {
        if end < self.header_len || end > self.physical_end {
            return Err(ContainerError::TruncatedChunk(end).into());
        }
        self.validate()?;
        let cursor = self.container.physical_cursor_through(end)?;
        Ok(HistoryCursor { cursor })
    }

    /// Read an object through this exact held source, preserving physical error type.
    pub(crate) fn read_object(&self, object: ObjectRef) -> Result<Vec<u8>, HistoryReadFailure> {
        self.validate()?;
        self.container
            .read_through(object, self.physical_end)
            .map_err(Into::into)
    }

    pub(crate) fn read(&self, object: ObjectRef) -> Result<Vec<u8>, String> {
        self.read_object(object).map_err(|error| error.to_string())
    }

    /// Materialize one accepted immutable Memory revision through the held source.
    pub(crate) fn memory_revision(
        &self,
        memories: &crate::memory_store::MemoryStore,
        id: crate::MemoryId,
        revision: u64,
    ) -> Result<crate::Memory, crate::MemoryError> {
        self.validate().map_err(|error| error.0)?;
        memories.memory_revision(self.container, id, revision)
    }

    /// Read one owner-local vector set while preserving immutable snapshot checks.
    pub(crate) fn memory_vector_set(
        &self,
        store: &crate::memory_vector_store::MemoryVectorStore,
        id: crate::MemoryVectorId,
    ) -> Result<crate::MemoryVectorSet, crate::MemoryVectorError> {
        self.validate().map_err(|error| error.0)?;
        store.get(self.container, id)
    }

    /// Read one owner-local Archive-vector set through the immutable snapshot.
    pub(crate) fn archive_vector_set(
        &self,
        store: &crate::archive_vector_store::ArchiveVectorStore,
        id: crate::ArchiveVectorId,
    ) -> Result<crate::ArchiveVectorSet, crate::ArchiveVectorError> {
        self.validate().map_err(|error| error.0)?;
        store.get(self.container, id)
    }

    /// Owner-local Archive interpretation through the held immutable source.
    pub(crate) fn archive_publication(
        &self,
        archive: &crate::Archive,
        version: crate::ArchiveRecordVersion,
    ) -> Result<crate::archive_codec::ArchiveRecord, crate::ArchiveError> {
        self.archive_publication_with_version(archive, version)
            .map(|(record, _)| record)
    }

    /// Owner-local Archive interpretation together with the accepted applying
    /// version, for proofs that must compare publication coordinates.
    pub(crate) fn archive_publication_with_version(
        &self,
        archive: &crate::Archive,
        version: crate::ArchiveRecordVersion,
    ) -> Result<
        (
            crate::archive_codec::ArchiveRecord,
            crate::ArchiveRecordVersion,
        ),
        crate::ArchiveError,
    > {
        self.validate().map_err(|error| error.0)?;
        archive.read_publication(self.container, version)
    }

    /// Read content through the immutable Archive view while enforcing the
    /// captured physical source boundary.
    pub(crate) fn archive_content_bytes(
        &self,
        archive: &crate::Archive,
        id: crate::ContentId,
    ) -> Result<Vec<u8>, crate::ArchiveError> {
        self.validate().map_err(|error| error.0)?;
        archive.content_bytes(self.container, id)
    }

    pub(crate) fn archive_retention_proof(
        &self,
        archive: &crate::Archive,
        conversation: &str,
        watermark: u64,
        lengths: &std::collections::HashMap<crate::ContentId, u64>,
    ) -> Result<crate::archive::history_prune::ArchiveRetentionProof, crate::ArchiveError> {
        self.validate().map_err(|error| error.0)?;
        archive.retention_proof(self.container, conversation, watermark, lengths)
    }

    /// Start derived Freshness replay over this owner at the empty prefix.
    pub(crate) fn begin_freshness_replay(&self, owner: [u8; 16]) -> Result<FreshnessStore, String> {
        if self.owner_uuid != Some(owner) {
            return Err("Freshness replay owner does not match history source".into());
        }
        self.validate().map_err(|error| error.to_string())?;
        let mut freshness = FreshnessStore::new(Some(owner));
        freshness.bind_history_source(self.container, 0)?;
        freshness.begin_rebuild();
        Ok(freshness)
    }

    /// Finish derived Freshness replay after binding it to an accepted prefix.
    pub(crate) fn finish_freshness_replay(
        &self,
        freshness: &mut FreshnessStore,
        prefix_end: u64,
    ) -> Result<(), String> {
        if prefix_end < self.header_len || prefix_end > self.physical_end {
            return Err(ContainerError::TruncatedChunk(prefix_end).to_string());
        }
        self.validate().map_err(|error| error.to_string())?;
        freshness.bind_history_source(self.container, prefix_end)?;
        freshness.finish_rebuild()
    }
}

pub(crate) struct HistoryCursor<'a> {
    cursor: PhysicalCursor<'a>,
}

impl HistoryCursor<'_> {
    pub(crate) fn next_frame(&mut self) -> Result<Option<HistoryFrame>, HistoryReadFailure> {
        self.cursor
            .next_frame()
            .map_err(HistoryReadFailure::from)
            .map(|frame| frame.map(to_history_frame))
    }
}

fn to_history_frame(frame: PhysicalFrame) -> HistoryFrame {
    HistoryFrame {
        object: frame.object,
        payload: frame.payload,
        latest_global: frame.latest_global,
        end: frame.end,
    }
}
