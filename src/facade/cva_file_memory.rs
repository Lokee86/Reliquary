use crate::{ArchiveError, Cva, FileId, MemoryId, StoredFile};

/// A source-side file link projected into the product's canonical Memory identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FileMemoryLink {
    pub file_id: FileId,
    pub memory_id: MemoryId,
}

impl From<reliquary_archive::file_memory_link_model::FileMemoryLink> for FileMemoryLink {
    fn from(value: reliquary_archive::file_memory_link_model::FileMemoryLink) -> Self {
        Self {
            file_id: value.file_id,
            memory_id: MemoryId(value.memory_id.0),
        }
    }
}

impl Cva {
    pub fn link_file_to_memory(
        &mut self,
        file_id: FileId,
        memory_id: MemoryId,
    ) -> Result<FileMemoryLink, ArchiveError> {
        self.memories
            .current_body_id(memory_id)
            .map_err(|_| ArchiveError::MissingFileMemoryTarget)?;
        self.archive
            .link_file_to_memory(
                &mut self.container,
                file_id,
                reliquary_archive::MemoryId(memory_id.0),
            )
            .map(Into::into)
    }

    pub fn file_memory_links(&self, file_id: FileId) -> Vec<FileMemoryLink> {
        self.archive
            .file_memory_links(file_id)
            .into_iter()
            .map(Into::into)
            .collect()
    }

    pub fn files_for_memory(&self, memory_id: MemoryId) -> Vec<StoredFile> {
        self.archive
            .files_for_memory(reliquary_archive::MemoryId(memory_id.0))
    }
}
