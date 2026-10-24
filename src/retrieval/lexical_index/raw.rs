use super::*;
use crate::archive::ArchiveReadView;
use crate::fragment_store::fragment_id;

impl LexicalIndex {
    pub(crate) fn ensure_raw_current(
        &mut self,
        archive: &ArchiveReadView<'_>,
    ) -> Result<(), ArchiveError> {
        let nodes = archive.nodes_from(self.raw_turns.len());
        for node in nodes {
            let text = archive.content(node.content_id)?;
            let slot = self.raw_turns.len();
            for (term, count) in term_counts(&text) {
                self.raw_postings.entry(term).or_default().push(Posting {
                    fragment_slot: slot,
                    frequency: u8::try_from(count.min(4)).unwrap_or(4),
                });
            }
            self.raw_turns.push(IndexedRawTurn {
                fragment: Fragment {
                    id: fragment_id(&node.conversation_id, &node.id, &node.id),
                    conversation_id: node.conversation_id,
                    start_node_id: node.id.clone(),
                    end_node_id: node.id,
                },
                timestamp_ns: node.timestamp_ns,
            });
        }
        Ok(())
    }

    pub(crate) fn search_raw(&self, terms: &[String], limit: usize) -> Vec<LexicalHit> {
        if terms.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut scores: HashMap<usize, ScoreParts> = HashMap::new();
        for term in terms {
            let Some(postings) = self.raw_postings.get(term) else {
                continue;
            };
            for posting in postings {
                add_score(&mut scores, posting.fragment_slot, posting.frequency);
            }
        }
        let mut hits = scores
            .into_iter()
            .map(|(slot, parts)| {
                let indexed = &self.raw_turns[slot];
                (
                    LexicalHit {
                        fragment: indexed.fragment.clone(),
                        score: score(parts, terms.len()),
                    },
                    indexed.timestamp_ns,
                )
            })
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| {
            right
                .0
                .score
                .total_cmp(&left.0.score)
                .then_with(|| right.1.cmp(&left.1))
                .then_with(|| left.0.fragment.id.0.cmp(&right.0.fragment.id.0))
        });
        hits.truncate(limit);
        hits.into_iter().map(|(hit, _)| hit).collect()
    }
}
