use crate::{ArchiveError, Episode, EpisodeBoundary, EpisodeId, EpisodeOrigin};

pub const EPISODE_MAGIC: [u8; 8] = *b"CVAEPIS1";

pub fn encode_episode(episode: &Episode) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(&EPISODE_MAGIC);
    encode_episode_fields(&mut out, episode)?;
    Ok(out)
}

pub(crate) fn encode_episode_fields(
    out: &mut Vec<u8>,
    episode: &Episode,
) -> Result<(), ArchiveError> {
    out.extend_from_slice(&episode.id.0);
    out.push(episode.origin.tag());
    out.push(episode.boundary.tag());
    out.extend_from_slice(&episode.source_through_ns.to_le_bytes());
    out.extend_from_slice(&episode.finalized_at_ns.to_le_bytes());
    write_string(out, &episode.conversation_id)?;
    write_string(out, &episode.start_node_id)?;
    write_string(out, &episode.end_node_id)?;
    Ok(())
}

pub fn decode_episode(bytes: &[u8]) -> Result<Option<Episode>, ArchiveError> {
    if bytes.len() < 8 || bytes[..8] != EPISODE_MAGIC {
        return Ok(None);
    }
    if bytes.len() < 58 {
        return Err(ArchiveError::CorruptRecord("short episode record"));
    }
    let mut cursor = 8;
    let episode = decode_episode_fields(bytes, &mut cursor)?;
    if cursor != bytes.len() {
        return Err(ArchiveError::CorruptRecord("episode trailing bytes"));
    }
    Ok(Some(episode))
}

pub(crate) fn decode_episode_fields(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Episode, ArchiveError> {
    let id = EpisodeId(read_fixed::<32>(bytes, cursor, "short episode record")?);
    let origin =
        EpisodeOrigin::from_tag(read_fixed::<1>(bytes, cursor, "short episode record")?[0])
            .ok_or(ArchiveError::CorruptRecord("invalid episode origin"))?;
    let boundary =
        EpisodeBoundary::from_tag(read_fixed::<1>(bytes, cursor, "short episode record")?[0])
            .ok_or(ArchiveError::CorruptRecord("invalid episode boundary"))?;
    let source_through_ns =
        i64::from_le_bytes(read_fixed::<8>(bytes, cursor, "short episode record")?);
    let finalized_at_ns =
        i64::from_le_bytes(read_fixed::<8>(bytes, cursor, "short episode record")?);
    let conversation_id = read_string(bytes, cursor)?;
    let start_node_id = read_string(bytes, cursor)?;
    let end_node_id = read_string(bytes, cursor)?;
    Ok(Episode {
        id,
        conversation_id,
        start_node_id,
        end_node_id,
        origin,
        boundary,
        source_through_ns,
        finalized_at_ns,
    })
}

fn read_fixed<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
    error: &'static str,
) -> Result<[u8; N], ArchiveError> {
    let end = cursor.checked_add(N).ok_or(ArchiveError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(ArchiveError::CorruptRecord(error))?;
    *cursor = end;
    Ok(raw.try_into().unwrap())
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), ArchiveError> {
    let len = u32::try_from(value.len()).map_err(|_| ArchiveError::FieldTooLarge)?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, ArchiveError> {
    let length_end = cursor.checked_add(4).ok_or(ArchiveError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..length_end)
        .ok_or(ArchiveError::CorruptRecord("missing episode string length"))?;
    let len = u32::from_le_bytes(raw.try_into().unwrap()) as usize;
    *cursor = length_end;
    let end = cursor.checked_add(len).ok_or(ArchiveError::FieldTooLarge)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(ArchiveError::CorruptRecord("truncated episode string"))?;
    *cursor = end;
    String::from_utf8(value.to_vec()).map_err(|_| ArchiveError::InvalidUtf8)
}
