use super::*;
use crate::records::memory_prune_proof::{memory_history_record, pointer_free_frame};
use reliquary_container::{ObjectRef, decode_publication};
use reliquary_memory_core::MemoryBodyId;
use std::io::Read;

pub(super) fn open_state(
    source: &Container,
    admission: &GraphAdmission,
) -> Result<
    (
        CommonState,
        Vec<ArchiveVectorInfo>,
        BTreeSet<[u8; 32]>,
        BTreeSet<[u8; 32]>,
    ),
    String,
> {
    match source.identity().file_kind {
        FileKind::Phylactery => PhySession::open(source, admission, [])
            .map(|session| (session.common, Vec::new(), BTreeSet::new(), BTreeSet::new())),
        FileKind::Reliquary => RelSession::open(source, admission, [], []).map(|session| {
            let files = session
                .archive
                .accepted_turn_sources()
                .into_iter()
                .flat_map(|source| {
                    session
                        .archive
                        .files_for_source(&source.conversation_id, &source.node_id)
                })
                .map(|file| file.content_id.0)
                .collect();
            let retained_archive_vectors = session
                .generations
                .generations()
                .iter()
                .map(|generation| generation.archive_vector_id.0)
                .collect();
            (
                session.common,
                session.archive_vectors.infos(),
                retained_archive_vectors,
                files,
            )
        }),
    }
    .map_err(|e| e.to_string())
}

pub(super) fn removable_backing(
    bytes: &[u8],
    memories: &MemoryStore,
    packed: &BTreeSet<[u8; 32]>,
    archive_vectors: &BTreeSet<[u8; 32]>,
    archive_files: &BTreeSet<[u8; 32]>,
) -> Result<bool, String> {
    let magic = bytes.get(..8).ok_or("short frame in reclamation source")?;
    match magic {
        b"RELMBOD1" | b"RELPVEC1" => {
            let id: [u8; 32] = bytes
                .get(8..40)
                .ok_or("truncated backing identity")?
                .try_into()
                .unwrap();
            Ok(if magic == b"RELMBOD1" {
                !memories.references_body(MemoryBodyId(id))
            } else {
                !packed.contains(&id)
            })
        }
        b"RELAVEC1" => {
            let (id, _) = reliquary_vector_store::archive_vector_object_packed_id(bytes)
                .map_err(|e| e.to_string())?
                .ok_or("missing Archive-vector mapping identity")?;
            Ok(!archive_vectors.contains(&id.0))
        }
        b"RELFILE1" => {
            let id: [u8; 32] = bytes
                .get(8..40)
                .ok_or("truncated Archive file identity")?
                .try_into()
                .unwrap();
            Ok(!archive_files.contains(&id))
        }
        b"RELCTM01" => {
            validate_conversation_tombstone(bytes)?;
            Ok(false)
        }
        b"RELTPR01" => {
            reliquary_archive::archive_turn_prune_marker(bytes)
                .map_err(|error| format!("invalid Archive prune marker: {error}"))?
                .ok_or("invalid Archive prune marker")?;
            Ok(false)
        }
        b"RELMCOR1" | b"RELMRET1" => {
            crate::records::user_memory::UserMemoryRecord::decode(bytes)?
                .ok_or("missing grouped user Memory mutation")?;
            Ok(false)
        }
        b"RELPUB01" => {
            let publication = decode_publication(bytes)
                .map_err(|e| e.to_string())?
                .ok_or("missing reclamation publication envelope")?;
            if !memory_history_record(publication.body) {
                return Err("unsupported reclamation publication carrier".into());
            }
            Ok(false)
        }
        // These are logical, pointer-free operation/history records. The
        // owner reader above has already decoded and replay-validated them;
        // retain them byte-for-byte as journals and authority evidence.
        b"RELMVEC1" | b"RELCPRO1" | b"RELCPRT1" | b"RELCPRP1" | b"RELRECL1" | b"RELFRS02"
        | b"RELHPR01" | b"RELVGEN1" | b"RELVGRC1" | b"RELXIDX2" | b"RELIEPS2" | b"RELIEPC2"
        | b"RELIFRS1" | b"RELIPLN1" | b"CVAIMPT1" | b"RELINSW1" | b"RELINSW2" | b"RELINSR1"
        | b"RELINSC7" | b"CVDREAM1" | b"CVDREAM2" | b"RELDRM01" | b"RELDRP02" | b"RELDRG01"
        | b"RELDRS01" | b"RELDRF01" | b"RELDRF02" | b"RELDRB01" | b"RELDRC01" | b"RELXMEM2"
        | b"RELXMEM3" | b"RELXMRP3" | b"RELXHST1" | b"RELPER01" | b"RELETM01" | b"RELRLP01"
        | b"RELGRF01" | b"RELCOM01" | b"RELCMP01" => Ok(false),
        _ if pointer_free_frame(magic) => Ok(false),
        _ => Err(format!(
            "unsupported reclamation frame family {}",
            String::from_utf8_lossy(magic)
        )),
    }
}

