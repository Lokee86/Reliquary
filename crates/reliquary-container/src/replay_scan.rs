//! Physical-order scans selected by payload prefix on the current lease.

use super::{Container, ContainerError, FrameScan, ObjectRef, PhysicalClock, PhysicalCursor};

// Covers the physical publication header and a nested owner's record marker.
pub(super) const REPLAY_PREFIX_BYTES: usize = 64;
const MAX_DIRECTORY_ENTRIES: u64 = 32 * 1024;

pub(super) struct ReplayEntry {
    pub(super) object: ObjectRef,
    pub(super) prefix: [u8; REPLAY_PREFIX_BYTES],
}

#[derive(Default)]
pub(super) struct ReplayDirectory {
    extent: u64,
    complete: bool,
    max_length: u64,
    min_length: u64,
    entries: Vec<ReplayEntry>,
}

impl ReplayDirectory {
    pub(super) fn new(extent: u64, complete: bool) -> Self {
        Self {
            extent,
            complete,
            max_length: 0,
            // Index the expensive skips. Small adjacent records share a read
            // buffer. The byte threshold bounds the directory on any input.
            min_length: extent.div_ceil(MAX_DIRECTORY_ENTRIES).max(1024),
            entries: Vec::new(),
        }
    }

    pub(super) fn record(&mut self, object: ObjectRef, bytes: &[u8]) {
        self.max_length = self.max_length.max(object.length);
        if object.length >= self.min_length {
            self.entries.push(ReplayEntry {
                object,
                prefix: bytes[..REPLAY_PREFIX_BYTES].try_into().unwrap(),
            });
        }
    }
}

/// A selected record and the physical clock frontier immediately after it.
pub struct ClockedScannedFrame<'a> {
    pub object: ObjectRef,
    pub payload: &'a [u8],
    pub global_version: u64,
}

pub struct ClockedFrameScan<'a> {
    cursor: PhysicalCursor<'a>,
    clock: PhysicalClock,
    payload: Vec<u8>,
    select: fn(&[u8]) -> bool,
}

impl ClockedFrameScan<'_> {
    pub fn next_frame(&mut self) -> Result<Option<ClockedScannedFrame<'_>>, ContainerError> {
        while let Some(object) = self
            .cursor
            .read_selected_frame_into(&mut self.payload, Some(&mut self.clock))?
        {
            let global_version = self.clock.observe_frame(&self.payload)?;
            let prefix_len = self.payload.len().min(REPLAY_PREFIX_BYTES);
            if (self.select)(&self.payload[..prefix_len]) {
                return Ok(Some(ClockedScannedFrame {
                    object,
                    payload: &self.payload,
                    global_version,
                }));
            }
        }
        Ok(None)
    }
}

pub(super) fn needs_payload(
    select: fn(&[u8]) -> bool,
    prefix: &[u8],
    clock: Option<&mut PhysicalClock>,
) -> Result<bool, ContainerError> {
    if select(prefix) {
        return Ok(true);
    }
    if let Some(clock) = clock {
        if super::version_clock::needs_full_payload(prefix) {
            return Ok(true);
        }
        clock.observe_frame(prefix)?;
    }
    Ok(false)
}

impl Container {
    /// Maximum payload length in the current file, including unselected records.
    pub fn max_frame_length(&self) -> Result<u64, ContainerError> {
        if self.replay_directory.complete && self.replay_directory.extent == self.extent()? {
            return Ok(self.replay_directory.max_length);
        }
        let mut max_length = 0;
        let mut cursor = self.physical_cursor()?;
        while let Some(frame) = cursor.next_frame()? {
            max_length = max_length.max(frame.object().length);
        }
        Ok(max_length)
    }

    /// Select payloads while advancing the same physical clock through every
    /// frame, including reservations and publications belonging to other stores.
    pub fn scan_clocked_matching_frames(
        &self,
        select: fn(&[u8]) -> bool,
    ) -> Result<ClockedFrameScan<'_>, ContainerError> {
        Ok(ClockedFrameScan {
            cursor: self.matching_physical_cursor(select)?,
            clock: PhysicalClock::default(),
            payload: Vec::new(),
            select,
        })
    }

    /// Stream selected frames in physical order. The selector sees the first
    /// 64 payload bytes, or the complete payload if shorter. A match yields
    /// the original full payload and object reference.
    pub fn matching_physical_cursor(
        &self,
        select: fn(&[u8]) -> bool,
    ) -> Result<PhysicalCursor<'_>, ContainerError> {
        let mut cursor = self.physical_cursor()?;
        cursor.select = Some(select);
        if cursor.file_len == self.replay_directory.extent {
            cursor.directory = &self.replay_directory.entries;
        }
        Ok(cursor)
    }

    /// Read only the selected frame payloads into the reusable scan buffer.
    pub fn scan_matching_frames(
        &self,
        select: fn(&[u8]) -> bool,
    ) -> Result<FrameScan<'_>, ContainerError> {
        Ok(FrameScan {
            cursor: self.matching_physical_cursor(select)?,
            payload: Vec::new(),
        })
    }

    /// Release transient frame prefixes after rebuilding a mounted session.
    pub fn release_replay_directory(&mut self) {
        self.replay_directory = ReplayDirectory::default();
    }
}
