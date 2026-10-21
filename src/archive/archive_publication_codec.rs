//! Archive-owned publication bodies. Physical envelopes supply global order.
use super::ArchiveRecord;
use crate::{ArchiveError, ArchiveRecordVersion, ObjectRef};

const NODE: &[u8; 8] = b"CVANOD02";
const BRANCH: &[u8; 8] = b"CVABRC02";
const METADATA: &[u8; 8] = b"CVACON04";
const TOMBSTONE: &[u8; 8] = b"CVACTM02";
const FRAGMENT: &[u8; 8] = b"CVAFRA02";
const EPISODE: &[u8; 8] = b"CVAEPI02";
const FILE: &[u8; 8] = b"CVAFIL02";
const TURN: &[u8; 8] = b"CVATUR03";
const IMPORT: &[u8; 8] = b"CVAIMP02";
const FILE_MEMORY: &[u8; 8] = b"CVAFME02";
const REPEAT: &[u8; 8] = b"CVAARP02";

pub(crate) enum ArchivePublication {
    Record {
        archive_version: u64,
        record: ArchiveRecord,
    },
    Repeat {
        archive_version: u64,
        first_applying_archive_version: u64,
    },
}

impl ArchivePublication {
    pub(crate) fn archive_version(&self) -> u64 {
        match self {
            Self::Record {
                archive_version, ..
            }
            | Self::Repeat {
                archive_version, ..
            } => *archive_version,
        }
    }
}

pub(crate) fn encode(publication: &ArchivePublication) -> Result<Vec<u8>, ArchiveError> {
    let local = publication.archive_version();
    if local == 0 {
        return Err(ArchiveError::InvalidArchiveRecordVersion);
    }
    let mut out = Vec::new();
    let magic = match publication {
        ArchivePublication::Repeat {
            first_applying_archive_version,
            ..
        } => {
            validate_repeat(local, *first_applying_archive_version)?;
            REPEAT
        }
        ArchivePublication::Record { record, .. } => match record {
            ArchiveRecord::Node(_) => NODE,
            ArchiveRecord::Branch(_) => BRANCH,
            ArchiveRecord::ConversationMetadata(_) => METADATA,
            ArchiveRecord::ConversationTombstone(value) => {
                if value.archive_version != local || value.global_version == 0 {
                    return Err(ArchiveError::InvalidArchiveRecordVersion);
                }
                TOMBSTONE
            }
            ArchiveRecord::Fragment(_) => FRAGMENT,
            ArchiveRecord::Episode(_) => EPISODE,
            ArchiveRecord::File(_) => FILE,
            ArchiveRecord::IngestedTurn(_) => TURN,
            ArchiveRecord::ImportReceipt(_) => IMPORT,
            ArchiveRecord::FileMemoryLink(_) => FILE_MEMORY,
            ArchiveRecord::Content(_, _) | ArchiveRecord::Other => {
                return Err(ArchiveError::CorruptRecord("not an Archive publication"));
            }
        },
    };
    out.extend_from_slice(magic);
    out.extend_from_slice(&local.to_le_bytes());
    match publication {
        ArchivePublication::Repeat {
            first_applying_archive_version,
            ..
        } => out.extend_from_slice(&first_applying_archive_version.to_le_bytes()),
        ArchivePublication::Record { record, .. } => match record {
            ArchiveRecord::Node(value) => super::encode_node_fields(&mut out, value)?,
            ArchiveRecord::Branch(value) => super::encode_branch_fields(&mut out, value)?,
            ArchiveRecord::ConversationMetadata(value) => {
                crate::conversation_metadata_codec::encode_conversation_metadata_fields(
                    &mut out, value,
                )?
            }
            ArchiveRecord::ConversationTombstone(value) => {
                super::encode_conversation_tombstone_fields(&mut out, value)?
            }
            ArchiveRecord::Fragment(value) => super::encode_fragment_fields(&mut out, value)?,
            ArchiveRecord::Episode(value) => {
                crate::episode_codec::encode_episode_fields(&mut out, value)?
            }
            ArchiveRecord::File(value) => super::encode_file_fields(&mut out, value)?,
            ArchiveRecord::IngestedTurn(value) => {
                crate::turn_ingest_codec::encode_ingested_turn_fields(&mut out, value)?
            }
            ArchiveRecord::ImportReceipt(value) => {
                crate::archive_import::encode_import_receipt_fields(&mut out, value)?
            }
            ArchiveRecord::FileMemoryLink(value) => {
                crate::file_memory_link_codec::encode_file_memory_link_fields(&mut out, value)?
            }
            ArchiveRecord::Content(_, _) | ArchiveRecord::Other => {
                unreachable!("rejected before encoding")
            }
        },
    }
    Ok(out)
}

