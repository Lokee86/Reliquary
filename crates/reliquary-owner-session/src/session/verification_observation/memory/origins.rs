use crate::records::insomnia_completion::GroupedInsomniaCompletion;
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
    let origins = scan_memory_origins(container, false, |id, version| {
        id == memory_id && version <= cut
    })?;
    Ok(origins
        .into_iter()
        .map(|(version, (_, origin))| (version, origin))
        .collect())
}

pub(super) fn all_memory_origins(
    container: &Container,
) -> Result<BTreeMap<u64, (MemoryId, MemoryPublicationOrigin)>, String> {
    scan_memory_origins(container, true, |_, _| true)
}

fn scan_memory_origins(
    container: &Container,
    include_grouped_insomnia: bool,
    include: impl Fn(MemoryId, u64) -> bool,
) -> Result<BTreeMap<u64, (MemoryId, MemoryPublicationOrigin)>, String> {
    let mut origins = BTreeMap::new();
    let mut add = |id, local_version, global_version, bound_reservation| {
        if include(id, local_version) {
            let origin = MemoryPublicationOrigin {
                global_version,
                transaction_time_ns: container.transaction_time_ns(global_version),
                bound_reservation,
            };
            if origins.insert(local_version, (id, origin)).is_some() {
                return Err("memory_origin_duplicate_local_version");
            }
        }
        Ok(())
    };
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
        let grouped = include_grouped_insomnia && &magic == b"RELINSC7";
        if !magic.starts_with(b"RELPUB") && !grouped {
            continue;
        }
        let mut bytes = magic.to_vec();
        frame
            .read_to_end(&mut bytes)
            .map_err(|_| "memory_origin_frame_read_failed")?;
        if grouped {
            let completion = GroupedInsomniaCompletion::decode(&bytes)
                .map_err(|_| "memory_origin_grouped_insomnia_invalid")?;
            for (index, creation) in completion.memory_records.iter().enumerate() {
                let record = decode_grouped_creation(creation)
                    .map_err(|_| "memory_origin_creation_invalid")?;
                let global = completion
                    .global_version_start
                    .checked_add(index as u64)
                    .ok_or("memory_origin_grouped_version_overflow")?;
                add(record.id, record.memory_version, global, false)?;
            }
            continue;
        }
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
        if let Some((id, local_version)) = change {
            add(
                id,
                local_version,
                publication.version,
                publication.bound_reservation,
            )?;
        }
    }
    Ok(origins)
}
