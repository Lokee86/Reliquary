use crate::{FileId, FileMemoryLink, MemoryId};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct FileMemoryLinkIndex {
    records: Vec<FileMemoryLink>,
    pairs: HashSet<(FileId, MemoryId)>,
    by_file: HashMap<FileId, Vec<usize>>,
    by_memory: HashMap<MemoryId, Vec<usize>>,
}

impl FileMemoryLinkIndex {
    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }

    pub(crate) fn insert(&mut self, link: FileMemoryLink) -> bool {
        if !self.pairs.insert((link.file_id, link.memory_id)) {
            return false;
        }
        let index = self.records.len();
        self.by_file.entry(link.file_id).or_default().push(index);
        self.by_memory
            .entry(link.memory_id)
            .or_default()
            .push(index);
        self.records.push(link);
        true
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &FileMemoryLink> {
        self.records.iter()
    }

    pub(crate) fn for_file(&self, file_id: FileId) -> impl Iterator<Item = &FileMemoryLink> {
        self.by_file
            .get(&file_id)
            .into_iter()
            .flatten()
            .map(|index| &self.records[*index])
    }

    pub(crate) fn for_memory(&self, memory_id: MemoryId) -> impl Iterator<Item = &FileMemoryLink> {
        self.by_memory
            .get(&memory_id)
            .into_iter()
            .flatten()
            .map(|index| &self.records[*index])
    }

    pub(crate) fn contains(&self, file_id: FileId, memory_id: MemoryId) -> bool {
        self.pairs.contains(&(file_id, memory_id))
    }
}
