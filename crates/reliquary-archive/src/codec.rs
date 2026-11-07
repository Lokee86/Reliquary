use crate::FragmentDefinition;
use crate::{
    ArchiveError, Branch, ContentId, Episode, EpisodeBoundary, EpisodeId, EpisodeOrigin,
    ImportReceipt, Node, ProjectFileRef, ProjectRepositoryKind, ProjectRepositoryRef,
    ProjectRevisionRef, SourceAttachment,
};

const TURN_MAGIC: [u8; 8] = *b"RELTURN2";
const FILE_MAGIC: [u8; 8] = *b"RELFILE1";
const BRANCH_MAGIC: [u8; 8] = *b"RELBRCH1";
const EPISODE_MAGIC: [u8; 8] = *b"RELEPIS1";
const IMPORT_MAGIC: [u8; 8] = *b"CVAIMPT1";
const FRAGMENT_DEFINITION_MAGIC: [u8; 8] = *b"CVAFRG01";
const TOMBSTONE_MAGIC: [u8; 8] = *b"RELCTM01";
const TITLE_MAGIC: [u8; 8] = *b"RELTIT01";
const PRUNED_TURN_MAGIC: [u8; 8] = *b"RELTPR01";
const MIN_ATTACHMENT_RECORD_LEN: usize = 32 + 32 + 8 + 4 + 1 + 1;

pub(crate) enum Record {
    Turn(Node, String, Vec<SourceAttachment>),
    FileBlob(ContentId, Vec<u8>),
    Branch(Branch),
    Episode(Episode),
    Import(ImportReceipt),
    FragmentDefinitions(Vec<FragmentDefinition>),
    ConversationTombstone(String),
    ConversationTitle(String, String),
    PrunedTurn(Node, Vec<SourceAttachment>, [u8; 32]),
}

pub fn prune_turn_frame(
    bytes: &[u8],
    manifest_digest: [u8; 32],
) -> Result<Option<(Vec<u8>, Node)>, ArchiveError> {
    let Some(Record::Turn(node, _, attachments)) = decode_record(bytes)? else {
        return Ok(None);
    };
    let mut encoded = encode_turn(&node, "", &attachments)?;
    encoded[..8].copy_from_slice(&PRUNED_TURN_MAGIC);
    encoded.splice(8..8, manifest_digest);
    Ok(Some((encoded, node)))
}

pub fn archive_turn_prune_marker(bytes: &[u8]) -> Result<Option<(Node, [u8; 32])>, ArchiveError> {
    let Some(Record::PrunedTurn(node, _, digest)) = decode_record(bytes)? else {
        return Ok(None);
    };
    Ok(Some((node, digest)))
}

pub(crate) fn encode_conversation_tombstone(
    conversation_id: &str,
) -> Result<Vec<u8>, ArchiveError> {
    let mut out = TOMBSTONE_MAGIC.to_vec();
    write_string(&mut out, conversation_id)?;
    Ok(out)
}

pub(crate) fn encode_conversation_title(
    conversation_id: &str,
    title: &str,
) -> Result<Vec<u8>, ArchiveError> {
    let mut out = TITLE_MAGIC.to_vec();
    write_string(&mut out, conversation_id)?;
    write_string(&mut out, title)?;
    Ok(out)
}

pub(crate) fn encode_turn(
    node: &Node,
    content: &str,
    attachments: &[SourceAttachment],
) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::new();
    out.extend_from_slice(&TURN_MAGIC);
    out.extend_from_slice(&node.timestamp_ns.to_le_bytes());
    out.extend_from_slice(&node.content_id.0);
    write_string(&mut out, &node.id)?;
    write_string(&mut out, &node.conversation_id)?;
    write_optional_string(&mut out, node.parent_id.as_deref())?;
    write_string(&mut out, &node.role)?;
    write_optional_string(&mut out, node.principal_id.as_deref())?;
    write_string(&mut out, content)?;
    write_u32(&mut out, attachments.len())?;
    for attachment in attachments {
        out.extend_from_slice(&attachment.id);
        out.extend_from_slice(&attachment.content_id.0);
        out.extend_from_slice(&attachment.byte_length.to_le_bytes());
        write_string(&mut out, &attachment.filename)?;
        write_optional_string(&mut out, attachment.mime_type.as_deref())?;
        if let Some(reference) = &attachment.project_reference {
            out.push(1);
            out.push(match reference.revision.repository.kind {
                ProjectRepositoryKind::Lore => 1,
                ProjectRepositoryKind::Git => 2,
            });
            write_string(&mut out, &reference.revision.repository.repository_id)?;
            write_string(&mut out, &reference.revision.repository.project_path)?;
            write_string(&mut out, &reference.revision.revision)?;
            write_string(&mut out, &reference.path)?;
            match reference.content_hash {
                Some(hash) => {
                    out.push(1);
                    out.extend_from_slice(&hash);
                }
                None => out.push(0),
            }
        } else {
            out.push(0);
        }
    }
    Ok(out)
}

