use super::*;

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) enum Payload<T> {
    Resident(T),
    Leased(FrameRange),
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(super) struct FrameRange {
    frame: ObjectRef,
    offset: u64,
    length: usize,
}

impl FrameRange {
    pub(super) fn new(frame: ObjectRef, offset: u64, length: usize) -> Self {
        Self {
            frame,
            offset,
            length,
        }
    }
}

impl<T> Payload<T> {
    fn len(&self, resident_len: impl FnOnce(&T) -> usize) -> usize {
        match self {
            Self::Resident(value) => resident_len(value),
            Self::Leased(range) => range.length,
        }
    }
}

impl Archive {
    pub(super) fn frame_range(
        &self,
        source: Option<(ObjectRef, &[u8])>,
        bytes: &[u8],
    ) -> Result<Option<FrameRange>, ArchiveError> {
        if self.reader.is_none() {
            return Ok(None);
        }
        let (frame, payload) =
            source.ok_or(ArchiveError::CorruptRecord("Archive payload has no frame"))?;
        let offset = bytes
            .as_ptr()
            .addr()
            .checked_sub(payload.as_ptr().addr())
            .filter(|offset| {
                bytes.len() <= payload.len().saturating_sub(*offset) && *offset <= payload.len()
            })
            .ok_or(ArchiveError::CorruptRecord(
                "Archive payload is outside its frame",
            ))?;
        Ok(Some(FrameRange::new(frame, offset as u64, bytes.len())))
    }

    pub(super) fn text_length(&self, id: ContentId) -> Option<usize> {
        self.content
            .get(&id)
            .map(|payload| payload.len(|text| text.len()))
    }

    pub(super) fn read_text(&self, id: ContentId) -> Result<Option<Arc<String>>, ArchiveError> {
        self.content
            .get(&id)
            .map(|payload| match payload {
                Payload::Resident(text) => Ok(Arc::clone(text)),
                Payload::Leased(range) => {
                    let bytes = self.read_range(*range, id)?;
                    String::from_utf8(bytes).map(Arc::new).map_err(|_| {
                        if let Some(reader) = &self.reader {
                            reader.fence_writes();
                        }
                        ArchiveError::InvalidUtf8
                    })
                }
            })
            .transpose()
    }

    pub(super) fn read_file(&self, id: ContentId) -> Result<Option<Vec<u8>>, ArchiveError> {
        self.file_content
            .get(&id)
            .map(|payload| match payload {
                Payload::Resident(bytes) => Ok(bytes.clone()),
                Payload::Leased(range) => self.read_range(*range, id),
            })
            .transpose()
    }

    pub(super) fn validate_replayed_attachments(
        &self,
        attachments: &[SourceAttachment],
        allow_retained_project_identity: bool,
    ) -> Result<(), ArchiveError> {
        validate_source_attachments(attachments, allow_retained_project_identity, |attachment| {
            let file = self
                .file_content
                .get(&attachment.content_id)
                .ok_or(ArchiveError::MissingContent)?;
            if file.len(Vec::len) as u64 != attachment.byte_length {
                return Err(ArchiveError::InvalidAttachment);
            }
            Ok(())
        })
    }

    fn read_range(&self, range: FrameRange, id: ContentId) -> Result<Vec<u8>, ArchiveError> {
        let reader = self.reader.as_ref().ok_or(ArchiveError::CorruptRecord(
            "Archive read has no owning lease",
        ))?;
        Ok(reader.read_frame_range(range.frame, range.offset, range.length as u64, id.0)?)
    }
}
