use crate::archive_search::ArchiveSearchHit;
use crate::{Cva, SearchError};

impl Cva {
    pub fn search_archive(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ArchiveSearchHit>, SearchError> {
        self.runtime_search_archive(query, limit)
    }
}