pub(crate) fn encode_file_blob(
    content_id: ContentId,
    bytes: &[u8],
) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::new();
    out.extend_from_slice(&FILE_MAGIC);
    out.extend_from_slice(&content_id.0);
    write_bytes(&mut out, bytes)?;
    Ok(out)
}

pub(crate) fn encode_branch(branch: &Branch) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::new();
    out.extend_from_slice(&BRANCH_MAGIC);
    out.push(u8::from(branch.canonical));
    write_string(&mut out, &branch.id)?;
    write_string(&mut out, &branch.conversation_id)?;
    write_string(&mut out, &branch.leaf_node_id)?;
    Ok(out)
}

pub(crate) fn encode_episode(episode: &Episode) -> Result<Vec<u8>, ArchiveError> {
    let mut out = Vec::new();
    out.extend_from_slice(&EPISODE_MAGIC);
    out.extend_from_slice(&episode.id.0);
    out.push(episode.origin.tag());
    out.push(episode.boundary.tag());
    out.extend_from_slice(&episode.source_through_ns.to_le_bytes());
    out.extend_from_slice(&episode.finalized_at_ns.to_le_bytes());
    write_string(&mut out, &episode.conversation_id)?;
    write_string(&mut out, &episode.start_node_id)?;
    write_string(&mut out, &episode.end_node_id)?;
    Ok(out)
}

pub(crate) fn encode_import_receipt(receipt: &ImportReceipt) -> Result<Vec<u8>, ArchiveError> {
    let mut out = IMPORT_MAGIC.to_vec();
    out.extend(
        serde_json::to_vec(receipt)
            .map_err(|_| ArchiveError::CorruptRecord("invalid import receipt"))?,
    );
    Ok(out)
}

pub(crate) fn encode_fragment_definitions(
    definitions: &[FragmentDefinition],
) -> Result<Vec<u8>, ArchiveError> {
    let mut out = FRAGMENT_DEFINITION_MAGIC.to_vec();
    out.extend(
        serde_json::to_vec(definitions)
            .map_err(|_| ArchiveError::CorruptRecord("invalid fragment definition"))?,
    );
    Ok(out)
}

pub(crate) fn decode_record(bytes: &[u8]) -> Result<Option<Record>, ArchiveError> {
    let Some(magic) = bytes.get(..8) else {
        return Ok(None);
    };
    if magic == TURN_MAGIC {
        decode_turn(bytes).map(Some)
    } else if magic == FILE_MAGIC {
        decode_file_blob(bytes).map(Some)
    } else if magic == BRANCH_MAGIC {
        decode_branch(bytes).map(Some)
    } else if magic == EPISODE_MAGIC {
        decode_episode(bytes).map(Some)
    } else if magic == IMPORT_MAGIC {
        let receipt = serde_json::from_slice(bytes.get(8..).unwrap_or_default())
            .map_err(|_| ArchiveError::CorruptRecord("invalid import receipt"))?;
        Ok(Some(Record::Import(receipt)))
    } else if magic == FRAGMENT_DEFINITION_MAGIC {
        let definitions = serde_json::from_slice(bytes.get(8..).unwrap_or_default())
            .map_err(|_| ArchiveError::CorruptRecord("invalid fragment definition"))?;
        Ok(Some(Record::FragmentDefinitions(definitions)))
    } else if magic == TOMBSTONE_MAGIC {
        let mut cursor = 8;
        let id = read_string(bytes, &mut cursor)?;
        require_end(bytes, cursor)?;
        Ok(Some(Record::ConversationTombstone(id)))
    } else if magic == TITLE_MAGIC {
        let mut cursor = 8;
        let conversation_id = read_string(bytes, &mut cursor)?;
        let title = read_string(bytes, &mut cursor)?;
        require_end(bytes, cursor)?;
        Ok(Some(Record::ConversationTitle(conversation_id, title)))
    } else if magic == PRUNED_TURN_MAGIC {
        if bytes.len() < 40 {
            return Err(ArchiveError::CorruptRecord("short pruned turn"));
        }
        let digest: [u8; 32] = bytes[8..40].try_into().unwrap();
        if digest == [0; 32] {
            return Err(ArchiveError::CorruptRecord("empty pruned turn proof"));
        }
        let mut restored = TURN_MAGIC.to_vec();
        restored.extend_from_slice(&bytes[40..]);
        let Record::Turn(node, content, attachments) = decode_turn(&restored)? else {
            unreachable!()
        };
        if !content.is_empty() {
            return Err(ArchiveError::CorruptRecord("pruned turn carries content"));
        }
        Ok(Some(Record::PrunedTurn(node, attachments, digest)))
    } else {
        Ok(None)
    }
}

