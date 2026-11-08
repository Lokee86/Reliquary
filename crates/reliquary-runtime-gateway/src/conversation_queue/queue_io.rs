use super::*;

impl ConversationQueue {
    pub(crate) fn open(state_dir: impl AsRef<Path>, owner: OwnerId) -> Result<Self, QueueError> {
        let dir = state_dir.as_ref().join("conversation-queues");
        fs::create_dir_all(&dir)?;
        sync_dir(state_dir.as_ref())?;
        let path = dir.join(format!("{}.rqj", uuid_filename(owner.as_bytes())));
        let lock_path = path.with_extension("rqj.lock");
        let writer_lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(lock_path)?;
        writer_lock.try_lock().map_err(|error| {
            QueueError::Io(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                error.to_string(),
            ))
        })?;
        let existed = path.exists();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;
        if !existed {
            file.write_all(&owner_header(owner))?;
            file.sync_all()?;
            sync_dir(&dir)?;
        }
        let (projection, good_end) = recover(&mut file, owner)?;
        if file.metadata()?.len() > good_end {
            file.set_len(good_end)?;
            file.sync_all()?;
        }
        file.seek(SeekFrom::End(0))?;
        Ok(Self {
            owner,
            path,
            file,
            projection,
            failed: false,
            writer_lock: Some(writer_lock),
        })
    }

    pub(crate) fn open_read(
        state_dir: impl AsRef<Path>,
        owner: OwnerId,
    ) -> Result<Self, QueueError> {
        let path = state_dir
            .as_ref()
            .join("conversation-queues")
            .join(format!("{}.rqj", uuid_filename(owner.as_bytes())));
        let mut file = File::open(&path)?;
        let (projection, _) = recover(&mut file, owner)?;
        Ok(Self {
            owner,
            path,
            file,
            projection,
            failed: false,
            writer_lock: None,
        })
    }
    pub(super) fn check_revision(
        &self,
        conversation_id: &str,
        expected: u64,
    ) -> Result<(), QueueError> {
        let actual = self.conversation_revision(conversation_id);
        if actual == expected {
            Ok(())
        } else {
            Err(QueueError::StaleRevision { expected, actual })
        }
    }
    pub(super) fn ensure(&self) -> Result<(), QueueError> {
        if self.writer_lock.is_none() {
            return Err(QueueError::InvalidState);
        }
        if self.failed {
            Err(QueueError::RecoveryRequired)
        } else {
            Ok(())
        }
    }
    pub(super) fn append(&mut self, event: Event) -> Result<(), QueueError> {
        self.ensure()?;
        let sequence = self
            .projection
            .sequence
            .checked_add(1)
            .ok_or(QueueError::SequenceOverflow)?;
        let frame = Frame { sequence, event };
        let mut next = self.projection.clone();
        next.apply(&frame)?;
        let bytes = serde_json::to_vec(&frame)?;
        if bytes.is_empty() || bytes.len() > FRAME_LIMIT {
            return Err(QueueError::JournalFull);
        }
        if self
            .file
            .metadata()?
            .len()
            .saturating_add(bytes.len() as u64 + 4 + CHECKSUM_BYTES as u64)
            > JOURNAL_LIMIT
        {
            return Err(QueueError::JournalFull);
        }
        let mut raw = Vec::with_capacity(bytes.len() + 4 + CHECKSUM_BYTES);
        raw.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        raw.extend_from_slice(&bytes);
        raw.extend_from_slice(&sha256(&bytes));
        if let Err(error) = self.file.write_all(&raw).and_then(|_| self.file.sync_all()) {
            self.failed = true;
            return Err(QueueError::Io(error));
        }
        self.projection = next;
        Ok(())
    }
}
