use super::{
    CHUNK_HEADER_LEN, ChunkRef, Container, ContainerError, ObjectRef, read_header, truncated_or_io,
};
use crate::container_version::observe_version_payload_state;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

impl Container {
    /// Opens a read-only handle for bounded streaming queries.
    ///
    /// Unlike normal open, this does not scan, repair a truncated tail, or
    /// initialize the in-memory version index. Use it only with read APIs.
    pub(crate) fn open_read_only_for_scan(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        let path = path.as_ref();
        let mut file = OpenOptions::new().read(true).open(path)?;
        let (version, identity, owner_uuid, header_len) = read_header(&mut file)?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
            version,
            identity,
            owner_uuid,
            header_len,
            next_version: 1,
            transaction_times: BTreeMap::new(),
            next_transaction_time_override: None,
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        Self::open_scanned(path, |_, _, _| Ok(()))
    }

    pub(crate) fn open_scanned<E>(
        path: impl AsRef<Path>,
        mut visitor: impl FnMut(ObjectRef, &[u8], u64) -> Result<(), E>,
    ) -> Result<Self, E>
    where
        E: From<ContainerError>,
    {
        let path = path.as_ref();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(ContainerError::Io)?;
        let (version, identity, owner_uuid, header_len) =
            read_header(&mut file).map_err(E::from)?;
        let mut container = Self {
            file,
            path: path.to_path_buf(),
            version,
            identity,
            owner_uuid,
            header_len,
            next_version: 1,
            transaction_times: BTreeMap::new(),
            next_transaction_time_override: None,
        };
        container.scan_payloads(&mut visitor)?;
        Ok(container)
    }

    fn scan_payloads<E>(
        &mut self,
        visitor: &mut impl FnMut(ObjectRef, &[u8], u64) -> Result<(), E>,
    ) -> Result<(), E>
    where
        E: From<ContainerError>,
    {
        const SCAN_BUFFER_BYTES: usize = 64 * 1024;

        let file_len = self.file.metadata().map_err(ContainerError::Io)?.len();
        let mut offset = self.header_len;
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(ContainerError::Io)?;

        let mut truncate_at = None;
        {
            let file = &mut self.file;
            let next_version = &mut self.next_version;
            let transaction_times = &mut self.transaction_times;
            let mut reader = BufReader::with_capacity(SCAN_BUFFER_BYTES, file);
            let mut length_bytes = [0_u8; CHUNK_HEADER_LEN as usize];
            let mut payload = Vec::new();

            while offset < file_len {
                if file_len - offset < CHUNK_HEADER_LEN {
                    truncate_at = Some(offset);
                    break;
                }
                reader
                    .read_exact(&mut length_bytes)
                    .map_err(|error| E::from(truncated_or_io(error, offset)))?;
                let len = u64::from_le_bytes(length_bytes);
                let end = offset
                    .checked_add(CHUNK_HEADER_LEN)
                    .and_then(|value| value.checked_add(len))
                    .ok_or_else(|| E::from(ContainerError::ChunkTooLarge))?;
                if end > file_len {
                    truncate_at = Some(offset);
                    break;
                }
                let payload_len =
                    usize::try_from(len).map_err(|_| E::from(ContainerError::ChunkTooLarge))?;
                payload.resize(payload_len, 0);
                reader
                    .read_exact(&mut payload)
                    .map_err(|error| E::from(truncated_or_io(error, offset)))?;

                let object = ObjectRef::from_chunk(ChunkRef { offset, len });
                observe_version_payload_state(next_version, transaction_times, &payload)
                    .map_err(E::from)?;
                visitor(object, &payload, next_version.saturating_sub(1))?;
                offset = end;
            }
        }

        if let Some(offset) = truncate_at {
            self.recover_truncated_tail(offset).map_err(E::from)?;
        }
        Ok(())
    }

    fn recover_truncated_tail(&mut self, offset: u64) -> Result<(), ContainerError> {
        self.file.set_len(offset)?;
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.sync_all()?;
        Ok(())
    }
}
