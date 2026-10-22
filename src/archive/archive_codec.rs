use crate::conversation_metadata_codec::{
    CONVERSATION_METADATA_MAGIC, decode_conversation_metadata,
};
use crate::episode_codec::{EPISODE_MAGIC, decode_episode};
use crate::file_memory_link_codec::{FILE_MEMORY_LINK_MAGIC, decode_file_memory_link};
use crate::turn_ingest_codec::{INGESTED_TURN_MAGIC, decode_ingested_turn};
use crate::{
    ArchiveError, Branch, ContentId, ConversationMetadata, ConversationTombstone,
    ConversationTombstoneClosure, Episode, FileId, FileMemoryLink, Fragment, FragmentId,
    IngestedTurn, Node, StoredFile,
};

const CONTENT_MAGIC: [u8; 8] = *b"CVACONT1";
const NODE_MAGIC: [u8; 8] = *b"CVANODE2";
const BRANCH_MAGIC: [u8; 8] = *b"CVABRCH1";
const FRAGMENT_MAGIC: [u8; 8] = *b"CVAFRAG1";
const FILE_MAGIC: [u8; 8] = *b"CVAFILE1";
const CONVERSATION_TOMBSTONE_MAGIC: [u8; 8] = *b"CVACTMB1";

pub enum ArchiveRecord {
    ImportReceipt(crate::imports::ImportReceipt),
    Content(ContentId, Vec<u8>),
    Node(Node),
    Branch(Branch),
    ConversationMetadata(ConversationMetadata),
    ConversationTombstone(ConversationTombstone),
    Fragment(Fragment),
    Episode(Episode),
    File(StoredFile),
    IngestedTurn(IngestedTurn),
    FileMemoryLink(FileMemoryLink),
    Other,
}

pub fn encode_content(id: ContentId, content: &[u8]) -> Result<Vec<u8>, ArchiveError> {
    let len = u32::try_from(content.len()).map_err(|_| ArchiveError::FieldTooLarge)?;
    let mut out = Vec::with_capacity(44 + content.len());
    out.extend_from_slice(&CONTENT_MAGIC);
    out.extend_from_slice(&id.0);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(content);
    Ok(out)
}

pub fn encode_node(node: &Node) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::with_capacity(96);
    out.extend_from_slice(&NODE_MAGIC);
    encode_node_fields(&mut out, node)?;
    Ok(out)
}

pub fn encode_branch(branch: &Branch) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&BRANCH_MAGIC);
    encode_branch_fields(&mut out, branch)?;
    Ok(out)
}

pub fn encode_conversation_tombstone(
    tombstone: &ConversationTombstone,
) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::new();
    out.extend_from_slice(&CONVERSATION_TOMBSTONE_MAGIC);
    out.extend_from_slice(&tombstone.archive_version.to_le_bytes());
    out.extend_from_slice(&tombstone.global_version.to_le_bytes());
    encode_conversation_tombstone_fields(&mut out, tombstone)?;
    Ok(out)
}

pub fn encode_fragment(fragment: &Fragment) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::with_capacity(96);
    out.extend_from_slice(&FRAGMENT_MAGIC);
    encode_fragment_fields(&mut out, fragment)?;
    Ok(out)
}

pub fn encode_file(file: &StoredFile) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::with_capacity(128 + file.filename.len());
    out.extend_from_slice(&FILE_MAGIC);
    encode_file_fields(&mut out, file)?;
    Ok(out)
}

pub(crate) fn encode_node_fields(out: &mut Vec<u8>, node: &Node) -> Result<(), ArchiveError> {
    out.extend_from_slice(&node.timestamp_ns.to_le_bytes());
    out.extend_from_slice(&node.content_id.0);
    write_string(out, &node.id)?;
    write_string(out, &node.conversation_id)?;
    write_string(out, node.parent_id.as_deref().unwrap_or(""))?;
    write_string(out, &node.role)?;
    write_string(out, node.principal_id.as_deref().unwrap_or(""))?;
    Ok(())
}

pub(crate) fn decode_node_fields(bytes: &[u8], cursor: &mut usize) -> Result<Node, ArchiveError> {
    let timestamp_ns = read_i64(bytes, cursor)?;
    let content_id = ContentId(read_fixed(bytes, cursor)?);
    let id = read_string(bytes, cursor)?;
    let conversation_id = read_string(bytes, cursor)?;
    let parent = read_string(bytes, cursor)?;
    let role = read_string(bytes, cursor)?;
    let principal = read_string(bytes, cursor)?;
    let principal_id = (!principal.is_empty()).then_some(principal);
    Ok(Node {
        id,
        conversation_id,
        parent_id: (!parent.is_empty()).then_some(parent),
        role,
        principal_id,
        timestamp_ns,
        content_id,
    })
}

