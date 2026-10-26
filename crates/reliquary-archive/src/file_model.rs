use crate::ContentId;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FileId(pub [u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredFile {
    pub id: FileId,
    pub content_id: ContentId,
    pub filename: String,
    pub mime_type: Option<String>,
    pub byte_length: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileSearchHit {
    pub file: StoredFile,
    pub score: f64,
}