fn validate_conversation_tombstone(bytes: &[u8]) -> Result<(), String> {
    let length = bytes
        .get(8..12)
        .ok_or("truncated Archive conversation tombstone")?;
    let length = u32::from_le_bytes(length.try_into().unwrap()) as usize;
    let end = 12usize
        .checked_add(length)
        .ok_or("invalid Archive conversation tombstone length")?;
    let conversation_id = bytes
        .get(12..end)
        .ok_or("truncated Archive conversation tombstone id")?;
    if end != bytes.len()
        || conversation_id.is_empty()
        || std::str::from_utf8(conversation_id).is_err()
    {
        return Err("invalid Archive conversation tombstone".into());
    }
    Ok(())
}

/// Only a structurally valid, wholly unreceipted generation is discardable.
/// A dangling receipt, incorrect identity or ambiguous physical target stops.
pub(super) fn unactivated_generation_offsets(source: &Container) -> Result<BTreeSet<u64>, String> {
    use reliquary_vector_store::generation::{decode_generation, vector_generation_id};
    let mut generations = BTreeMap::new();
    let mut activated = BTreeSet::new();
    for_each_frame(source, |object, bytes| {
        if bytes.starts_with(b"RELVGEN1") {
            if source.identity().file_kind != FileKind::Reliquary {
                return Err("generation frame is not valid in PHY".into());
            }
            let generation = decode_generation(bytes)
                .map_err(|e| e.to_string())?
                .ok_or("missing generation identity")?;
            let expected = vector_generation_id(
                generation.compatibility_profile_id,
                generation.archive_vector_id,
                generation.source_archive_version,
            );
            if expected != generation.id {
                return Err("invalid unactivated generation identity".into());
            }
            if generations.insert(object.offset, object).is_some() {
                return Err("duplicate generation frame offset".into());
            }
        }
        if let Some(target) = generation_receipt_target(bytes).map_err(|e| e.to_string())? {
            let actual = generations
                .get(&target.offset)
                .ok_or("generation receipt has no physical target")?;
            if *actual != target {
                return Err("generation receipt target length mismatch".into());
            }
            activated.insert(target.offset);
        }
        Ok(())
    })?;
    Ok(generations
        .keys()
        .filter(|offset| !activated.contains(offset))
        .copied()
        .collect())
}

pub(super) fn for_each_frame(
    source: &Container,
    mut visit: impl FnMut(ObjectRef, &[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let mut cursor = source.physical_cursor().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    while let Some(mut frame) = cursor.next_frame().map_err(|e| e.to_string())? {
        bytes.clear();
        frame.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        visit(frame.object(), &bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod marker_tests {
    use super::validate_conversation_tombstone;

    #[test]
    fn archive_tombstone_marker_is_validated_and_retained() {
        let id = b"conv-terminal";
        let mut marker = b"RELCTM01".to_vec();
        marker.extend_from_slice(&(id.len() as u32).to_le_bytes());
        marker.extend_from_slice(id);
        assert_eq!(validate_conversation_tombstone(&marker), Ok(()));

        marker.pop();
        assert!(validate_conversation_tombstone(&marker).is_err());
        marker.push(*id.last().unwrap());
        marker.push(0);
        assert!(validate_conversation_tombstone(&marker).is_err());
    }

    #[test]
    fn archive_prune_marker_decoder_rejects_malformed_payload() {
        let marker = [b"RELTPR01".as_slice(), &[0; 32], b"not-a-turn"].concat();
        assert!(reliquary_archive::archive_turn_prune_marker(&marker).is_err());
    }
}
