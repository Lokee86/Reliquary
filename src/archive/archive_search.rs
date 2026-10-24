use crate::Fragment;

pub const MAX_ARCHIVE_SEARCH_QUERY_BYTES: usize = 512;
pub const MAX_ARCHIVE_SEARCH_RESULTS: usize = 10;

#[derive(Clone, Debug, PartialEq)]
pub struct ArchiveSearchHit {
    pub fragment: Fragment,
    pub text: String,
    pub score: f64,
}
