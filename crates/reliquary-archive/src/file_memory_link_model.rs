use crate::{FileId, MemoryId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FileMemoryLink {
    pub file_id: FileId,
    pub memory_id: MemoryId,
}
