use crate::owner_lifecycle::{OwnerError, ReliquaryOwner};
use crate::{ContentId, ObjectRef};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectAttachmentRepackResult {
    pub copied_chunks: usize,
    pub dropped_content_objects: usize,
    pub dropped_payload_bytes: u64,
    pub source_file_bytes: u64,
    pub output_file_bytes: u64,
}

impl ReliquaryOwner {
    /// Copy this REL into a compact container while omitting embedded content
    /// blobs that are now fully backed by ProjectFileRef bindings.
    ///
    /// All retained durable payloads are copied unchanged except the
    /// version-record families that encode physical ObjectRef locations. Those
    /// references are relocated to their corresponding copied chunks.
    pub(crate) fn repack_project_backed_attachments(
        &mut self,
        output_path: impl AsRef<Path>,
    ) -> Result<ProjectAttachmentRepackResult, OwnerError> {
        let output_path = output_path.as_ref();
        self.container.sync()?;
        let source_file_bytes = fs::metadata(self.container.path())
            .map_err(|error| OwnerError::Repack(error.to_string()))?
            .len();

        let removable = self.removable_project_content_ids();
        if removable.is_empty() {
            return Err(OwnerError::Repack(
                "no repository-backed embedded attachment content is removable".into(),
            ));
        }

        let payloads = self.container.object_payloads()?;
        let mut output = self.container.create_empty_like(output_path)?;
        let output_guard = output.read_source()?;
        let mut relocated = BTreeMap::<ObjectRef, ObjectRef>::new();
        let mut copied_chunks = 0_usize;
        let mut dropped_content_objects = 0_usize;
        let mut dropped_payload_bytes = 0_u64;

        let copy_result = (|| {
            for (source_ref, payload) in payloads {
                if let Some(content_id) = crate::archive_codec::decode_content_id(&payload)?
                    && removable.contains(&content_id)
                {
                    dropped_content_objects += 1;
                    dropped_payload_bytes = dropped_payload_bytes
                        .checked_add(u64::try_from(payload.len()).map_err(|_| {
                            OwnerError::Repack("dropped payload length overflow".into())
                        })?)
                        .ok_or_else(|| {
                            OwnerError::Repack("dropped payload byte overflow".into())
                        })?;
                    continue;
                }

                let payload =
                    crate::owner_lifecycle::relocation::relocate_payload(&payload, &relocated)
                        .map_err(OwnerError::Repack)?;
                let destination_ref = output.append(&payload)?;
                relocated.insert(source_ref, destination_ref);
                copied_chunks += 1;
            }
            output.sync()?;
            Ok::<(), OwnerError>(())
        })();

        if let Err(error) = copy_result {
            drop(output);
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path);
            }
            return Err(error);
        }
        if dropped_content_objects != removable.len() {
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path);
            }
            return Err(OwnerError::Repack(format!(
                "expected to drop {} repository-backed content objects but dropped {dropped_content_objects}",
                removable.len()
            )));
        }

        let reopened =
            match crate::maintenance::reconciliation_owner_access::open_reconciliation_container(
                output,
            ) {
                Ok(reopened) => reopened,
                Err(error) => {
                    if output_guard.validate_path_identity().is_ok() {
                        let _ = fs::remove_file(output_path);
                    }
                    return Err(error);
                }
            };
        if let Err(error) = self.validate_historical_import_copy(&reopened) {
            drop(reopened);
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path);
            }
            return Err(error);
        }
        if reopened.owner_uuid() != self.owner_uuid()
            || reopened.latest_global_version() != self.latest_global_version()
            || reopened.archive.archive_version() != self.archive.archive_version()
            || !self.archive.activity_history_matches(&reopened.archive)
            || reopened.memories.memory_version() != self.memories.memory_version()
            || reopened.graph.graph_version() != self.graph.graph_version()
        {
            drop(reopened);
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path);
            }
            return Err(OwnerError::Repack(
                "repacked REL changed owner, semantic versions or activity history".into(),
            ));
        }
        drop(reopened);

        let output_file_bytes = fs::metadata(output_path)
            .map_err(|error| OwnerError::Repack(error.to_string()))?
            .len();
        if output_file_bytes >= source_file_bytes {
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path);
            }
            return Err(OwnerError::Repack(format!(
                "repacked REL did not shrink: source={source_file_bytes} output={output_file_bytes}"
            )));
        }

        Ok(ProjectAttachmentRepackResult {
            copied_chunks,
            dropped_content_objects,
            dropped_payload_bytes,
            source_file_bytes,
            output_file_bytes,
        })
    }

    fn removable_project_content_ids(&self) -> HashSet<ContentId> {
        let mut removable = HashSet::new();
        let mut protected = HashSet::new();

        for file in self.archive.files.iter() {
            if self.project_files.contains(file.id) {
                removable.insert(file.content_id);
            } else {
                protected.insert(file.content_id);
            }
        }
        for node in self.archive.nodes.iter() {
            protected.insert(node.content_id);
        }
        removable.retain(|content_id| !protected.contains(content_id));
        removable
    }
}
