use reliquary_memory_core::{Memory, MemoryEntityMention, MemoryTextField};
use reliquary_retrieval::lexical_terms;
use std::collections::HashSet;

pub fn context_query(memory: &Memory, mention: &MemoryEntityMention) -> String {
    let field = match mention.field {
        MemoryTextField::Title => memory.title.as_str(),
        MemoryTextField::Content => memory.content.as_str(),
    };
    let start = usize::try_from(mention.start_byte).unwrap_or(usize::MAX);
    let end = usize::try_from(mention.end_byte).unwrap_or(usize::MAX);
    let local = local_context(field, start, end);

    let mut terms = Vec::new();
    let mut seen = HashSet::new();
    append_terms(&mention.text, &mut terms, &mut seen);
    if mention.field == MemoryTextField::Content {
        append_terms(&memory.title, &mut terms, &mut seen);
    }
    append_terms(&local, &mut terms, &mut seen);
    terms.truncate(crate::MAX_ENTITY_CANDIDATE_CONTEXT_TERMS);
    terms.join(" ")
}

pub fn memory_contains_surface(memory: &Memory, surface: &str) -> bool {
    let needle = surface.trim().to_lowercase();
    if needle.is_empty() {
        return false;
    }
    memory.title.to_lowercase().contains(&needle) || memory.content.to_lowercase().contains(&needle)
}

fn append_terms(value: &str, terms: &mut Vec<String>, seen: &mut HashSet<String>) {
    for term in lexical_terms(value) {
        if seen.insert(term.clone()) {
            terms.push(term);
        }
    }
}

fn local_context(value: &str, start: usize, end: usize) -> String {
    let (Some(before), Some(after)) = (value.get(..start), value.get(end..)) else {
        return String::new();
    };
    let mut prefix: Vec<_> = before.chars().rev().take(96).collect();
    prefix.reverse();
    let suffix: String = after.chars().take(96).collect();
    format!("{} {}", prefix.into_iter().collect::<String>(), suffix)
}
