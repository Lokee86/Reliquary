use reliquary_container::{Container, decode_publication};
use reliquary_memory_core::{MemoryId, decode_grouped_creation, history_change_in_frame};
use std::{collections::BTreeMap, io::Read};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryPublicationOrigin {
    pub global_version: u64,
    pub transaction_time_ns: Option<i64>,
    pub bound_reservation: bool,
}

/// Resolve versioned standalone records against the same held file that the
/// normal current owner opened. Grouped origin coverage is separate.
pub(super) fn memory_origins(
    container: &Container,
    memory_id: MemoryId,
    cut: u64,
) -> Result<BTreeMap<u64, MemoryPublicationOrigin>, String> {
    let mut origins = BTreeMap::new();
    let mut cursor = container
        .physical_cursor()
        .map_err(|_| "memory_origin_scan_failed")?;
    while let Some(mut frame) = cursor
        .next_frame()
        .map_err(|_| "memory_origin_scan_failed")?
    {
        if frame.object().length < 8 {
            continue;
        }
        let mut magic = [0; 8];
        frame
            .read_exact(&mut magic)
            .map_err(|_| "memory_origin_frame_read_failed")?;
        if !magic.starts_with(b"RELPUB") {
            continue;
        }
        let mut bytes = magic.to_vec();
        frame
            .read_to_end(&mut bytes)
            .map_err(|_| "memory_origin_frame_read_failed")?;
        let publication = decode_publication(&bytes)
            .map_err(|_| "memory_origin_envelope_invalid")?
            .ok_or("memory_origin_envelope_missing")?;
        let change = if publication.body.starts_with(b"RELMCRT1") {
            let record = decode_grouped_creation(publication.body)
                .map_err(|_| "memory_origin_creation_invalid")?;
            Some((record.id, record.memory_version))
        } else {
            history_change_in_frame(publication.body).map_err(|_| "memory_origin_change_invalid")?
        };
        let Some((_, local_version)) =
            change.filter(|(id, version)| *id == memory_id && *version <= cut)
        else {
            continue;
        };
        let origin = MemoryPublicationOrigin {
            global_version: publication.version,
            transaction_time_ns: container.transaction_time_ns(publication.version),
            bound_reservation: publication.bound_reservation,
        };
        if origins.insert(local_version, origin).is_some() {
            return Err("memory_origin_duplicate_local_version".into());
        }
    }
    Ok(origins)
}
