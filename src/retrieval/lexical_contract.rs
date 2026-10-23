//! Immutable lexical results and deterministic tokenization shared by owner services.
use crate::Fragment;
use std::collections::{HashMap, HashSet};

pub(crate) struct LexicalHit {
    pub fragment: Fragment,
    pub score: f64,
}

pub(crate) fn lexical_terms(value: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut terms = Vec::new();
    let mut word = String::new();
    let flush = |word: &mut String, seen: &mut HashSet<String>, terms: &mut Vec<String>| {
        if word.is_empty() {
            return;
        }
        let term = word.to_lowercase();
        word.clear();
        if seen.insert(term.clone()) {
            terms.push(term);
        }
    };
    for current in value.chars() {
        if current.is_alphanumeric() {
            word.push(current);
        } else {
            flush(&mut word, &mut seen, &mut terms);
        }
    }
    flush(&mut word, &mut seen, &mut terms);
    terms
}

pub(crate) fn term_counts(value: &str) -> HashMap<String, usize> {
    let lower = value.to_lowercase();
    lexical_terms(value)
        .into_iter()
        .map(|term| {
            let count = lower.matches(&term).count();
            (term, count)
        })
        .collect()
}
