use crate::conversation_metadata_codec::{
    ACTIVE_CONVERSATION_METADATA_MAGIC, CONVERSATION_METADATA_MAGIC,
    LEGACY_CONVERSATION_METADATA_MAGIC, decode_conversation_metadata,
};
use crate::episode_codec::{EPISODE_MAGIC, decode_episode};
use crate::file_memory_link_codec::{FILE_MEMORY_LINK_MAGIC, decode_file_memory_link};
use crate::turn_ingest_codec::{
    INGESTED_TURN_MAGIC, LEGACY_INGESTED_TURN_MAGIC, decode_ingested_turn,
};
use crate::{
    ArchiveError, Branch, ContentId, ConversationMetadata, Episode, FileId, FileMemoryLink,
    Fragment, FragmentId, IngestedTurn, Node, StoredFile,
};

const CONTENT_MAGIC: [u8; 8] = *b"CVACONT1";
const LEGACY_NODE_MAGIC: [u8; 8] = *b"CVANODE1";
const NODE_MAGIC: [u8; 8] = *b"CVANODE2";
const BRANCH_MAGIC: [u8; 8] = *b"CVABRCH1";
const FRAGMENT_MAGIC: [u8; 8] = *b"CVAFRAG1";
const FILE_MAGIC: [u8; 8] = *b"CVAFILE1";

pub enum ArchiveRecord {
    ImportReceipt(crate::imports::ImportReceipt),
    Content(ContentId, Vec<u8>),
    Node(Node),
    Branch(Branch),
    ConversationMetadata(ConversationMetadata),
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
    out.extend_from_slice(&node.timestamp_ns.to_le_bytes());
    out.extend_from_slice(&node.content_id.0);
    write_string(&mut out, &node.id)?;
    write_string(&mut out, &node.conversation_id)?;
    write_string(&mut out, node.parent_id.as_deref().unwrap_or(""))?;
    write_string(&mut out, &node.role)?;
    write_string(&mut out, node.principal_id.as_deref().unwrap_or(""))?;
    Ok(out)
}

pub fn encode_branch(branch: &Branch) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&BRANCH_MAGIC);
    out.push(u8::from(branch.canonical));
    write_string(&mut out, &branch.id)?;
    write_string(&mut out, &branch.conversation_id)?;
    write_string(&mut out, &branch.leaf_node_id)?;
    Ok(out)
}

pub fn encode_fragment(fragment: &Fragment) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::with_capacity(96);
    out.extend_from_slice(&FRAGMENT_MAGIC);
    out.extend_from_slice(&fragment.id.0);
    write_string(&mut out, &fragment.conversation_id)?;
    write_string(&mut out, &fragment.start_node_id)?;
    write_string(&mut out, &fragment.end_node_id)?;
    Ok(out)
}

pub fn encode_file(file: &StoredFile) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::with_capacity(128 + file.filename.len());
    out.extend_from_slice(&FILE_MAGIC);
    out.extend_from_slice(&file.id.0);
    out.extend_from_slice(&file.content_id.0);
    out.extend_from_slice(&file.byte_length.to_le_bytes());
    write_string(&mut out, &file.filename)?;
    write_string(&mut out, file.mime_type.as_deref().unwrap_or(""))?;
    Ok(out)
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
    if bytes[..8] == NODE_MAGIC || bytes[..8] == LEGACY_NODE_MAGIC {
        return decode_node(bytes);
    }
    if bytes[..8] == BRANCH_MAGIC {
        return decode_branch(bytes);
    }
    if bytes[..8] == CONVERSATION_METADATA_MAGIC
        || bytes[..8] == ACTIVE_CONVERSATION_METADATA_MAGIC
        || bytes[..8] == LEGACY_CONVERSATION_METADATA_MAGIC
    {
        return decode_conversation_metadata(bytes).map(ArchiveRecord::ConversationMetadata);
    }
    if bytes[..8] == FRAGMENT_MAGIC {
        return decode_fragment(bytes);
    }
    if bytes[..8] == FILE_MAGIC {
        return decode_file(bytes);
    }
    if bytes[..8] == INGESTED_TURN_MAGIC || bytes[..8] == LEGACY_INGESTED_TURN_MAGIC {
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
    let timestamp_ns = i64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let content_id = ContentId(bytes[16..48].try_into().unwrap());
    let mut cursor = 48;
    let id = read_string(bytes, &mut cursor)?;
    let conversation_id = read_string(bytes, &mut cursor)?;
    let parent = read_string(bytes, &mut cursor)?;
    let role = read_string(bytes, &mut cursor)?;
    let principal_id = if bytes[..8] == NODE_MAGIC {
        let principal = read_string(bytes, &mut cursor)?;
        (!principal.is_empty()).then_some(principal)
    } else {
        None
    };
    require_end(bytes, cursor)?;
    Ok(ArchiveRecord::Node(Node {
        id,
        conversation_id,
        parent_id: (!parent.is_empty()).then_some(parent),
        role,
        principal_id,
        timestamp_ns,
        content_id,
    }))
}

fn decode_branch(bytes: &[u8]) -> Result<ArchiveRecord, ArchiveError> {
    if bytes.len() < 9 {
        return Err(ArchiveError::CorruptRecord("short branch record"));
    }
    let canonical = match bytes[8] {
        0 => false,
        1 => true,
        _ => return Err(ArchiveError::CorruptRecord("invalid branch flag")),
    };
    let mut cursor = 9;
    let id = read_string(bytes, &mut cursor)?;
    let conversation_id = read_string(bytes, &mut cursor)?;
    let leaf_node_id = read_string(bytes, &mut cursor)?;
    require_end(bytes, cursor)?;
    Ok(ArchiveRecord::Branch(Branch {
        id,
        conversation_id,
        leaf_node_id,
        canonical,
    }))
}

fn decode_fragment(bytes: &[u8]) -> Result<ArchiveRecord, ArchiveError> {
    if bytes.len() < 40 {
        return Err(ArchiveError::CorruptRecord("short fragment record"));
    }
    let id = FragmentId(bytes[8..40].try_into().unwrap());
    let mut cursor = 40;
    let conversation_id = read_string(bytes, &mut cursor)?;
    let start_node_id = read_string(bytes, &mut cursor)?;
    let end_node_id = read_string(bytes, &mut cursor)?;
    require_end(bytes, cursor)?;
    Ok(ArchiveRecord::Fragment(Fragment {
        id,
        conversation_id,
        start_node_id,
        end_node_id,
    }))
}

fn decode_file(bytes: &[u8]) -> Result<ArchiveRecord, ArchiveError> {
    if bytes.len() < 80 {
        return Err(ArchiveError::CorruptRecord("short file record"));
    }
    let id = FileId(bytes[8..40].try_into().unwrap());
    let content_id = ContentId(bytes[40..72].try_into().unwrap());
    let byte_length = u64::from_le_bytes(bytes[72..80].try_into().unwrap());
    let mut cursor = 80;
    let filename = read_string(bytes, &mut cursor)?;
    let mime_type = read_string(bytes, &mut cursor)?;
    require_end(bytes, cursor)?;
    Ok(ArchiveRecord::File(StoredFile {
        id,
        content_id,
        filename,
        mime_type: (!mime_type.is_empty()).then_some(mime_type),
        byte_length,
    }))
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

fn require_end(bytes: &[u8], cursor: usize) -> Result<(), ArchiveError> {
    if cursor == bytes.len() {
        Ok(())
    } else {
        Err(ArchiveError::CorruptRecord("trailing bytes"))
    }
}
