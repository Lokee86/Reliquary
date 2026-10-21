#[cfg(test)]
use crate::ObjectRef;
use crate::archive_codec::{ArchiveRecord, decode_record};
use crate::archive_history_codec::encode_archive_format;
#[cfg(test)]
use crate::archive_history_codec::encode_record_version;
use crate::{Archive, ArchiveError, ArchiveRecordVersion, Branch, Container};

pub(crate) struct PreparedArchivePublication {
    body: Vec<u8>,
    archive_version: u64,
    next_archive_version: u64,
    global_version: u64,
}

#[cfg(test)]
#[path = "archive_publication_writer_tests.rs"]
mod publication_writer_tests;

impl Archive {
    pub(crate) fn initialize_history_format(
        &self,
        container: &mut Container,
    ) -> Result<(), ArchiveError> {
        container.require_storage_format_writable()?;
        container.append(&encode_archive_format())?;
        Ok(())
    }

    pub fn archive_version(&self) -> u64 {
        self.next_archive_version.saturating_sub(1)
    }

    pub fn record_versions(&self) -> &[ArchiveRecordVersion] {
        &self.record_versions
    }

    pub fn record_version(&self, archive_version: u64) -> Option<&ArchiveRecordVersion> {
        let index = archive_version.checked_sub(1)? as usize;
        self.record_versions.get(index)
    }

    /// Resolve semantic evidence at its first applying coordinates. Repeated
    /// bindings retain their own cuts, but must not reapply an older mutation.
    pub(crate) fn read_publication(
        &self,
        container: &mut Container,
        version: ArchiveRecordVersion,
    ) -> Result<(ArchiveRecord, ArchiveRecordVersion), ArchiveError> {
        use crate::archive_codec::publication::{self, ArchivePublication};
        let payload = container.read(version.record)?;
        if let Some(envelope) = crate::container::publication_envelope::decode(&payload)? {
            if envelope.global_version != version.global_version {
                return Err(ArchiveError::InvalidArchiveRecordVersion);
            }
            match publication::decode(envelope.body, envelope.global_version)? {
                Some(ArchivePublication::Record {
                    archive_version,
                    record,
                }) if archive_version == version.archive_version => return Ok((record, version)),
                Some(ArchivePublication::Repeat {
                    archive_version,
                    first_applying_archive_version,
                }) if archive_version == version.archive_version => {
                    let first = *self
                        .record_version(first_applying_archive_version)
                        .ok_or(ArchiveError::InvalidArchiveRecordVersion)?;
                    let (record, resolved) = self.read_publication(container, first)?;
                    if resolved.archive_version != first_applying_archive_version {
                        return Err(ArchiveError::InvalidArchiveRecordVersion);
                    }
                    return Ok((record, resolved));
                }
                _ => return Err(ArchiveError::InvalidArchiveRecordVersion),
            }
        }
        let first = self
            .record_versions
            .iter()
            .find(|entry| entry.record == version.record)
            .copied()
            .ok_or(ArchiveError::InvalidArchiveRecordVersion)?;
        Ok((decode_record(&payload)?, first))
    }

    pub(crate) fn branch_at(
        &self,
        container: &mut Container,
        conversation_id: &str,
        branch_id: &str,
        archive_version: u64,
    ) -> Result<Option<Branch>, ArchiveError> {
        let mut found = None;
        for version in &self.record_versions {
            if version.archive_version > archive_version {
                break;
            }
            let (record, applying) = self.read_publication(container, *version)?;
            if applying.archive_version != version.archive_version {
                continue;
            }
            if let ArchiveRecord::Branch(branch) = record
                && branch.conversation_id == conversation_id
                && branch.id == branch_id
            {
                found = Some(branch);
            }
        }
        Ok(found)
    }

    pub(crate) fn prepare_publication(
        &self,
        container: &Container,
        record: ArchiveRecord,
    ) -> Result<PreparedArchivePublication, ArchiveError> {
        container.require_storage_format_writable()?;
        let archive_version = self.next_archive_version;
        let next_archive_version = archive_version
            .checked_add(1)
            .ok_or(ArchiveError::ArchiveVersionExhausted)?;
        let global_version = container
            .latest_version()
            .checked_add(1)
            .ok_or(crate::ContainerError::VersionExhausted)?;
        global_version
            .checked_add(1)
            .ok_or(crate::ContainerError::VersionExhausted)?;
        let body = crate::archive_codec::publication::encode(
            &crate::archive_codec::publication::ArchivePublication::Record {
                archive_version,
                record,
            },
        )?;
        Ok(PreparedArchivePublication {
            body,
            archive_version,
            next_archive_version,
            global_version,
        })
    }

    pub(crate) fn publish_prepared(
        &mut self,
        container: &mut Container,
        prepared: PreparedArchivePublication,
    ) -> Result<ArchiveRecordVersion, ArchiveError> {
        if self.next_archive_version != prepared.archive_version
            || container.latest_version().checked_add(1) != Some(prepared.global_version)
        {
            return Err(ArchiveError::InvalidArchiveRecordVersion);
        }
        let (global_version, record) = container.append_publication_envelope(&prepared.body)?;
        let version = ArchiveRecordVersion {
            global_version,
            archive_version: prepared.archive_version,
            record,
        };
        self.record_versions.push(version);
        self.next_archive_version = prepared.next_archive_version;
        Ok(version)
    }

    pub(crate) fn publish_publication(
        &mut self,
        container: &mut Container,
        record: ArchiveRecord,
    ) -> Result<ArchiveRecordVersion, ArchiveError> {
        let prepared = self.prepare_publication(container, record)?;
        self.publish_prepared(container, prepared)
    }

    /// Explicit standalone fixtures retain the old representation for parity
    /// tests; ordinary writers use the native prepared publication path.
    #[cfg(test)]
    pub(crate) fn publish_record(
        &mut self,
        container: &mut Container,
        record: ObjectRef,
    ) -> Result<ArchiveRecordVersion, ArchiveError> {
        container.require_storage_format_writable()?;
        let archive_version = self.next_archive_version;
        let next_archive_version = archive_version
            .checked_add(1)
            .ok_or(ArchiveError::ArchiveVersionExhausted)?;
        let global_version = container.allocate_version()?;
        let version = ArchiveRecordVersion {
            global_version,
            archive_version,
            record,
        };
        container.append(&encode_record_version(version))?;
        self.record_versions.push(version);
        self.next_archive_version = next_archive_version;
        Ok(version)
    }
}
