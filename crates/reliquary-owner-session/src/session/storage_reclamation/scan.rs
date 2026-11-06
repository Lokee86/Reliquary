use super::*;
use crate::records::memory_prune_proof::{memory_history_record, pointer_free_frame};
use reliquary_container::{ObjectRef, decode_publication};
use reliquary_memory_core::MemoryBodyId;
use std::io::Read;

pub(super) fn open_state(
    source: &Container,
    admission: &GraphAdmission,
) -> Result<(CommonState, Vec<ArchiveVectorInfo>, BTreeSet<[u8; 32]>), String> {
    match source.identity().file_kind {
        FileKind::Phylactery => PhySession::open(source, admission, [])
            .map(|session| (session.common, Vec::new(), BTreeSet::new())),
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
            (session.common, session.archive_vectors.infos(), files)
        }),
    }
    .map_err(|e| e.to_string())
}

pub(super) fn removable_backing(
    bytes: &[u8],
    memories: &MemoryStore,
    packed: &BTreeSet<[u8; 32]>,
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
        b"RELFILE1" => {
            let id: [u8; 32] = bytes
                .get(8..40)
                .ok_or("truncated Archive file identity")?
                .try_into()
                .unwrap();
            Ok(!archive_files.contains(&id))
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
        b"RELAVEC1" | b"RELMVEC1" | b"RELCPRO1" | b"RELCPRT1" | b"RELCPRP1" | b"RELRECL1"
        | b"RELFRS02" | b"RELVGEN1" | b"RELVGRC1" => Ok(false),
        _ if pointer_free_frame(magic) => Ok(false),
        _ => Err(format!(
            "unsupported reclamation frame family {}",
            String::from_utf8_lossy(magic)
        )),
    }
}

pub(super) fn reject_unactivated_generations(source: &Container) -> Result<(), String> {
    let mut generations = BTreeSet::new();
    let mut activated = BTreeSet::new();
    for_each_frame(source, |object, bytes| {
        if bytes.starts_with(b"RELVGEN1") {
            generations.insert(object.offset);
        }
        if let Some(target) = generation_receipt_target(bytes).map_err(|e| e.to_string())? {
            activated.insert(target.offset);
        }
        Ok(())
    })?;
    if generations.iter().any(|offset| !activated.contains(offset)) {
        return Err("unactivated vector generation requires recovery before reclamation".into());
    }
    Ok(())
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