fn decode_turn(bytes: &[u8]) -> Result<Record, ArchiveError> {
    let mut cursor = 8;
    let timestamp_ns = i64::from_le_bytes(read_fixed(bytes, &mut cursor, "short turn record")?);
    let content_id = ContentId(read_fixed(bytes, &mut cursor, "short turn content ID")?);
    let id = read_string(bytes, &mut cursor)?;
    let conversation_id = read_string(bytes, &mut cursor)?;
    let parent_id = read_optional_string(bytes, &mut cursor)?;
    let role = read_string(bytes, &mut cursor)?;
    let principal_id = read_optional_string(bytes, &mut cursor)?;
    let content = read_string(bytes, &mut cursor)?;
    let count = read_u32(bytes, &mut cursor)? as usize;
    if count > bytes.len().saturating_sub(cursor) / MIN_ATTACHMENT_RECORD_LEN {
        return Err(ArchiveError::CorruptRecord("impossible attachment count"));
    }
    let mut attachments = Vec::with_capacity(count);
    for _ in 0..count {
        let id = read_fixed(bytes, &mut cursor, "short source attachment ID")?;
        let attachment_content_id = ContentId(read_fixed(
            bytes,
            &mut cursor,
            "short source attachment content ID",
        )?);
        let byte_length = u64::from_le_bytes(read_fixed(
            bytes,
            &mut cursor,
            "short source attachment length",
        )?);
        let filename = read_string(bytes, &mut cursor)?;
        let mime_type = read_optional_string(bytes, &mut cursor)?;
        let project_reference =
            match read_fixed::<1>(bytes, &mut cursor, "missing project reference flag")?[0] {
                0 => None,
                1 => {
                    let kind = match read_fixed::<1>(bytes, &mut cursor, "missing repository kind")?
                        [0]
                    {
                        1 => ProjectRepositoryKind::Lore,
                        2 => ProjectRepositoryKind::Git,
                        _ => return Err(ArchiveError::CorruptRecord("invalid repository kind")),
                    };
                    let repository_id = read_string(bytes, &mut cursor)?;
                    let project_path = read_string(bytes, &mut cursor)?;
                    let revision = read_string(bytes, &mut cursor)?;
                    let path = read_string(bytes, &mut cursor)?;
                    let content_hash = match read_fixed::<1>(
                        bytes,
                        &mut cursor,
                        "missing project content hash flag",
                    )?[0]
                    {
                        0 => None,
                        1 => Some(read_fixed::<32>(
                            bytes,
                            &mut cursor,
                            "short project content hash",
                        )?),
                        _ => {
                            return Err(ArchiveError::CorruptRecord(
                                "invalid project content hash flag",
                            ));
                        }
                    };
                    Some(ProjectFileRef {
                        revision: ProjectRevisionRef {
                            repository: ProjectRepositoryRef {
                                kind,
                                repository_id,
                                project_path,
                            },
                            revision,
                        },
                        path,
                        content_hash,
                    })
                }
                _ => {
                    return Err(ArchiveError::CorruptRecord(
                        "invalid project reference flag",
                    ));
                }
            };
        attachments.push(SourceAttachment {
            id,
            content_id: attachment_content_id,
            filename,
            mime_type,
            byte_length,
            bytes: Vec::new(),
            project_reference,
        });
    }
    require_end(bytes, cursor)?;
    Ok(Record::Turn(
        Node {
            id,
            conversation_id,
            parent_id,
            role,
            principal_id,
            timestamp_ns,
            content_id,
        },
        content,
        attachments,
    ))
}

fn decode_file_blob(bytes: &[u8]) -> Result<Record, ArchiveError> {
    let mut cursor = 8;
    let content_id = ContentId(read_fixed(bytes, &mut cursor, "short file content ID")?);
    let content = read_bytes(bytes, &mut cursor)?;
    require_end(bytes, cursor)?;
    Ok(Record::FileBlob(content_id, content))
}

fn decode_branch(bytes: &[u8]) -> Result<Record, ArchiveError> {
    let mut cursor = 8;
    let canonical = match read_fixed::<1>(bytes, &mut cursor, "short branch record")?[0] {
        0 => false,
        1 => true,
        _ => return Err(ArchiveError::CorruptRecord("invalid branch flag")),
    };
    let branch = Branch {
        id: read_string(bytes, &mut cursor)?,
        conversation_id: read_string(bytes, &mut cursor)?,
        leaf_node_id: read_string(bytes, &mut cursor)?,
        canonical,
    };
    require_end(bytes, cursor)?;
    Ok(Record::Branch(branch))
}

