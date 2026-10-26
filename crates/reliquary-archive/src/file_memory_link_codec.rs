use crate::{ArchiveError, FileId, FileMemoryLink, MemoryId};

pub(crate) const FILE_MEMORY_LINK_MAGIC: [u8; 8] = *b"CVAFMEM1";

pub(crate) fn encode_file_memory_link(link: FileMemoryLink) -> Vec<u8> {
    let mut out = Vec::with_capacity(72);
    out.extend_from_slice(&FILE_MEMORY_LINK_MAGIC);
    encode_file_memory_link_fields(&mut out, &link).expect("fixed file-memory fields fit");
    out
}

pub(crate) fn encode_file_memory_link_fields(
    out: &mut Vec<u8>,
    link: &FileMemoryLink,
) -> Result<(), ArchiveError> {
    out.extend_from_slice(&link.file_id.0);
    out.extend_from_slice(&link.memory_id.0);
    Ok(())
}

pub(crate) fn decode_file_memory_link(bytes: &[u8]) -> Result<FileMemoryLink, ArchiveError> {
    if bytes.len() != 72 {
        return Err(ArchiveError::CorruptRecord("invalid file-memory link"));
    }
    let mut cursor = 8;
    let link = decode_file_memory_link_fields(bytes, &mut cursor)?;
    debug_assert_eq!(cursor, bytes.len());
    Ok(link)
}

pub(crate) fn decode_file_memory_link_fields(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<FileMemoryLink, ArchiveError> {
    let file_id = FileId(read_fixed(bytes, cursor)?);
    let memory_id = MemoryId(read_fixed(bytes, cursor)?);
    Ok(FileMemoryLink { file_id, memory_id })
}

fn read_fixed<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], ArchiveError> {
    let end = cursor.checked_add(N).ok_or(ArchiveError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(ArchiveError::CorruptRecord("invalid file-memory link"))?;
    *cursor = end;
    Ok(raw.try_into().unwrap())
}
