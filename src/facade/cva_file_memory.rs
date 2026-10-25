use crate::{ArchiveError, Cva, FileId, FileMemoryLink, MemoryId, StoredFile};

impl Cva {
    pub fn link_file_to_memory(
        &mut self,
        file_id: FileId,
        memory_id: MemoryId,
    ) -> Result<FileMemoryLink, ArchiveError> {
        self.runtime_link_file_to_memory(file_id, memory_id)
    }

    pub fn file_memory_links(&self, file_id: FileId) -> Vec<FileMemoryLink> {
        self.runtime_file_memory_links(file_id)
    }

    pub fn files_for_memory(&self, memory_id: MemoryId) -> Vec<StoredFile> {
        self.runtime_files_for_memory(memory_id)
    }
}
