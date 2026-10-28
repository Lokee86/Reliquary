use reliquary_archive::Fragment;

use crate::{LexicalHit, lexical_score};

/// Scores owner-supplied fragment text in source order.
pub struct OrderedFragmentSearch<'a> {
    terms: &'a [String],
    hits: Vec<(LexicalHit, usize)>,
}

impl<'a> OrderedFragmentSearch<'a> {
    pub fn new(terms: &'a [String]) -> Self {
        Self {
            terms,
            hits: Vec::new(),
        }
    }

    pub fn add(&mut self, fragment: Fragment, text: &str, position: usize) {
        let score = lexical_score(text, self.terms);
        if score > 0.0 {
            self.hits.push((LexicalHit { fragment, score }, position));
        }
    }

    pub fn finish(mut self, limit: usize) -> Vec<LexicalHit> {
        self.hits.sort_by(|left, right| {
            right
                .0
                .score
                .total_cmp(&left.0.score)
                .then_with(|| right.1.cmp(&left.1))
                .then_with(|| left.0.fragment.id.0.cmp(&right.0.fragment.id.0))
        });
        self.hits.truncate(limit);
        self.hits.into_iter().map(|(hit, _)| hit).collect()
    }
}
