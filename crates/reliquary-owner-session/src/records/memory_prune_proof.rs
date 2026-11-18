//! Physical, retained-history proof for terminal Memory body pruning.
//! Digests preserve all source frames while canonicalizing only selected bodies.
use crate::records::GroupedInsomniaCompletion;
use reliquary_container::{Container, FileKind, ReliquaryScope, decode_publication};
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
        if &prefix == b"RELINSC7" {
            let mut payload = prefix.to_vec();
            frame.read_to_end(&mut payload).map_err(|e| e.to_string())?;
            let (canonical, _) = grouped_completion_without_bodies(&payload, pruned)?;
            hash.update((canonical.len() as u64).to_le_bytes());
            hash.update(canonical);
            continue;
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
        } else if &prefix == b"RELVGRC1" {
            // A copy transform relocates this receipt's physical ObjectRef.
            // Hash the same receipt with only its offset canonicalized; the
            // generation frame and receipt length remain bound by the digest,
            // and owner replay validates the relocated target after copying.
            let mut receipt = prefix.to_vec();
            frame.read_to_end(&mut receipt).map_err(|e| e.to_string())?;
            let target = reliquary_vector_store::generation::generation_receipt_target(&receipt)
                .map_err(|e| e.to_string())?
                .ok_or("missing vector generation receipt target")?;
            let canonical = reliquary_vector_store::generation::relocate_generation_receipt(
                &receipt,
                reliquary_container::ObjectRef {
                    offset: 0,
                    length: target.length,
                },
            )
            .map_err(|e| e.to_string())?;
            hash.update(len.to_le_bytes());
            hash.update(canonical);
            continue;
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
        if len < 8 {
            continue;
        }
        let mut prefix = [0u8; 8];
        frame.read_exact(&mut prefix).map_err(|e| e.to_string())?;
        if &prefix == MANIFEST {
            break;
        }
        if &prefix == b"RELPUB01" {
            let mut payload = prefix.to_vec();
            frame.read_to_end(&mut payload).map_err(|e| e.to_string())?;
            let envelope = decode_publication(&payload)
                .map_err(|e| e.to_string())?
                .ok_or("missing publication envelope")?;
            if memory_history_record(envelope.body)
                && envelope.body.get(8..40) == Some(memory_id.0.as_slice())
            {
                count += 1;
                hash.update(len.to_le_bytes());
                hash.update(payload);
            }
            continue;
        }
        if &prefix == b"RELINSC7" {
            let mut payload = prefix.to_vec();
            frame.read_to_end(&mut payload).map_err(|e| e.to_string())?;
            let completion = GroupedInsomniaCompletion::decode(&payload)?;
            for record in completion.memory_records {
                let decoded = reliquary_memory_core::decode_grouped_creation(&record)
                    .map_err(|e| e.to_string())?;
                if decoded.id == memory_id {
                    hash_memory_record(&mut hash, &mut count, &record);
                }
            }
            continue;
        }
        if &prefix == b"RELDRB01" {
            let mut payload = prefix.to_vec();
            frame.read_to_end(&mut payload).map_err(|e| e.to_string())?;
            let batch = crate::records::dream_batch::DreamBatchRecord::decode(&payload)?
                .ok_or("invalid Dream batch")?;
            for lifecycle in batch.lifecycles {
                if history_record_targets(&lifecycle.delta, memory_id)? {
                    hash_memory_record(&mut hash, &mut count, &lifecycle.delta);
                }
            }
            continue;
        }
        if &prefix == b"RELDRF02" {
            let mut payload = prefix.to_vec();
            frame.read_to_end(&mut payload).map_err(|e| e.to_string())?;
            let settlement = crate::records::dream_empty::InitialDreamSettlement::decode(&payload)?
                .ok_or("invalid initial Dream settlement")?;
            if let Some(delta) = settlement.memory_delta
                && history_record_targets(&delta, memory_id)?
            {
                hash_memory_record(&mut hash, &mut count, &delta);
            }
            continue;
        }
        if &prefix == b"RELDRS01" {
            let mut payload = prefix.to_vec();
            frame.read_to_end(&mut payload).map_err(|e| e.to_string())?;
            let supersession =
                crate::records::dream_supersession::GroupedDreamSupersession::decode(&payload)?
                    .ok_or("invalid Dream supersession")?;
            if history_record_targets(&supersession.delta, memory_id)? {
                hash_memory_record(&mut hash, &mut count, &supersession.delta);
            }
            continue;
        }
        if matches!(&prefix, b"RELMCOR1" | b"RELMRET1") {
            let mut payload = prefix.to_vec();
            frame.read_to_end(&mut payload).map_err(|e| e.to_string())?;
            let mutation = crate::records::user_memory::UserMemoryRecord::decode(&payload)?
                .ok_or("invalid user Memory record")?;
            for effect in mutation.history_frames() {
                if effect.get(8..40) == Some(memory_id.0.as_slice()) {
                    hash_memory_record(&mut hash, &mut count, effect);
                }
            }
            continue;
        }
        if !memory_history_record(&prefix) {
            continue;
        }
        let mut id = [0u8; 32];
        frame.read_exact(&mut id).map_err(|e| e.to_string())?;
        if id != memory_id.0 {
            continue;
        }
        hash.update(len.to_le_bytes());
        hash.update(prefix);
        hash.update(id);
        count += 1;
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

pub(crate) fn grouped_completion_without_bodies(
    payload: &[u8],
    selected: &BTreeSet<MemoryBodyId>,
) -> Result<(Vec<u8>, BTreeSet<MemoryBodyId>), String> {
    let mut completion = GroupedInsomniaCompletion::decode(payload)?;
    let mut removed = BTreeSet::new();
    completion.bodies.retain(|body| {
        if selected.contains(&body.id) {
            removed.insert(body.id);
            false
        } else {
            true
        }
    });
    if removed.is_empty() {
        return Ok((payload.to_vec(), removed));
    }
    Ok((completion.encode()?, removed))
}

fn history_record_targets(bytes: &[u8], memory_id: MemoryId) -> Result<bool, String> {
    Ok(reliquary_memory_core::history_change_in_frame(bytes)
        .map_err(|e| e.to_string())?
        .is_some_and(|(id, _)| id == memory_id))
}

fn hash_memory_record(hash: &mut Sha256, count: &mut u64, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    *count += 1;
}

pub(crate) fn memory_history_record(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(..8),
        Some(b"RELMCRT1" | b"RELMDLT1" | b"RELMTMB1")
    )
}

