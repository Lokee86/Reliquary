use super::*;

pub(super) fn validate_receipt(entry: &Entry, receipt: &ArchiveReceipt) -> Result<(), QueueError> {
    let capture = entry
        .finalize_capture
        .as_ref()
        .ok_or(QueueError::Corrupt("missing finalize capture"))?;
    if receipt.node_id != entry.submission.id.submission_id.as_str()
        || receipt.branch_id != capture.branch_id
        || receipt.parent_node_id != capture.parent_node_id
        || receipt.principal_id != capture.verified_principal_id
        || receipt.committed_timestamp_ns != capture.committed_timestamp_ns
        || receipt.payload_hash != entry.submission.payload_hash
    {
        return Err(QueueError::ArchiveIdentityConflict);
    }
    Ok(())
}

pub(super) fn validate_host_operation_transition(
    previous: Option<&DurableHostOperation>,
    operation: &DurableHostOperation,
) -> Result<(), QueueError> {
    validate_id(&operation.stable_id, "host operation id")?;
    match previous {
        None => {
            if operation.transition_revision != 1
                || operation.phase != DurableHostOperationPhase::HandoffIntent
                || operation.cancel_requested
                || operation.last_known_phase.is_some()
            {
                return Err(QueueError::Corrupt("invalid initial host operation state"));
            }
        }
        Some(old) => {
            if old.stable_id != operation.stable_id
                || old.payload_digest != operation.payload_digest
                || Some(operation.transition_revision) != old.transition_revision.checked_add(1)
                || (!operation.cancel_requested && old.cancel_requested)
                || (matches!(
                    operation.phase,
                    DurableHostOperationPhase::CancelIntent
                        | DurableHostOperationPhase::RunningCancellationRequested
                ) && !operation.cancel_requested)
                || !host_phase_transition_allowed(&old.phase, &operation.phase)
            {
                return Err(QueueError::Corrupt("invalid host operation transition"));
            }
            let expected_last_known = if operation.phase == DurableHostOperationPhase::Held {
                Some(
                    old.last_known_phase
                        .clone()
                        .unwrap_or_else(|| old.phase.clone()),
                )
            } else {
                Some(operation.phase.clone())
            };
            if operation.last_known_phase != expected_last_known {
                return Err(QueueError::Corrupt(
                    "invalid last-known host operation phase",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn host_phase_transition_allowed(
    old: &DurableHostOperationPhase,
    new: &DurableHostOperationPhase,
) -> bool {
    use DurableHostOperationPhase::*;
    match old {
        HandoffIntent => matches!(
            new,
            Accepted | Pending | Running | CancelIntent | Cancelled | Completed | Held
        ),
        Accepted => matches!(
            new,
            Pending | Running | CancelIntent | Cancelled | Completed | Held
        ),
        Pending => matches!(new, Running | CancelIntent | Cancelled | Completed | Held),
        Running => matches!(
            new,
            CancelIntent | RunningCancellationRequested | Completed | Held
        ),
        CancelIntent => matches!(
            new,
            Pending | Running | RunningCancellationRequested | Cancelled | Completed | Held
        ),
        RunningCancellationRequested => matches!(
            new,
            CancelIntent | RunningCancellationRequested | Cancelled | Completed | Held
        ),
        Held => matches!(
            new,
            Accepted
                | Pending
                | Running
                | CancelIntent
                | RunningCancellationRequested
                | Cancelled
                | Completed
        ),
        Cancelled | Completed => false,
    }
}

pub(super) fn validate_generation_update(
    previous: Option<&DurableGeneration>,
    next: &DurableGeneration,
) -> Result<Option<DurableGenerationCheckpoint>, QueueError> {
    for (value, kind) in [
        (&next.operation_id, "generation operation id"),
        (&next.generation_id, "generation id"),
        (&next.branch_id, "branch id"),
        (&next.parent_node_id, "parent node id"),
    ] {
        validate_id(value, kind)?;
    }
    match previous {
        None => {
            if next.transition_revision != 1
                || next.stream_cursor != 0
                || !matches!(
                    next.phase,
                    DurableGenerationPhase::Pending | DurableGenerationPhase::Running
                )
                || next.checkpoint.is_some()
            {
                return Err(QueueError::Corrupt("invalid initial generation state"));
            }
            Ok(None)
        }
        Some(old) => {
            if old.phase.terminal()
                || old.operation_id != next.operation_id
                || old.generation_id != next.generation_id
                || old.branch_id != next.branch_id
                || old.parent_node_id != next.parent_node_id
                || old.started_timestamp_ns != next.started_timestamp_ns
                || Some(next.transition_revision) != old.transition_revision.checked_add(1)
                || Some(next.stream_cursor) != old.stream_cursor.checked_add(1)
                || (old.phase != next.phase
                    && !generation_phase_transition_allowed(old.phase, next.phase))
            {
                return Err(QueueError::Corrupt(
                    "generation identity or phase transition",
                ));
            }
            match (&old.checkpoint, &next.checkpoint) {
                (None, None) => Ok(None),
                (None, Some(checkpoint))
                    if checkpoint.sequence == 1 && checkpoint.delivery_cursor == 1 =>
                {
                    validate_checkpoint(checkpoint, None)?;
                    Ok(Some(checkpoint.clone()))
                }
                (Some(previous), Some(checkpoint)) if checkpoint == previous => Ok(None),
                (Some(previous), Some(checkpoint))
                    if Some(checkpoint.sequence) == previous.sequence.checked_add(1)
                        && Some(checkpoint.delivery_cursor)
                            == previous.delivery_cursor.checked_add(1)
                        && checkpoint.utf8_bytes >= previous.utf8_bytes
                        && checkpoint
                            .content_prefix
                            .starts_with(&previous.content_prefix) =>
                {
                    validate_checkpoint(checkpoint, Some(previous))?;
                    Ok(Some(checkpoint.clone()))
                }
                _ => Err(QueueError::Corrupt(
                    "invalid generation checkpoint transition",
                )),
            }
        }
    }
}

pub(super) fn validate_checkpoint(
    checkpoint: &DurableGenerationCheckpoint,
    previous: Option<&DurableGenerationCheckpoint>,
) -> Result<(), QueueError> {
    let bytes = checkpoint.content_prefix.len();
    let digest: [u8; 32] = Sha256::digest(checkpoint.content_prefix.as_bytes()).into();
    if checkpoint.sequence == 0
        || checkpoint.delivery_cursor == 0
        || bytes > MAX_CHECKPOINT_TEXT_BYTES
        || checkpoint.utf8_bytes != bytes as u64
        || digest != checkpoint.sha256
        || previous.is_some_and(|old| !checkpoint.content_prefix.starts_with(&old.content_prefix))
    {
        return Err(QueueError::Corrupt(
            "checkpoint content does not match durable identity",
        ));
    }
    Ok(())
}

pub(super) fn checkpoint_history_bytes(history: &VecDeque<DurableGenerationCheckpoint>) -> usize {
    history
        .iter()
        .map(|checkpoint| checkpoint.content_prefix.len())
        .sum()
}

pub(super) fn generation_event_history_bytes(history: &VecDeque<GenerationStreamRecord>) -> usize {
    history
        .iter()
        .filter_map(|event| event.checkpoint.as_ref())
        .map(|checkpoint| checkpoint.content_prefix.len())
        .sum()
}

pub(super) fn generation_phase_transition_allowed(
    old: DurableGenerationPhase,
    new: DurableGenerationPhase,
) -> bool {
    use DurableGenerationPhase::*;
    match old {
        Pending => matches!(
            new,
            Running | CancellationRequested | Interrupted | Held | Unknown | Completed | Cancelled
        ),
        Running => matches!(
            new,
            CancellationRequested | Interrupted | Held | Unknown | Completed
        ),
        CancellationRequested => {
            matches!(new, Interrupted | Held | Unknown | Completed | Cancelled)
        }
        Held | Unknown => matches!(
            new,
            Pending | Running | CancellationRequested | Interrupted | Completed | Cancelled
        ),
        Interrupted | Completed | Cancelled => false,
    }
}

pub(super) fn validate_id(value: &str, kind: &'static str) -> Result<(), QueueError> {
    if value.is_empty()
        || value.len() > 1024
        || value.bytes().any(|b| b == 0 || b.is_ascii_control())
    {
        Err(QueueError::InvalidIdentifier(kind))
    } else {
        Ok(())
    }
}
pub(super) fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
pub(super) fn uuid_filename(bytes: [u8; 16]) -> String {
    let raw = hex(&bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &raw[0..8],
        &raw[8..12],
        &raw[12..16],
        &raw[16..20],
        &raw[20..32]
    )
}
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(super) fn owner_header(owner: OwnerId) -> Vec<u8> {
    let mut header = Vec::with_capacity(HEADER_BYTES as usize);
    header.extend_from_slice(JOURNAL_MAGIC);
    header.extend_from_slice(&owner.as_bytes());
    let checksum = sha256(&header);
    header.extend_from_slice(&checksum);
    header
}
pub(super) fn recover(file: &mut File, expected: OwnerId) -> Result<(Projection, u64), QueueError> {
    file.seek(SeekFrom::Start(0))?;
    let length = file.metadata()?.len();
    if length < HEADER_BYTES {
        return Err(QueueError::Corrupt("short owner header"));
    }
    let mut magic = [0; 8];
    file.read_exact(&mut magic)?;
    if &magic != JOURNAL_MAGIC {
        return Err(QueueError::Corrupt("unknown journal header"));
    }
    let mut owner = [0; 16];
    file.read_exact(&mut owner)?;
    if owner != expected.as_bytes() {
        return Err(QueueError::OwnerMismatch);
    }
    let mut checksum = [0; CHECKSUM_BYTES];
    file.read_exact(&mut checksum)?;
    let mut header = Vec::with_capacity(24);
    header.extend_from_slice(&magic);
    header.extend_from_slice(&owner);
    if sha256(&header) != checksum {
        return Err(QueueError::Corrupt("owner header checksum"));
    }
    let mut projection = Projection::empty(expected);
    let mut position = HEADER_BYTES;
    let mut good_end = position;
    while position < length {
        file.seek(SeekFrom::Start(position))?;
        let mut len_bytes = [0; 4];
        if let Err(error) = file.read_exact(&mut len_bytes) {
            if error.kind() == std::io::ErrorKind::UnexpectedEof {
                break;
            }
            return Err(error.into());
        }
        let frame_len = u32::from_le_bytes(len_bytes) as usize;
        if frame_len == 0 || frame_len > FRAME_LIMIT {
            return Err(QueueError::Corrupt("invalid complete frame length"));
        }
        let end = position
            .checked_add(4 + frame_len as u64 + CHECKSUM_BYTES as u64)
            .ok_or(QueueError::Corrupt("frame length overflow"))?;
        if end > length {
            break;
        }
        let mut data = vec![0; frame_len];
        file.read_exact(&mut data)?;
        let mut got = [0; CHECKSUM_BYTES];
        file.read_exact(&mut got)?;
        if sha256(&data) != got {
            return Err(QueueError::Corrupt("complete frame checksum"));
        }
        projection.apply(&serde_json::from_slice::<Frame>(&data)?)?;
        position = end;
        good_end = end;
    }
    Ok((projection, good_end))
}
pub(super) fn sync_dir(path: &Path) -> Result<(), QueueError> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()?;
        Ok(())
    }
    #[cfg(windows)]
    {
        use std::{os::windows::ffi::OsStrExt, ptr::null_mut};
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn CreateFileW(
                name: *const u16,
                access: u32,
                share: u32,
                security: *mut core::ffi::c_void,
                creation: u32,
                flags: u32,
                template: *mut core::ffi::c_void,
            ) -> *mut core::ffi::c_void;
            fn FlushFileBuffers(handle: *mut core::ffi::c_void) -> i32;
            fn CloseHandle(handle: *mut core::ffi::c_void) -> i32;
        }
        let path = fs::canonicalize(path)?;
        let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let handle = unsafe {
            CreateFileW(
                name.as_ptr(),
                0x40000000,
                7,
                null_mut(),
                3,
                0x02000000,
                null_mut(),
            )
        };
        if handle as isize == -1 {
            return Err(std::io::Error::last_os_error().into());
        }
        let result = unsafe { FlushFileBuffers(handle) };
        let error = (result == 0).then(std::io::Error::last_os_error);
        unsafe {
            CloseHandle(handle);
        }
        if let Some(error) = error {
            return Err(error.into());
        }
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(QueueError::UnsupportedDurability)
    }
}
