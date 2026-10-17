use crate::file_memory_link_codec::encode_file_memory_link;
use crate::memory_store::MemoryStore;
use crate::{Archive, ArchiveError, Container, FileId, FileMemoryLink, MemoryId, StoredFile};

impl Archive {
    pub(crate) fn link_file_to_memory(
        &mut self,
        container: &mut Container,
        file_id: FileId,
        memory_id: MemoryId,
    ) -> Result<FileMemoryLink, ArchiveError> {
        if self.files.get(file_id).is_none() {
            return Err(ArchiveError::MissingFile);
        }
        let link = FileMemoryLink { file_id, memory_id };
        if self.file_memory_links.contains(file_id, memory_id) {
            return Ok(link);
        }
        let record = container.append(&encode_file_memory_link(link))?;
        self.publish_record(container, record)?;
        self.file_memory_links.insert(link);
        Ok(link)
    }

    pub(crate) fn file_memory_links(&self, file_id: FileId) -> Vec<FileMemoryLink> {
        self.file_memory_links.for_file(file_id).copied().collect()
    }

    pub(crate) fn files_for_memory(&self, memory_id: MemoryId) -> Vec<StoredFile> {
        self.file_memory_links
            .for_memory(memory_id)
            .filter_map(|link| self.files.get(link.file_id).cloned())
            .collect()
    }

    pub(crate) fn validate_file_memory_links(&self) -> Result<(), ArchiveError> {
        for link in self.file_memory_links.iter() {
            if self.files.get(link.file_id).is_none() {
                return Err(ArchiveError::MissingFile);
            }
        }
        Ok(())
    }
}

pub(crate) fn validate_file_memory_targets(
    archive: &Archive,
    memories: &MemoryStore,
) -> Result<(), ArchiveError> {
    for link in archive.file_memory_links.iter() {
        if !memories.contains_historical_memory(link.memory_id) {
            return Err(ArchiveError::MissingFileMemoryTarget);
        }
    }
    Ok(())
}
