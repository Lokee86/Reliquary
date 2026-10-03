use crate::{ContentId, Cva, CvaError, ObjectRef};
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

impl Cva {
    /// Copy this REL into a compact container while omitting embedded content
    /// blobs that are now fully backed by ProjectFileRef bindings.
    ///
    /// All retained durable payloads are copied unchanged except the four
    /// version-record families that encode physical ObjectRef locations. Those
    /// references are relocated to their corresponding copied chunks.
    pub fn repack_project_backed_attachments(
        &mut self,
        output_path: impl AsRef<Path>,
    ) -> Result<ProjectAttachmentRepackResult, CvaError> {
        let output_path = output_path.as_ref();
        self.container.sync()?;
        let source_file_bytes = fs::metadata(self.container.path())
            .map_err(|error| CvaError::Repack(error.to_string()))?
            .len();

        let removable = self.removable_project_content_ids();
        if removable.is_empty() {
            return Err(CvaError::Repack(
                "no repository-backed embedded attachment content is removable".into(),
            ));
        }

        let payloads = self.container.object_payloads()?;
        let mut output = self.container.create_empty_like(output_path)?;
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
                            CvaError::Repack("dropped payload length overflow".into())
                        })?)
                        .ok_or_else(|| CvaError::Repack("dropped payload byte overflow".into()))?;
                    continue;
                }

                let payload = crate::storage_reclamation::relocate_payload(&payload, &relocated)
                    .map_err(CvaError::Repack)?;
                let destination_ref = output.append(&payload)?;
                relocated.insert(source_ref, destination_ref);
                copied_chunks += 1;
            }
            output.sync()?;
            Ok::<(), CvaError>(())
        })();

        if let Err(error) = copy_result {
            drop(output);
            let _ = fs::remove_file(output_path);
            return Err(error);
        }
        drop(output);

        if dropped_content_objects != removable.len() {
            let _ = fs::remove_file(output_path);
            return Err(CvaError::Repack(format!(
                "expected to drop {} repository-backed content objects but dropped {dropped_content_objects}",
                removable.len()
            )));
        }

        let reopened = match Cva::open(output_path) {
            Ok(reopened) => reopened,
            Err(error) => {
                let _ = fs::remove_file(output_path);
                return Err(error);
            }
        };
        if reopened.owner_uuid() != self.owner_uuid()
            || reopened.latest_global_version() != self.latest_global_version()
            || reopened.archive_version() != self.archive_version()
            || !self.archive.activity_history_matches(&reopened.archive)
            || reopened.memory_version() != self.memory_version()
            || reopened.graph_version() != self.graph_version()
        {
            drop(reopened);
            let _ = fs::remove_file(output_path);
            return Err(CvaError::Repack(
                "repacked REL changed owner, semantic versions or activity history".into(),
            ));
        }
        drop(reopened);

        let output_file_bytes = fs::metadata(output_path)
            .map_err(|error| CvaError::Repack(error.to_string()))?
            .len();
        if output_file_bytes >= source_file_bytes {
            let _ = fs::remove_file(output_path);
            return Err(CvaError::Repack(format!(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn object(offset: u64, len: u64) -> ObjectRef {
        let mut bytes = [0_u8; 16];
        bytes[..8].copy_from_slice(&offset.to_le_bytes());
        bytes[8..].copy_from_slice(&len.to_le_bytes());
        ObjectRef::from_legacy_bytes(bytes)
    }

    #[test]
    fn relocate_payload_rewrites_every_durable_object_ref_record() {
        let old = object(100, 24);
        let new = object(80, 24);
        let relocated = BTreeMap::from([(old, new)]);

        let archive = crate::ArchiveRecordVersion {
            global_version: 3,
            archive_version: 2,
            record: old,
        };
        let archive = crate::storage_reclamation::relocate_payload(
            &crate::archive_history_codec::encode_record_version(archive),
            &relocated,
        )
        .unwrap();
        assert_eq!(
            crate::archive_history_codec::decode_record_version(&archive)
                .unwrap()
                .unwrap()
                .record,
            new
        );

        let memory = crate::memory_codec::MemoryVersion {
            global_version: 3,
            memory_version: 2,
            record: old,
        };
        let memory = crate::storage_reclamation::relocate_payload(
            &crate::memory_codec::encode_version(memory),
            &relocated,
        )
        .unwrap();
        assert_eq!(
            crate::memory_codec::decode_version(&memory)
                .unwrap()
                .unwrap()
                .record,
            new
        );

        let graph = crate::graph_codec::GraphVersionPayload {
            global_version: 3,
            graph_version: 2,
            mutation: old,
        };
        let graph = crate::storage_reclamation::relocate_payload(
            &crate::graph_codec::encode_version(graph),
            &relocated,
        )
        .unwrap();
        assert_eq!(
            crate::graph_codec::decode_version(&graph)
                .unwrap()
                .unwrap()
                .mutation,
            new
        );

        let generation = crate::vector_generation_codec::GenerationVersion {
            global_version: 3,
            vector_version: 2,
            record: old,
        };
        let generation = crate::storage_reclamation::relocate_payload(
            &crate::vector_generation_codec::encode_version(generation),
            &relocated,
        )
        .unwrap();
        assert_eq!(
            crate::vector_generation_codec::decode_version(&generation)
                .unwrap()
                .unwrap()
                .record,
            new
        );
    }
}
