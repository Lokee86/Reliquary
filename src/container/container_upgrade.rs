//! Physical frame transport only. Semantic migration belongs to owner codecs.
use super::{Container, ContainerError};
use std::io::{Read, Seek, SeekFrom};

impl Container {
    pub(crate) fn copy_upgrade_shell_frames(
        &mut self,
        destination: &mut Self,
    ) -> Result<(), ContainerError> {
        if self.version.major != 1
            || self.version.minor != 0
            || destination.version != super::COMPACT_VERSION
            || self.header_len != destination.header_len
            || self.identity != destination.identity
            || self.owner_uuid != destination.owner_uuid
        {
            return Err(ContainerError::InvalidIdentity);
        }
        destination.require_write_admission()?;
        if destination.file.metadata()?.len() != destination.header_len {
            return Err(ContainerError::InvalidIdentity);
        }
        let retained_bytes = self
            .file
            .metadata()?
            .len()
            .checked_sub(self.header_len)
            .ok_or(ContainerError::InvalidIdentity)?;
        self.file.seek(SeekFrom::Start(self.header_len))?;
        destination
            .file
            .seek(SeekFrom::Start(destination.header_len))?;
        let copied = std::io::copy(
            &mut (&mut self.file).take(retained_bytes),
            &mut destination.file,
        )?;
        if copied != retained_bytes {
            return Err(ContainerError::TruncatedChunk(self.header_len + copied));
        }
        // Never allocate tickets or timestamps when carrying accepted frames.
        destination.next_version = self.next_version;
        destination.transaction_times = self.transaction_times.clone();
        Ok(())
    }
}
