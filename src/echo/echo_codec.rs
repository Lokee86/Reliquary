use crate::{EchoError, EchoEvent, EchoEventKind};

const RECORD_MAGIC: [u8; 8] = *b"CVAECHO1";

pub(crate) fn encode(event: &EchoEvent) -> Result<Vec<u8>, EchoError> {
    validate(event)?;
    let mut out = Vec::with_capacity(64 + event.content.len());
    out.extend_from_slice(&RECORD_MAGIC);
    write_string(&mut out, &event.conversation_id)?;
    write_string(&mut out, &event.message_id)?;
    out.extend_from_slice(&event.sequence.to_le_bytes());
    out.extend_from_slice(&event.timestamp_ns.to_le_bytes());
    write_optional_u32(&mut out, event.model_round);
    out.push(kind_tag(event.kind));
    write_optional_string(&mut out, event.correlation_id.as_deref())?;
    write_optional_string(&mut out, event.name.as_deref())?;
    write_string(&mut out, &event.content)?;
    Ok(out)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Option<EchoEvent>, EchoError> {
    if bytes.len() < RECORD_MAGIC.len() || bytes[..8] != RECORD_MAGIC {
        return Ok(None);
    }
    let mut cursor = 8;
    let event = EchoEvent {
        conversation_id: read_string(bytes, &mut cursor)?,
        message_id: read_string(bytes, &mut cursor)?,
        sequence: read_u64(bytes, &mut cursor)?,
        timestamp_ns: read_i64(bytes, &mut cursor)?,
        model_round: read_optional_u32(bytes, &mut cursor)?,
        kind: tag_kind(read_byte(bytes, &mut cursor)?)?,
        correlation_id: read_optional_string(bytes, &mut cursor)?,
        name: read_optional_string(bytes, &mut cursor)?,
        content: read_string(bytes, &mut cursor)?,
    };
    if cursor != bytes.len() {
        return Err(EchoError::InvalidRecord("trailing bytes"));
    }
    validate(&event)?;
    Ok(Some(event))
}

fn validate(event: &EchoEvent) -> Result<(), EchoError> {
    if event.conversation_id.is_empty() {
        return Err(EchoError::InvalidRecord("conversation ID is required"));
    }
    if event.message_id.is_empty() {
        return Err(EchoError::InvalidRecord("message ID is required"));
    }
    match event.kind {
        EchoEventKind::ToolCall => {
            require_text(event.correlation_id.as_deref(), "tool call ID is required")?;
            require_text(event.name.as_deref(), "tool name is required")?;
        }
        EchoEventKind::ToolResult
        | EchoEventKind::ToolContextAccepted
        | EchoEventKind::ActivityStarted
        | EchoEventKind::ActivityCompleted
        | EchoEventKind::ActivityFailed => {
            require_text(
                event.correlation_id.as_deref(),
                "correlation ID is required",
            )?;
        }
        EchoEventKind::ReasoningSummary
        | EchoEventKind::Commentary
        | EchoEventKind::ReasoningTrace => {}
    }
    Ok(())
}

fn require_text(value: Option<&str>, message: &'static str) -> Result<(), EchoError> {
    if value.is_some_and(|value| !value.is_empty()) {
        Ok(())
    } else {
        Err(EchoError::InvalidRecord(message))
    }
}

fn kind_tag(kind: EchoEventKind) -> u8 {
    match kind {
        EchoEventKind::ReasoningSummary => 0,
        EchoEventKind::Commentary => 1,
        EchoEventKind::ReasoningTrace => 2,
        EchoEventKind::ToolCall => 3,
        EchoEventKind::ToolResult => 4,
        EchoEventKind::ActivityStarted => 5,
        EchoEventKind::ActivityCompleted => 6,
        EchoEventKind::ActivityFailed => 7,
        EchoEventKind::ToolContextAccepted => 8,
    }
}

fn tag_kind(tag: u8) -> Result<EchoEventKind, EchoError> {
    match tag {
        0 => Ok(EchoEventKind::ReasoningSummary),
        1 => Ok(EchoEventKind::Commentary),
        2 => Ok(EchoEventKind::ReasoningTrace),
        3 => Ok(EchoEventKind::ToolCall),
        4 => Ok(EchoEventKind::ToolResult),
        5 => Ok(EchoEventKind::ActivityStarted),
        6 => Ok(EchoEventKind::ActivityCompleted),
        7 => Ok(EchoEventKind::ActivityFailed),
        8 => Ok(EchoEventKind::ToolContextAccepted),
        _ => Err(EchoError::InvalidRecord("unknown event kind")),
    }
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), EchoError> {
    let len = u32::try_from(value.len()).map_err(|_| EchoError::RecordTooLarge)?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_optional_string(out: &mut Vec<u8>, value: Option<&str>) -> Result<(), EchoError> {
    match value {
        Some(value) => {
            out.push(1);
            write_string(out, value)?;
        }
        None => out.push(0),
    }
    Ok(())
}

fn write_optional_u32(out: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(value) => {
            out.push(1);
            out.extend_from_slice(&value.to_le_bytes());
        }
        None => out.push(0),
    }
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, EchoError> {
    let len = read_u32(bytes, cursor)? as usize;
    let end = cursor.checked_add(len).ok_or(EchoError::RecordTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(EchoError::InvalidRecord("truncated string"))?;
    *cursor = end;
    String::from_utf8(raw.to_vec()).map_err(|_| EchoError::InvalidRecord("invalid UTF-8"))
}

fn read_optional_string(bytes: &[u8], cursor: &mut usize) -> Result<Option<String>, EchoError> {
    match read_byte(bytes, cursor)? {
        0 => Ok(None),
        1 => Ok(Some(read_string(bytes, cursor)?)),
        _ => Err(EchoError::InvalidRecord("invalid optional string flag")),
    }
}

fn read_optional_u32(bytes: &[u8], cursor: &mut usize) -> Result<Option<u32>, EchoError> {
    match read_byte(bytes, cursor)? {
        0 => Ok(None),
        1 => Ok(Some(read_u32(bytes, cursor)?)),
        _ => Err(EchoError::InvalidRecord("invalid model-round flag")),
    }
}

fn read_byte(bytes: &[u8], cursor: &mut usize) -> Result<u8, EchoError> {
    let value = *bytes
        .get(*cursor)
        .ok_or(EchoError::InvalidRecord("truncated byte"))?;
    *cursor += 1;
    Ok(value)
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, EchoError> {
    let end = cursor.checked_add(4).ok_or(EchoError::RecordTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(EchoError::InvalidRecord("truncated u32"))?;
    *cursor = end;
    Ok(u32::from_le_bytes(raw.try_into().unwrap()))
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, EchoError> {
    let end = cursor.checked_add(8).ok_or(EchoError::RecordTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(EchoError::InvalidRecord("truncated u64"))?;
    *cursor = end;
    Ok(u64::from_le_bytes(raw.try_into().unwrap()))
}

fn read_i64(bytes: &[u8], cursor: &mut usize) -> Result<i64, EchoError> {
    let end = cursor.checked_add(8).ok_or(EchoError::RecordTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(EchoError::InvalidRecord("truncated i64"))?;
    *cursor = end;
    Ok(i64::from_le_bytes(raw.try_into().unwrap()))
}
