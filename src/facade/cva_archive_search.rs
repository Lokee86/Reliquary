use crate::archive_search::{
    ArchiveSearchHit, MAX_ARCHIVE_SEARCH_QUERY_BYTES, MAX_ARCHIVE_SEARCH_RESULTS,
};
use crate::{Cva, SearchError};

impl Cva {
    pub fn search_archive(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ArchiveSearchHit>, SearchError> {
        let query = query.trim();
        if query.is_empty() || query.len() > MAX_ARCHIVE_SEARCH_QUERY_BYTES {
            return Err(SearchError::EmptyQuery);
        }
        if !(1..=MAX_ARCHIVE_SEARCH_RESULTS).contains(&limit) {
            return Err(SearchError::InvalidConfig);
        }
        let hits = self
            .raw_archive_candidates(query, crate::MAX_ARCHIVE_SEARCH_RESULTS)?
            .into_iter()
            .filter(|hit| {
                !self
                    .archive
                    .is_conversation_tombstoned(&hit.fragment.conversation_id)
            })
            .take(limit)
            .collect::<Vec<_>>();
        hits.into_iter()
            .map(|hit| {
                let text = self
                    .archive
                    .fragment_text_for(&mut self.container, &hit.fragment)?;
                Ok(ArchiveSearchHit {
                    fragment: hit.fragment,
                    text,
                    score: hit.score,
                })
            })
            .collect()
    }
}
