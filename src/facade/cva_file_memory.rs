use crate::{ArchiveError, Cva, FileId, FileMemoryLink, MemoryId, StoredFile};

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
            .link_file_to_memory(&mut self.container, file_id, memory_id)
    }

    pub fn file_memory_links(&self, file_id: FileId) -> Vec<FileMemoryLink> {
        self.archive.file_memory_links(file_id)
    }

    pub fn files_for_memory(&self, memory_id: MemoryId) -> Vec<StoredFile> {
        self.archive.files_for_memory(memory_id)
    }
}
