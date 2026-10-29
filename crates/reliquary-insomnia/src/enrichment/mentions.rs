use super::*;

pub(super) fn append_mentions(
    candidate: &InsomniaCandidate,
    value: &Value,
    output: &mut Vec<MemoryEntityMention>,
) -> Result<(), InsomniaExtractionError> {
    let field = match value.get("field").and_then(Value::as_str) {
        Some("title") => MemoryTextField::Title,
        Some("content") => MemoryTextField::Content,
        _ => return Err(invalid("entity mention field is invalid")),
    };
    let text = value
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("entity mention text is missing"))?;
    let occurrence = value
        .get("occurrence")
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    if text.is_empty()
        || text != text.trim()
        || text.len() > MAX_MEMORY_ROUTING_TEXT_BYTES
        || occurrence < -1
    {
        return Ok(());
    }
    let source = match field {
        MemoryTextField::Title => candidate.title.as_str(),
        MemoryTextField::Content => candidate.content.as_str(),
    };
    let text = normalize_inline_code_mention(source, text);
    if is_non_reusable_generic_surface(text) {
        return Ok(());
    }
    let starts = occurrence_starts(source, text)
        .into_iter()
        .filter(|start| {
            let end = *start + text.len();
            is_semantic_match_boundary(source, *start, end, &source[*start..end])
        })
        .collect::<Vec<_>>();
    let selected = if occurrence == -1 {
        starts
    } else {
        usize::try_from(occurrence)
            .ok()
            .and_then(|index| starts.get(index).copied())
            .into_iter()
            .collect()
    };
    for start in selected {
        let end = start + text.len();
        let surface = &source[start..end];
        output.push(MemoryEntityMention {
            field,
            start_byte: u32::try_from(start)
                .map_err(|_| invalid("entity mention span overflow"))?,
            end_byte: u32::try_from(end).map_err(|_| invalid("entity mention span overflow"))?,
            text: surface.to_owned(),
        });
    }
    Ok(())
}
fn occurrence_starts(source: &str, text: &str) -> Vec<usize> {
    let exact = source
        .match_indices(text)
        .map(|(start, _)| start)
        .collect::<Vec<_>>();
    if !exact.is_empty() || !text.is_ascii() {
        return exact;
    }
    source
        .as_bytes()
        .windows(text.len())
        .enumerate()
        .filter_map(|(start, window)| {
            let end = start + text.len();
            (source.is_char_boundary(start)
                && source.is_char_boundary(end)
                && window.eq_ignore_ascii_case(text.as_bytes()))
            .then_some(start)
        })
        .collect()
}

pub(super) fn retain_most_specific_mentions(mentions: &mut Vec<MemoryEntityMention>) {
    let keep = mentions
        .iter()
        .enumerate()
        .map(|(index, mention)| {
            !mentions.iter().enumerate().any(|(other_index, other)| {
                index != other_index
                    && mention.field == other.field
                    && other.start_byte <= mention.start_byte
                    && other.end_byte >= mention.end_byte
                    && (other.start_byte < mention.start_byte || other.end_byte > mention.end_byte)
            })
        })
        .collect::<Vec<_>>();
    let mut index = 0usize;
    mentions.retain(|_| {
        let retain = keep[index];
        index += 1;
        retain
    });
}

fn normalize_inline_code_mention<'a>(source: &str, text: &'a str) -> &'a str {
    if text.len() >= 3 && text.starts_with('`') && text.ends_with('`') {
        let inner = &text[1..text.len() - 1];
        if !inner.is_empty()
            && !inner.starts_with('`')
            && !inner.ends_with('`')
            && !occurrence_starts(source, inner).is_empty()
        {
            return inner;
        }
    }
    text
}

fn is_semantic_match_boundary(source: &str, start: usize, end: usize, text: &str) -> bool {
    let starts_word = text.chars().next().is_some_and(is_word_char);
    let ends_word = text.chars().next_back().is_some_and(is_word_char);
    let left_ok = !starts_word
        || source[..start]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_word_char(ch));
    let right_ok = !ends_word
        || source[end..]
            .chars()
            .next()
            .is_none_or(|ch| !is_word_char(ch));
    left_ok && right_ok
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}
