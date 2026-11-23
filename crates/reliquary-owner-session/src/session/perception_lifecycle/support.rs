use super::*;

pub(in crate::session) fn owner_hex(owner: &[u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(owner.len() * 2);
    for byte in owner {
        value.push(HEX[usize::from(byte >> 4)] as char);
        value.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    value
}

pub(super) fn source_memory(
    common: &CommonState,
    memory_id: MemoryId,
    revision: u64,
    body_id: MemoryBodyId,
    field: MemoryTextField,
    start_byte: usize,
    end_byte: usize,
    quote: &str,
) -> Result<MemoryRevision, OwnerSessionError> {
    let memory = common.memories.current(memory_id)?;
    if memory.record.revision != revision || memory.record.body_id != body_id {
        return Err(OwnerSessionError::ManagementStale);
    }
    if !has_direct_source(&memory.record) {
        return Err(OwnerSessionError::ManagementEvidence);
    }
    let PayloadState::Available(body) = &memory.payload else {
        return Err(OwnerSessionError::Record(
            "semantic source Memory body is unavailable".into(),
        ));
    };
    let text = text_field(body, field);
    if quote.trim().is_empty() || text.get(start_byte..end_byte) != Some(quote) {
        return Err(OwnerSessionError::ManagementEvidence);
    }
    Ok(memory)
}

pub(in crate::session) fn has_direct_source(record: &reliquary_memory_core::MemoryRecord) -> bool {
    record.authority_kind == "direct"
        && record.source_ref.as_ref().is_some_and(|source| {
            !source.owner_id.trim().is_empty() && !source.source_node_id.trim().is_empty()
        })
}

pub(in crate::session) fn text_field(
    body: &reliquary_memory_core::MemoryBody,
    field: MemoryTextField,
) -> &str {
    match field {
        MemoryTextField::Title => &body.title,
        MemoryTextField::Content => &body.content,
    }
}

pub(in crate::session) fn contains_normalized(haystack: &str, needle: &str) -> bool {
    let normalize = |value: &str| {
        value
            .split(|ch: char| !ch.is_alphanumeric())
            .filter(|token| !token.is_empty())
            .map(str::to_lowercase)
            .collect::<Vec<_>>()
    };
    let needle = normalize(needle);
    !needle.is_empty()
        && normalize(haystack)
            .windows(needle.len())
            .any(|tokens| tokens == needle)
}

#[cfg(test)]
#[test]
fn evidence_names_require_complete_contiguous_unicode_tokens() {
    assert!(!contains_normalized("planning a party", "Ann"));
    assert!(!contains_normalized("planning a party", "Art"));
    assert!(!contains_normalized(
        "Mary reviews Jane's plan",
        "Mary Jane"
    ));
    assert!(contains_normalized("ÉLODIE's release notes", "Élodie"));
    assert!(contains_normalized(
        "Dr. Mira-Lantern reviews it",
        "Mira Lantern"
    ));
    assert!(!contains_normalized("Mira", "---"));
}
