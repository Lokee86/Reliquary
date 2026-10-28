use super::*;
use crate::{Fragment, lexical_search::LexicalHit};
use reliquary_archive::fragment_store::fragment_id;

impl LexicalIndex {
    pub(crate) fn ensure_raw_current(
        &mut self,
        archive: &Archive,
        container: &mut Container,
    ) -> Result<(), ArchiveError> {
        for node in archive
            .source_nodes()
            .skip(self.engine.raw_turn_count())
            .cloned()
            .collect::<Vec<_>>()
        {
            let text = archive.content(container, node.content_id)?;
            let fragment = Fragment {
                id: fragment_id(&node.conversation_id, &node.id, &node.id),
                conversation_id: node.conversation_id.clone(),
                start_node_id: node.id.clone(),
                end_node_id: node.id.clone(),
            };
            self.engine.add_raw_turn(fragment, node.timestamp_ns, &text);
        }
        Ok(())
    }
    pub(crate) fn search_raw(&self, terms: &[String], limit: usize) -> Vec<LexicalHit> {
        self.engine.search_raw(terms, limit)
    }
}