pub(crate) fn decode(
    bytes: &[u8],
    global_version: u64,
) -> Result<Option<ArchivePublication>, ArchiveError> {
    let selected = [
        NODE,
        BRANCH,
        METADATA,
        TOMBSTONE,
        FRAGMENT,
        EPISODE,
        FILE,
        TURN,
        IMPORT,
        FILE_MEMORY,
        REPEAT,
    ]
    .into_iter()
    .find(|magic| bytes.starts_with(&magic[..6]));
    let Some(magic) = selected else {
        if bytes.starts_with(b"CVAAFM") || bytes.starts_with(b"CVAARE") {
            return Err(ArchiveError::CorruptRecord("nonpublication Archive body"));
        }
        return Ok(None);
    };
    if bytes.get(..8) != Some(magic.as_slice()) {
        return Err(ArchiveError::CorruptRecord(
            "unsupported Archive publication version",
        ));
    }
    if global_version == 0 {
        return Err(ArchiveError::InvalidArchiveRecordVersion);
    }
    let mut cursor = 8;
    let local = read_u64(bytes, &mut cursor)?;
    if local == 0 {
        return Err(ArchiveError::InvalidArchiveRecordVersion);
    }
    let publication = if magic == REPEAT {
        let first = read_u64(bytes, &mut cursor)?;
        validate_repeat(local, first)?;
        ArchivePublication::Repeat {
            archive_version: local,
            first_applying_archive_version: first,
        }
    } else {
        let record = if magic == NODE {
            ArchiveRecord::Node(super::decode_node_fields(bytes, &mut cursor, true)?)
        } else if magic == BRANCH {
            ArchiveRecord::Branch(super::decode_branch_fields(bytes, &mut cursor)?)
        } else if magic == METADATA {
            ArchiveRecord::ConversationMetadata(
                crate::conversation_metadata_codec::decode_conversation_metadata_fields(
                    bytes,
                    &mut cursor,
                )?,
            )
        } else if magic == TOMBSTONE {
            ArchiveRecord::ConversationTombstone(super::decode_conversation_tombstone_fields(
                bytes,
                &mut cursor,
                local,
                global_version,
            )?)
        } else if magic == FRAGMENT {
            ArchiveRecord::Fragment(super::decode_fragment_fields(bytes, &mut cursor)?)
        } else if magic == EPISODE {
            ArchiveRecord::Episode(crate::episode_codec::decode_episode_fields(
                bytes,
                &mut cursor,
            )?)
        } else if magic == FILE {
            ArchiveRecord::File(super::decode_file_fields(bytes, &mut cursor)?)
        } else if magic == TURN {
            ArchiveRecord::IngestedTurn(crate::turn_ingest_codec::decode_ingested_turn_fields(
                bytes,
                &mut cursor,
                true,
            )?)
        } else if magic == IMPORT {
            ArchiveRecord::ImportReceipt(crate::archive_import::decode_import_receipt_fields(
                bytes,
                &mut cursor,
            )?)
        } else {
            ArchiveRecord::FileMemoryLink(
                crate::file_memory_link_codec::decode_file_memory_link_fields(bytes, &mut cursor)?,
            )
        };
        ArchivePublication::Record {
            archive_version: local,
            record,
        }
    };
    if cursor != bytes.len() {
        return Err(ArchiveError::CorruptRecord(
            "Archive publication trailing bytes",
        ));
    }
    Ok(Some(publication))
}

fn validate_repeat(local: u64, first: u64) -> Result<(), ArchiveError> {
    if first == 0 || first >= local {
        return Err(ArchiveError::InvalidArchiveRecordVersion);
    }
    Ok(())
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, ArchiveError> {
    let end = cursor.checked_add(8).ok_or(ArchiveError::FieldTooLarge)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(ArchiveError::CorruptRecord("short Archive publication"))?;
    *cursor = end;
    Ok(u64::from_le_bytes(value.try_into().unwrap()))
}

/// Resolve only explicit publication coordinates. The owner admits the facts.
pub(crate) fn version_at(
    object: ObjectRef,
    payload: &[u8],
) -> Result<Option<ArchiveRecordVersion>, ArchiveError> {
    if let Some(version) = crate::archive_history_codec::decode_record_version(payload)? {
        return Ok(Some(version));
    }
    let Some(envelope) = crate::container::publication_envelope::decode(payload)? else {
        return Ok(None);
    };
    let Some(publication) = decode(envelope.body, envelope.global_version)? else {
        return Ok(None);
    };
    Ok(Some(ArchiveRecordVersion {
        global_version: envelope.global_version,
        archive_version: publication.archive_version(),
        record: object,
    }))
}

#[cfg(test)]
#[path = "archive_publication_bundle_tests.rs"]
mod bundle_tests;
#[cfg(test)]
#[path = "archive_publication_core_tests.rs"]
mod core_tests;
#[cfg(test)]
#[path = "archive_publication_extra_tests.rs"]
mod extra_tests;
