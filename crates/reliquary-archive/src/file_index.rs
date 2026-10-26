use crate::{ArchiveError, FileId, StoredFile};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct FileIndex {
    records: Vec<StoredFile>,
    by_id: HashMap<FileId, usize>,
}

impl FileIndex {
    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }

    pub(crate) fn get(&self, id: FileId) -> Option<&StoredFile> {
        self.by_id.get(&id).map(|index| &self.records[*index])
    }

    pub(crate) fn insert(&mut self, file: StoredFile) -> Result<bool, ArchiveError> {
        if let Some(existing) = self.get(file.id) {
            return if existing == &file {
                Ok(false)
            } else {
                Err(ArchiveError::ConflictingFile)
            };
        }
        let index = self.records.len();
        self.by_id.insert(file.id, index);
        self.records.push(file);
        Ok(true)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &StoredFile> {
        self.records.iter()
    }
}
