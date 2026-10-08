use super::*;
use crate::{FileId, ProjectRevisionCorrelation, ProjectRevisionRef, StoredFile};
use std::path::Path;

impl WorkspaceManagement {
    /// Inventory of one exact source turn, including repository-backed references.
    pub fn source_files(
        &self,
        owner: &str,
        conversation: &str,
        message: &str,
        offset: usize,
        limit: usize,
    ) -> Result<ManagementPage<SourceFile>, ManagementError> {
        validate_page(offset, limit)?;
        let token = self.token(owner)?;
        let mut files = self
            .gateway
            .files_for_source_for(&self.instance, owner, conversation, message)
            .map_err(unavailable)?;
        files.sort_by_key(|f| f.id.0);
        let inventory = page(files, offset, limit);
        let mut values = Vec::with_capacity(inventory.items.len());
        for file in inventory.items {
            let project_reference = self
                .gateway
                .project_file_ref_for(&self.instance, owner, file.id)
                .map_err(unavailable)?;
            values.push(SourceFile {
                file,
                project_reference,
            });
        }
        text_bound(
            values
                .iter()
                .map(|x| {
                    x.file.filename.len()
                        + x.file.mime_type.as_ref().map_or(0, String::len)
                        + x.project_reference.as_ref().map_or(0, |r| {
                            r.path.len()
                                + r.revision.revision.len()
                                + r.revision.repository.repository_id.len()
                                + r.revision.repository.project_path.len()
                        })
                })
                .sum(),
        )?;
        self.deliver(
            &token,
            ManagementPage {
                items: values,
                total_items: inventory.total_items,
                next_offset: inventory.next_offset,
            },
        )
    }
    pub fn project(
        &self,
        owner: &str,
    ) -> Result<Option<ProjectRevisionCorrelation>, ManagementError> {
        let token = self.token(owner)?;
        let value = self
            .gateway
            .latest_project_revision_correlation_for(&self.instance, owner)
            .map_err(unavailable)?;
        self.deliver(&token, value)
    }
    /// Native bytes or the exact repository revision, never arbitrary filesystem reads.
    /// The lower file owner may allocate the complete file before this response cap.
    pub fn read_file(
        &self,
        owner: &str,
        file: FileId,
        project_dir: Option<&Path>,
    ) -> Result<Vec<u8>, ManagementError> {
        let token = self.token(owner)?;
        let bytes = self
            .gateway
            .resolved_file_bytes_for(&self.instance, owner, project_dir, file)
            .map_err(unavailable)?;
        if bytes.len() > MAX_MANAGEMENT_FILE_BYTES {
            return Err(ManagementError::ResponseTooLarge);
        }
        self.deliver(&token, bytes)
    }
    pub fn add_project_attachment(
        &self,
        owner: &str,
        project_dir: &Path,
        source: &Path,
    ) -> Result<StoredFile, ManagementError> {
        self.gateway
            .ingest_project_attachment_for(&self.instance, owner, project_dir, source)
            .map_err(unavailable)
    }
    pub fn validate_project(
        &self,
        owner: &str,
        project_dir: &Path,
    ) -> Result<ProjectRevisionRef, ManagementError> {
        self.gateway
            .validate_project_environment_for(&self.instance, owner, project_dir)
            .map_err(unavailable)
    }
    pub fn checkpoint_project(
        &self,
        owner: &str,
        project_dir: &Path,
    ) -> Result<ProjectRevisionRef, ManagementError> {
        self.gateway
            .checkpoint_manual_project_for(&self.instance, owner, project_dir)
            .map_err(unavailable)
    }
}