pub(crate) fn encode_branch_fields(out: &mut Vec<u8>, branch: &Branch) -> Result<(), ArchiveError> {
    out.push(u8::from(branch.canonical));
    write_string(out, &branch.id)?;
    write_string(out, &branch.conversation_id)?;
    write_string(out, &branch.leaf_node_id)?;
    Ok(())
}

pub(crate) fn decode_branch_fields(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Branch, ArchiveError> {
    let canonical = match read_u8(bytes, cursor)? {
        0 => false,
        1 => true,
        _ => return Err(ArchiveError::CorruptRecord("invalid branch flag")),
    };
    Ok(Branch {
        id: read_string(bytes, cursor)?,
        conversation_id: read_string(bytes, cursor)?,
        leaf_node_id: read_string(bytes, cursor)?,
        canonical,
    })
}

pub(crate) fn encode_fragment_fields(
    out: &mut Vec<u8>,
    fragment: &Fragment,
) -> Result<(), ArchiveError> {
    out.extend_from_slice(&fragment.id.0);
    write_string(out, &fragment.conversation_id)?;
    write_string(out, &fragment.start_node_id)?;
    write_string(out, &fragment.end_node_id)?;
    Ok(())
}

pub(crate) fn decode_fragment_fields(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Fragment, ArchiveError> {
    Ok(Fragment {
        id: FragmentId(read_fixed(bytes, cursor)?),
        conversation_id: read_string(bytes, cursor)?,
        start_node_id: read_string(bytes, cursor)?,
        end_node_id: read_string(bytes, cursor)?,
    })
}

pub(crate) fn encode_file_fields(out: &mut Vec<u8>, file: &StoredFile) -> Result<(), ArchiveError> {
    out.extend_from_slice(&file.id.0);
    out.extend_from_slice(&file.content_id.0);
    out.extend_from_slice(&file.byte_length.to_le_bytes());
    write_string(out, &file.filename)?;
    write_string(out, file.mime_type.as_deref().unwrap_or(""))?;
    Ok(())
}

pub(crate) fn decode_file_fields(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<StoredFile, ArchiveError> {
    let id = FileId(read_fixed(bytes, cursor)?);
    let content_id = ContentId(read_fixed(bytes, cursor)?);
    let byte_length = read_u64(bytes, cursor)?;
    let filename = read_string(bytes, cursor)?;
    let mime_type = read_string(bytes, cursor)?;
    Ok(StoredFile {
        id,
        content_id,
        filename,
        mime_type: (!mime_type.is_empty()).then_some(mime_type),
        byte_length,
    })
}

pub(crate) fn encode_conversation_tombstone_fields(
    out: &mut Vec<u8>,
    tombstone: &ConversationTombstone,
) -> Result<(), ArchiveError> {
    write_string(out, &tombstone.conversation_id)?;
    write_strings(out, &tombstone.closure.node_ids)?;
    write_strings(out, &tombstone.closure.branch_ids)?;
    write_ids(out, tombstone.closure.fragment_ids.iter().map(|id| id.0))?;
    write_ids(out, tombstone.closure.episode_ids.iter().map(|id| id.0))?;
    write_len(out, tombstone.closure.file_ids.len())?;
    for file_id in &tombstone.closure.file_ids {
        out.extend_from_slice(&file_id.0);
    }
    write_len(out, tombstone.closure.metadata_archive_versions.len())?;
    for version in &tombstone.closure.metadata_archive_versions {
        out.extend_from_slice(&version.to_le_bytes());
    }
    Ok(())
}

pub(crate) fn decode_conversation_tombstone_fields(
    bytes: &[u8],
    cursor: &mut usize,
    archive_version: u64,
    global_version: u64,
) -> Result<ConversationTombstone, ArchiveError> {
    let conversation_id = read_string(bytes, cursor)?;
    let node_ids = read_strings(bytes, cursor)?;
    let branch_ids = read_strings(bytes, cursor)?;
    let fragment_ids = read_ids(bytes, cursor)?
        .into_iter()
        .map(FragmentId)
        .collect();
    let episode_ids = read_ids(bytes, cursor)?
        .into_iter()
        .map(crate::EpisodeId)
        .collect();
    let file_count = read_len(bytes, cursor)?;
    let mut file_ids = Vec::with_capacity(file_count);
    for _ in 0..file_count {
        file_ids.push(FileId(read_fixed_named(
            bytes,
            cursor,
            "truncated tombstone file id",
        )?));
    }
    let metadata_count = read_len(bytes, cursor)?;
    let mut metadata_archive_versions = Vec::with_capacity(metadata_count);
    for _ in 0..metadata_count {
        metadata_archive_versions.push(u64::from_le_bytes(read_fixed_named(
            bytes,
            cursor,
            "truncated tombstone metadata version",
        )?));
    }
    Ok(ConversationTombstone {
        conversation_id,
        closure: ConversationTombstoneClosure {
            node_ids,
            branch_ids,
            fragment_ids,
            episode_ids,
            file_ids,
            metadata_archive_versions,
        },
        archive_version,
        global_version,
    })
}

pub(crate) fn decode_content_id(bytes: &[u8]) -> Result<Option<ContentId>, ArchiveError> {
    if bytes.len() < 8 || bytes[..8] != CONTENT_MAGIC {
        return Ok(None);
    }
    if bytes.len() < 44 {
        return Err(ArchiveError::CorruptRecord("short content record"));
    }
    let len = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    if 44_usize.checked_add(len) != Some(bytes.len()) {
        return Err(ArchiveError::CorruptRecord("invalid content length"));
    }
    Ok(Some(ContentId(bytes[8..40].try_into().unwrap())))
}

pub fn decode_record(bytes: &[u8]) -> Result<ArchiveRecord, ArchiveError> {
    if bytes.len() < 8 {
        return Ok(ArchiveRecord::Other);
    }
    if bytes[..8] == *crate::archive_import::IMPORT_MAGIC {
        return crate::archive_import::decode(bytes).map(ArchiveRecord::ImportReceipt);
    }
    if bytes[..8] == CONTENT_MAGIC {
        return decode_content(bytes);
    }
    if bytes[..8] == NODE_MAGIC {
        return decode_node(bytes);
    }
    if bytes[..8] == BRANCH_MAGIC {
        return decode_branch(bytes);
    }
    if bytes[..8] == CONVERSATION_TOMBSTONE_MAGIC {
        return decode_conversation_tombstone(bytes);
    }
    if bytes[..8] == CONVERSATION_METADATA_MAGIC {
        return decode_conversation_metadata(bytes).map(ArchiveRecord::ConversationMetadata);
    }
    if bytes[..8] == FRAGMENT_MAGIC {
        return decode_fragment(bytes);
    }
    if bytes[..8] == FILE_MAGIC {
        return decode_file(bytes);
    }
    if bytes[..8] == INGESTED_TURN_MAGIC {
        return decode_ingested_turn(bytes).map(ArchiveRecord::IngestedTurn);
    }
    if bytes[..8] == FILE_MEMORY_LINK_MAGIC {
        return decode_file_memory_link(bytes).map(ArchiveRecord::FileMemoryLink);
    }
    if bytes[..8] == EPISODE_MAGIC {
        return decode_episode(bytes)?
            .map(ArchiveRecord::Episode)
            .ok_or(ArchiveError::CorruptRecord("invalid episode record"));
    }
    Ok(ArchiveRecord::Other)
}

fn decode_content(bytes: &[u8]) -> Result<ArchiveRecord, ArchiveError> {
    if bytes.len() < 44 {
        return Err(ArchiveError::CorruptRecord("short content record"));
    }
    let id = ContentId(bytes[8..40].try_into().unwrap());
    let len = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    let body = bytes
        .get(44..44 + len)
        .ok_or(ArchiveError::CorruptRecord("truncated content"))?;
    if 44 + len != bytes.len() {
        return Err(ArchiveError::CorruptRecord("content trailing bytes"));
    }
    Ok(ArchiveRecord::Content(id, body.to_vec()))
}

fn decode_node(bytes: &[u8]) -> Result<ArchiveRecord, ArchiveError> {
    if bytes.len() < 48 {
        return Err(ArchiveError::CorruptRecord("short node record"));
    }
    let mut cursor = 8;
    let node = decode_node_fields(bytes, &mut cursor)?;
    require_end(bytes, cursor)?;
    Ok(ArchiveRecord::Node(node))
}

fn decode_branch(bytes: &[u8]) -> Result<ArchiveRecord, ArchiveError> {
    if bytes.len() < 9 {
        return Err(ArchiveError::CorruptRecord("short branch record"));
    }
    let mut cursor = 8;
    let branch = decode_branch_fields(bytes, &mut cursor)?;
    require_end(bytes, cursor)?;
    Ok(ArchiveRecord::Branch(branch))
}

fn decode_conversation_tombstone(bytes: &[u8]) -> Result<ArchiveRecord, ArchiveError> {
    if bytes.len() < 24 {
        return Err(ArchiveError::CorruptRecord("short conversation tombstone"));
    }
    let archive_version = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let global_version = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
    let mut cursor = 24;
    let tombstone =
        decode_conversation_tombstone_fields(bytes, &mut cursor, archive_version, global_version)?;
    require_end(bytes, cursor)?;
    Ok(ArchiveRecord::ConversationTombstone(tombstone))
}

fn decode_fragment(bytes: &[u8]) -> Result<ArchiveRecord, ArchiveError> {
    if bytes.len() < 40 {
        return Err(ArchiveError::CorruptRecord("short fragment record"));
    }
    let mut cursor = 8;
    let fragment = decode_fragment_fields(bytes, &mut cursor)?;
    require_end(bytes, cursor)?;
    Ok(ArchiveRecord::Fragment(fragment))
}

fn decode_file(bytes: &[u8]) -> Result<ArchiveRecord, ArchiveError> {
    if bytes.len() < 80 {
        return Err(ArchiveError::CorruptRecord("short file record"));
    }
    let mut cursor = 8;
    let file = decode_file_fields(bytes, &mut cursor)?;
    require_end(bytes, cursor)?;
    Ok(ArchiveRecord::File(file))
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), ArchiveError> {
    let len = u32::try_from(value.len()).map_err(|_| ArchiveError::FieldTooLarge)?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_len(out: &mut Vec<u8>, len: usize) -> Result<(), ArchiveError> {
    let len = u32::try_from(len).map_err(|_| ArchiveError::FieldTooLarge)?;
    out.extend_from_slice(&len.to_le_bytes());
    Ok(())
}

fn write_strings(out: &mut Vec<u8>, values: &[String]) -> Result<(), ArchiveError> {
    write_len(out, values.len())?;
    for value in values {
        write_string(out, value)?;
    }
    Ok(())
}

fn write_ids(
    out: &mut Vec<u8>,
    values: impl ExactSizeIterator<Item = [u8; 32]>,
) -> Result<(), ArchiveError> {
    write_len(out, values.len())?;
    for value in values {
        out.extend_from_slice(&value);
    }
    Ok(())
}

fn read_fixed<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], ArchiveError> {
    read_fixed_named(bytes, cursor, "truncated fixed field")
}

fn read_fixed_named<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
    error: &'static str,
) -> Result<[u8; N], ArchiveError> {
    let end = cursor.checked_add(N).ok_or(ArchiveError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(ArchiveError::CorruptRecord(error))?;
    let value = raw
        .try_into()
        .map_err(|_| ArchiveError::CorruptRecord("invalid fixed field"))?;
    *cursor = end;
    Ok(value)
}

fn read_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8, ArchiveError> {
    Ok(read_fixed::<1>(bytes, cursor)?[0])
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, ArchiveError> {
    Ok(u64::from_le_bytes(read_fixed(bytes, cursor)?))
}

fn read_i64(bytes: &[u8], cursor: &mut usize) -> Result<i64, ArchiveError> {
    Ok(i64::from_le_bytes(read_fixed(bytes, cursor)?))
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, ArchiveError> {
    let length_end = cursor.checked_add(4).ok_or(ArchiveError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..length_end)
        .ok_or(ArchiveError::CorruptRecord("missing string length"))?;
    let len = u32::from_le_bytes(raw.try_into().unwrap()) as usize;
    *cursor = length_end;
    let end = cursor.checked_add(len).ok_or(ArchiveError::FieldTooLarge)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(ArchiveError::CorruptRecord("truncated string"))?;
    *cursor = end;
    String::from_utf8(value.to_vec()).map_err(|_| ArchiveError::InvalidUtf8)
}

fn read_len(bytes: &[u8], cursor: &mut usize) -> Result<usize, ArchiveError> {
    let end = cursor.checked_add(4).ok_or(ArchiveError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(ArchiveError::CorruptRecord("missing collection length"))?;
    *cursor = end;
    Ok(u32::from_le_bytes(raw.try_into().unwrap()) as usize)
}

fn read_strings(bytes: &[u8], cursor: &mut usize) -> Result<Vec<String>, ArchiveError> {
    let len = read_len(bytes, cursor)?;
    let mut values = Vec::with_capacity(len);
    for _ in 0..len {
        values.push(read_string(bytes, cursor)?);
    }
    Ok(values)
}

fn read_ids(bytes: &[u8], cursor: &mut usize) -> Result<Vec<[u8; 32]>, ArchiveError> {
    let len = read_len(bytes, cursor)?;
    let mut values = Vec::with_capacity(len);
    for _ in 0..len {
        let end = cursor.checked_add(32).ok_or(ArchiveError::FieldTooLarge)?;
        let raw = bytes
            .get(*cursor..end)
            .ok_or(ArchiveError::CorruptRecord("truncated tombstone id"))?;
        values.push(raw.try_into().unwrap());
        *cursor = end;
    }
    Ok(values)
}

fn require_end(bytes: &[u8], cursor: usize) -> Result<(), ArchiveError> {
    if cursor == bytes.len() {
        Ok(())
    } else {
        Err(ArchiveError::CorruptRecord("trailing bytes"))
    }
}

#[path = "archive_publication_codec.rs"]
pub(crate) mod publication;
