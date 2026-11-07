use super::*;

pub(super) fn owner_hex(owner: &[u8; 16]) -> String {
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
        return Err(OwnerSessionError::Record(
            "semantic source Memory changed before mutation".into(),
        ));
    }
    if memory.record.source_ref.as_ref().is_none_or(|source| {
        source.owner_id.trim().is_empty() || source.source_node_id.trim().is_empty()
    }) || memory.record.authority_kind != "direct"
    {
        return Err(OwnerSessionError::Record(
            "semantic mutation requires accepted Memory provenance".into(),
        ));
    }
    let PayloadState::Available(body) = &memory.payload else {
        return Err(OwnerSessionError::Record(
            "semantic source Memory body is unavailable".into(),
        ));
    };
    let text = match field {
        MemoryTextField::Title => &body.title,
        MemoryTextField::Content => &body.content,
    };
    if quote.trim().is_empty() || text.get(start_byte..end_byte) != Some(quote) {
        return Err(OwnerSessionError::Record(
            "semantic evidence is not an exact accepted source span".into(),
        ));
    }
    Ok(memory)
}

pub(super) fn contains_normalized(haystack: &str, needle: &str) -> bool {
    let normalize = |value: &str| {
        value
            .chars()
            .filter(|ch| ch.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    !normalize(needle).is_empty() && normalize(haystack).contains(&normalize(needle))
}
