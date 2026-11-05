//! Physical, retained-history proof for standalone terminal Memory body pruning.
//! The digest covers every surviving source frame before the manifest.
use reliquary_container::{Container, FileKind, ReliquaryScope};
use reliquary_memory_core::{MemoryBodyId, MemoryId, TerminalTombstone};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, io::Read};

pub(crate) const MEMORY_BODY: &[u8; 8] = b"RELMBOD1";
pub(crate) const MANIFEST: &[u8; 8] = b"RELHPR01";

pub(crate) fn tombstone_digest(id: MemoryId, tombstone: TerminalTombstone) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-prune-memory-tombstone-v1\0");
    hash.update(id.0);
    hash.update(tombstone.revision.to_le_bytes());
    hash.update(tombstone.memory_version.to_le_bytes());
    hash.finalize().into()
}

pub(crate) fn frame_history_digest(
    container: &Container,
    pruned: &BTreeSet<MemoryBodyId>,
) -> Result<[u8; 32], String> {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-retained-owner-frames-v1\0");
    hash.update(container.owner_id());
    hash.update([match container.identity().file_kind {
        FileKind::Reliquary => 1u8,
        FileKind::Phylactery => 2u8,
    }]);
    hash.update([match container.identity().scope {
        ReliquaryScope::Unspecified => 0u8,
        ReliquaryScope::Organization => 1u8,
        ReliquaryScope::Project => 2u8,
        ReliquaryScope::Connection => 3u8,
    }]);
    let mut cursor = container.physical_cursor().map_err(|e| e.to_string())?;
    while let Some(mut frame) = cursor.next_frame().map_err(|e| e.to_string())? {
        let len = frame.object().length;
        if len < 8 {
            return Err("unsupported short owner frame in transformed source".into());
        }
        let mut prefix = [0u8; 8];
        frame.read_exact(&mut prefix).map_err(|e| e.to_string())?;
        if &prefix == MANIFEST {
            break;
        }
        if &prefix == MEMORY_BODY && len >= 40 {
            let mut raw_id = [0u8; 32];
            frame.read_exact(&mut raw_id).map_err(|e| e.to_string())?;
            if pruned.contains(&MemoryBodyId(raw_id)) {
                continue;
            }
            hash.update(len.to_le_bytes());
            hash.update(prefix);
            hash.update(raw_id);
        } else {
            hash.update(len.to_le_bytes());
            hash.update(prefix);
        }
        let mut buffer = [0u8; 65536];
        loop {
            let count = frame.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
    }
    for id in pruned {
        hash.update(id.0);
    }
    Ok(hash.finalize().into())
}

pub(crate) fn memory_structure_digest(
    container: &Container,
    memory_id: MemoryId,
) -> Result<[u8; 32], String> {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-retained-memory-records-v1\0");
    hash.update(memory_id.0);
    let mut count = 0u64;
    let mut cursor = container.physical_cursor().map_err(|e| e.to_string())?;
    while let Some(mut frame) = cursor.next_frame().map_err(|e| e.to_string())? {
        let len = frame.object().length;
        if len < 40 {
            continue;
        }
        let mut prefix = [0u8; 8];
        frame.read_exact(&mut prefix).map_err(|e| e.to_string())?;
        if &prefix == MANIFEST {
            break;
        }
        if !matches!(&prefix, b"RELMCRT1" | b"RELMDLT1" | b"RELMTMB1") {
            continue;
        }
        let mut id = [0u8; 32];
        frame.read_exact(&mut id).map_err(|e| e.to_string())?;
        if id != memory_id.0 {
            continue;
        }
        count += 1;
        hash.update(len.to_le_bytes());
        hash.update(prefix);
        hash.update(id);
        let mut buffer = [0u8; 65536];
        loop {
            let amount = frame.read(&mut buffer).map_err(|e| e.to_string())?;
            if amount == 0 {
                break;
            }
            hash.update(&buffer[..amount]);
        }
    }
    if count < 2 {
        return Err("terminal Memory must retain creation and tombstone frames".into());
    }
    Ok(hash.finalize().into())
}

/// Restrict this transform to frames with no offset-bearing cross-domain
/// payloads. More families require their own relocation and proof contracts.
pub(crate) fn safe_memory_only_frames(
    container: &Container,
    has_manifest: bool,
) -> Result<(), String> {
    let mut cursor = container.physical_cursor().map_err(|e| e.to_string())?;
    while let Some(mut frame) = cursor.next_frame().map_err(|e| e.to_string())? {
        let mut magic = [0u8; 8];
        frame.read_exact(&mut magic).map_err(|e| e.to_string())?;
        if has_manifest && &magic == MANIFEST {
            return Ok(());
        }
        if !matches!(
            &magic,
            b"RELMFMT1"
                | b"RELMBOD1"
                | b"RELMCRT1"
                | b"RELMDLT1"
                | b"RELMTMB1"
                | b"RELPVFM1"
                | b"RELMVFM1"
                | b"RELCPFM1"
                | b"RELAVFM1"
                | b"RELVGFM1"
                | b"RELFRP01"
                | b"RELVERS1"
                | b"RELVERS2"
        ) {
            return Err(format!(
                "transform needs an owner proof for frame family {}",
                String::from_utf8_lossy(&magic)
            ));
        }
    }
    Ok(())
}