fn decode_episode(bytes: &[u8]) -> Result<Record, ArchiveError> {
    let mut cursor = 8;
    let id = EpisodeId(read_fixed(bytes, &mut cursor, "short Episode ID")?);
    let origin =
        EpisodeOrigin::from_tag(read_fixed::<1>(bytes, &mut cursor, "short Episode origin")?[0])
            .ok_or(ArchiveError::CorruptRecord("invalid Episode origin"))?;
    let boundary = EpisodeBoundary::from_tag(
        read_fixed::<1>(bytes, &mut cursor, "short Episode boundary")?[0],
    )
    .ok_or(ArchiveError::CorruptRecord("invalid Episode boundary"))?;
    let source_through_ns =
        i64::from_le_bytes(read_fixed(bytes, &mut cursor, "short Episode source time")?);
    let finalized_at_ns =
        i64::from_le_bytes(read_fixed(bytes, &mut cursor, "short Episode final time")?);
    let episode = Episode {
        id,
        conversation_id: read_string(bytes, &mut cursor)?,
        start_node_id: read_string(bytes, &mut cursor)?,
        end_node_id: read_string(bytes, &mut cursor)?,
        origin,
        boundary,
        source_through_ns,
        finalized_at_ns,
    };
    require_end(bytes, cursor)?;
    Ok(Record::Episode(episode))
}

fn write_optional_string(out: &mut Vec<u8>, value: Option<&str>) -> Result<(), ArchiveError> {
    match value {
        Some(value) => {
            out.push(1);
            write_string(out, value)?;
        }
        None => out.push(0),
    }
    Ok(())
}

fn read_optional_string(bytes: &[u8], cursor: &mut usize) -> Result<Option<String>, ArchiveError> {
    match read_fixed::<1>(bytes, cursor, "short optional string flag")?[0] {
        0 => Ok(None),
        1 => Ok(Some(read_string(bytes, cursor)?)),
        _ => Err(ArchiveError::CorruptRecord("invalid optional string flag")),
    }
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), ArchiveError> {
    let bytes = value.as_bytes();
    write_bytes(out, bytes)
}

fn write_u32(out: &mut Vec<u8>, value: usize) -> Result<(), ArchiveError> {
    let value = u32::try_from(value).map_err(|_| ArchiveError::FieldTooLarge)?;
    out.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, ArchiveError> {
    Ok(u32::from_le_bytes(read_fixed(
        bytes,
        cursor,
        "short source attachment length",
    )?))
}

fn write_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), ArchiveError> {
    let length = u32::try_from(value.len()).map_err(|_| ArchiveError::FieldTooLarge)?;
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, ArchiveError> {
    String::from_utf8(read_bytes(bytes, cursor)?).map_err(|_| ArchiveError::InvalidUtf8)
}

fn read_bytes(bytes: &[u8], cursor: &mut usize) -> Result<Vec<u8>, ArchiveError> {
    let length = u32::from_le_bytes(read_fixed(bytes, cursor, "missing field length")?) as usize;
    let end = cursor
        .checked_add(length)
        .ok_or(ArchiveError::FieldTooLarge)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(ArchiveError::CorruptRecord("truncated field"))?;
    *cursor = end;
    Ok(value.to_vec())
}

fn read_fixed<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
    reason: &'static str,
) -> Result<[u8; N], ArchiveError> {
    let end = cursor.checked_add(N).ok_or(ArchiveError::FieldTooLarge)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(ArchiveError::CorruptRecord(reason))?;
    *cursor = end;
    Ok(value.try_into().unwrap())
}

fn require_end(bytes: &[u8], cursor: usize) -> Result<(), ArchiveError> {
    if cursor == bytes.len() {
        Ok(())
    } else {
        Err(ArchiveError::CorruptRecord("trailing bytes"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_decoder_rejects_impossible_attachment_count_before_allocating() {
        let mut bytes = Vec::from(TURN_MAGIC);
        bytes.extend_from_slice(&0i64.to_le_bytes());
        bytes.extend_from_slice(&[0; 32]);
        for field in [b"".as_slice(), b"".as_slice()] {
            bytes.extend_from_slice(&(field.len() as u32).to_le_bytes());
            bytes.extend_from_slice(field);
        }
        bytes.push(0); // parent
        bytes.extend_from_slice(&0u32.to_le_bytes()); // role
        bytes.push(0); // principal
        bytes.extend_from_slice(&0u32.to_le_bytes()); // content
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());

        assert!(matches!(
            decode_record(&bytes),
            Err(ArchiveError::CorruptRecord("impossible attachment count"))
        ));
    }
}
