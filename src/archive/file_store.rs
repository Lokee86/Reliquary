use crate::archive_codec::ArchiveRecord;
use crate::archive_store::{hash_content, validate_text};
use crate::{Archive, ArchiveError, Container, ContentId, FileId, StoredFile};
use sha2::{Digest, Sha256};

impl Archive {
    pub(crate) fn store_file(
        &mut self,
        container: &mut Container,
        filename: String,
        mime_type: Option<String>,
        bytes: &[u8],
    ) -> Result<StoredFile, ArchiveError> {
        container.require_storage_format_writable()?;
        let file = build_stored_file(filename, mime_type, bytes)?;
        if let Some(existing) = self.files.get(file.id) {
            return if existing == &file {
                Ok(existing.clone())
            } else {
                Err(ArchiveError::ConflictingFile)
            };
        }
        let prepared = self.prepare_publication(container, ArchiveRecord::File(file.clone()))?;
        self.put_content_bytes(container, file.content_id, bytes)?;
        self.publish_prepared(container, prepared)?;
        self.files.insert(file.clone())?;
        Ok(file)
    }

    pub(crate) fn register_file(
        &mut self,
        container: &mut Container,
        file: StoredFile,
    ) -> Result<StoredFile, ArchiveError> {
        container.require_storage_format_writable()?;
        if let Some(existing) = self.files.get(file.id) {
            return if existing == &file {
                Ok(existing.clone())
            } else {
                Err(ArchiveError::ConflictingFile)
            };
        }
        self.publish_publication(container, ArchiveRecord::File(file.clone()))?;
        self.files.insert(file.clone())?;
        Ok(file)
    }

    pub fn file(&self, id: FileId) -> Option<&StoredFile> {
        self.files.get(id)
    }

    pub fn files(&self) -> Vec<StoredFile> {
        self.files.iter().cloned().collect()
    }

    pub(crate) fn file_bytes(
        &self,
        container: &mut Container,
        id: FileId,
    ) -> Result<Vec<u8>, ArchiveError> {
        let file = self.files.get(id).ok_or(ArchiveError::MissingFile)?;
        let bytes = self.content_bytes(container, file.content_id)?;
        if bytes.len() as u64 != file.byte_length {
            return Err(ArchiveError::CorruptFile);
        }
        Ok(bytes)
    }

    pub(crate) fn validate_files(
        &self,
        project_files: &crate::project_file_binding_store::ProjectFileStore,
    ) -> Result<(), ArchiveError> {
        for file in self.files.iter() {
            self.validate_file(file, project_files)?;
        }
        Ok(())
    }

    fn validate_file(
        &self,
        file: &StoredFile,
        project_files: &crate::project_file_binding_store::ProjectFileStore,
    ) -> Result<(), ArchiveError> {
        validate_text(&file.filename, "filename")?;
        if let Some(value) = file.mime_type.as_deref() {
            validate_text(value, "mime type")?;
        }
        let legacy_id = file_id(
            &file.filename,
            file.mime_type.as_deref(),
            file.content_id,
            file.byte_length,
        );
        let valid_id = match project_files.get(file.id) {
            Some(reference) => {
                if reference.content_hash != Some(file.content_id.0) {
                    return Err(ArchiveError::CorruptFile);
                }
                let project_id = project_file_id(
                    &file.filename,
                    file.mime_type.as_deref(),
                    file.content_id,
                    file.byte_length,
                    reference,
                );
                file.id == project_id || file.id == legacy_id
            }
            None => file.id == legacy_id,
        };
        if !valid_id {
            return Err(ArchiveError::InvalidFileId);
        }
        if !self.contents.contains(file.content_id) && !project_files.contains(file.id) {
            return Err(ArchiveError::MissingContent);
        }
        Ok(())
    }
}

pub(crate) fn build_stored_file(
    filename: String,
    mime_type: Option<String>,
    bytes: &[u8],
) -> Result<StoredFile, ArchiveError> {
    let byte_length = u64::try_from(bytes.len()).map_err(|_| ArchiveError::FieldTooLarge)?;
    build_stored_file_from_content_id(filename, mime_type, hash_content(bytes), byte_length)
}

pub(crate) fn build_stored_file_from_content_id(
    filename: String,
    mime_type: Option<String>,
    content_id: ContentId,
    byte_length: u64,
) -> Result<StoredFile, ArchiveError> {
    build_stored_file_with_id(filename, mime_type, content_id, byte_length, None)
}

pub(crate) fn build_project_stored_file(
    filename: String,
    mime_type: Option<String>,
    content_id: ContentId,
    byte_length: u64,
    reference: &crate::ProjectFileRef,
) -> Result<StoredFile, ArchiveError> {
    build_stored_file_with_id(
        filename,
        mime_type,
        content_id,
        byte_length,
        Some(reference),
    )
}

fn build_stored_file_with_id(
    filename: String,
    mime_type: Option<String>,
    content_id: ContentId,
    byte_length: u64,
    project_ref: Option<&crate::ProjectFileRef>,
) -> Result<StoredFile, ArchiveError> {
    validate_text(&filename, "filename")?;
    if let Some(value) = mime_type.as_deref() {
        validate_text(value, "mime type")?;
    }
    let id = match project_ref {
        Some(reference) => project_file_id(
            &filename,
            mime_type.as_deref(),
            content_id,
            byte_length,
            reference,
        ),
        None => file_id(&filename, mime_type.as_deref(), content_id, byte_length),
    };
    Ok(StoredFile {
        id,
        content_id,
        filename,
        mime_type,
        byte_length,
    })
}

fn file_id(
    filename: &str,
    mime_type: Option<&str>,
    content_id: ContentId,
    byte_length: u64,
) -> FileId {
    let mime_type = mime_type.unwrap_or("");
    let mut hasher = Sha256::new();
    hasher.update(b"CVAFILE1-ID\0");
    hasher.update(content_id.0);
    hasher.update(byte_length.to_le_bytes());
    hasher.update((filename.len() as u64).to_le_bytes());
    hasher.update(filename.as_bytes());
    hasher.update((mime_type.len() as u64).to_le_bytes());
    hasher.update(mime_type.as_bytes());
    FileId(hasher.finalize().into())
}

fn project_file_id(
    filename: &str,
    mime_type: Option<&str>,
    content_id: ContentId,
    byte_length: u64,
    reference: &crate::ProjectFileRef,
) -> FileId {
    let mime_type = mime_type.unwrap_or("");
    let repository = &reference.revision.repository;
    let kind = match repository.kind {
        crate::ProjectRepositoryKind::Lore => 1_u8,
        crate::ProjectRepositoryKind::Git => 2_u8,
    };
    let mut hasher = Sha256::new();
    hasher.update(b"WARLOCK-PROJECT-FILE1-ID\0");
    hasher.update([kind]);
    hash_text(&mut hasher, &repository.repository_id);
    hash_text(&mut hasher, &repository.project_path);
    hash_text(&mut hasher, &reference.revision.revision);
    hash_text(&mut hasher, &reference.path);
    hasher.update(content_id.0);
    hasher.update(byte_length.to_le_bytes());
    hash_text(&mut hasher, filename);
    hash_text(&mut hasher, mime_type);
    FileId(hasher.finalize().into())
}

fn hash_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}