/// Restrict this transform to frames with no offset-bearing cross-domain
/// payloads. More families require their own relocation and proof contracts.
pub(crate) fn safe_memory_only_frames(
    container: &Container,
    has_manifest: bool,
) -> Result<u64, String> {
    let mut version = 0;
    let mut cursor = container.physical_cursor().map_err(|e| e.to_string())?;
    while let Some(mut frame) = cursor.next_frame().map_err(|e| e.to_string())? {
        let mut magic = [0u8; 8];
        frame.read_exact(&mut magic).map_err(|e| e.to_string())?;
        if has_manifest && &magic == MANIFEST {
            return Ok(version);
        }
        if &magic == b"RELINSC7" {
            let mut payload = magic.to_vec();
            frame.read_to_end(&mut payload).map_err(|e| e.to_string())?;
            let completion = GroupedInsomniaCompletion::decode(&payload)?;
            if !completion.memory_records.is_empty() {
                let last = completion
                    .global_version_start
                    .checked_add(completion.memory_records.len() as u64 - 1)
                    .ok_or("grouped publication range overflow")?;
                version = version.max(last);
            }
            continue;
        }
        if &magic == b"RELPUB01" {
            let mut payload = magic.to_vec();
            frame.read_to_end(&mut payload).map_err(|e| e.to_string())?;
            let envelope = decode_publication(&payload)
                .map_err(|e| e.to_string())?
                .ok_or("missing publication envelope")?;
            if !memory_history_record(envelope.body) {
                return Err("transform needs an owner proof for publication body".into());
            }
            version = version.max(envelope.version);
            continue;
        }
        if &magic == b"RELVGRC1" {
            let mut receipt = magic.to_vec();
            frame.read_to_end(&mut receipt).map_err(|e| e.to_string())?;
            reliquary_vector_store::generation::generation_receipt_target(&receipt)
                .map_err(|e| e.to_string())?
                .ok_or("missing vector generation receipt target")?;
            // This exact pointer-bearing family is accepted only because
            // memory pruning relocates it through the generation codec.
            continue;
        }
        if matches!(&magic, b"RELVERS1" | b"RELVERS2") {
            let mut raw = [0; 8];
            frame.read_exact(&mut raw).map_err(|e| e.to_string())?;
            version = version.max(u64::from_le_bytes(raw));
        }
        if !pointer_free_frame(&magic) {
            return Err(format!(
                "transform needs an owner proof for frame family {}",
                String::from_utf8_lossy(&magic)
            ));
        }
    }
    if has_manifest {
        return Err("missing prune manifest".into());
    }
    Ok(version)
}

pub(crate) fn pointer_free_frame(magic: &[u8]) -> bool {
    matches!(
        magic,
        b"RELMFMT1"
            | b"RELMBOD1"
            | b"RELMCRT1"
            | b"RELMDLT1"
            | b"RELMTMB1"
            | b"RELMCOR1"
            | b"RELMRET1"
            | b"RELPVFM1"
            | b"RELMVFM1"
            | b"RELCPFM1"
            | b"RELAVFM1"
            | b"RELVGFM1"
            | b"RELFRP01"
            | b"RELTURN2"
            | b"RELFILE1"
            | b"RELBRCH1"
            | b"RELTIT01"
            | b"RELEPIS1"
            | b"CVAFRG01"
            | b"RELECHO1"
            | b"RELVERS1"
            | b"RELVERS2"
            | b"RELMVEC1"
            | b"RELAVEC1"
            | b"RELVGEN1"
            | b"RELCPRO1"
            | b"RELCPRT1"
            | b"RELCPRP1"
            | b"RELFRS02"
            | b"RELCTM01"
            | b"RELTPR01"
            | b"RELPVEC1"
            | b"RELIEPS2"
            | b"RELIEPC2"
            | b"RELIFRS1"
            | b"RELIPLN1"
            | b"CVAIMPT1"
            | b"RELINSW1"
            | b"RELINSW2"
            | b"RELINSR1"
            | b"RELINSC7"
            | b"CVDREAM1"
            | b"CVDREAM2"
            | b"RELDRM01"
            | b"RELDRP02"
            | b"RELDRG01"
            | b"RELDRS01"
            | b"RELDRF01"
            | b"RELDRF02"
            | b"RELDRB01"
            | b"RELDRC01"
            | b"RELXIDX2"
            | b"RELXMEM2"
            | b"RELXMEM3"
            | b"RELXMRP3"
            | b"RELXHST1"
            | b"RELPER01"
            | b"RELETM01"
            | b"RELRLP01"
            | b"RELGRF01"
            | b"RELASC01"
            | b"RELCOM01"
            | b"RELCMP01"
            | b"RELCMN01"
    )
}
