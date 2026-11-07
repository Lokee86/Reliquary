use super::{CurrentOwnerSession, ReadOnlyCurrentOwnerObserver};
use reliquary_archive::ArchivePublicationBinding;
use reliquary_container::{Container, ObjectRef, decode_publication};
use std::{collections::BTreeMap, io::Read};

const ARCHIVE_BINDING_MARKER: &[u8; 8] = b"RELAPB01";
const ARCHIVE_BINDING_PREFIX: &[u8; 6] = b"RELAPB";
const ENVELOPE_HEADER_MAX: usize = 25;
const ARCHIVE_BINDING_HEADER: usize = 17;
const REPEAT_BODY_LENGTH: usize = ARCHIVE_BINDING_HEADER + 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PhysicalArchiveBinding {
    object: ObjectRef,
    archive_version: u64,
    global_version: u64,
    transaction_time_ns: Option<i64>,
    bound_reservation: bool,
    first_applying_archive_version: u64,
}

impl ReadOnlyCurrentOwnerObserver {
    /// Validate the accepted Archive binding prefix against the physical
    /// publication envelopes under the same lease as current owner replay.
    pub fn archive_publication_bindings_at_cut(
        &self,
        cut: u64,
    ) -> Result<&[ArchivePublicationBinding], String> {
        let CurrentOwnerSession::Rel(session) = &self.session else {
            return Err("archive_observation_requires_rel".into());
        };
        if cut > session.archive_version() {
            return Err("archive_cut_unavailable".into());
        }
        let cut_count = usize::try_from(cut).map_err(|_| "archive_binding_cut_exceeds_platform")?;
        let native = session
            .archive_publication_bindings()
            .get(..cut_count)
            .ok_or("archive_binding_cut_incomplete")?;
        let physical = scan_bindings(&self.container, cut)?;
        reconcile_bindings(native, &physical)?;
        Ok(native)
    }
}

fn scan_bindings(container: &Container, cut: u64) -> Result<Vec<PhysicalArchiveBinding>, String> {
    let mut result = Vec::new();
    let mut reservations = BTreeMap::new();
    let mut cursor = container
        .physical_cursor()
        .map_err(|_| "archive_binding_scan_failed")?;
    while let Some(mut frame) = cursor
        .next_frame()
        .map_err(|_| "archive_binding_scan_failed")?
    {
        let object = frame.object();
        // The envelope and wrapper have bounded fixed headers. Current
        // Container open validates the complete framing and version clock.
        let mut header = [0; ENVELOPE_HEADER_MAX + REPEAT_BODY_LENGTH];
        let count = object.length.min(header.len() as u64) as usize;
        let prefix = &mut header[..count];
        frame
            .read_exact(prefix)
            .map_err(|_| "archive_binding_header_read_failed")?;
        if let Some((version, time)) = reservation(prefix)? {
            if reservations.insert(version, time).is_some() {
                return Err("archive_binding_reservation_reused".into());
            }
            continue;
        }
        let Some(binding) = parse_binding(container, object, prefix, &mut reservations)? else {
            continue;
        };
        if binding.archive_version <= cut {
            result.push(binding);
        }
    }
    Ok(result)
}

fn reservation(prefix: &[u8]) -> Result<Option<(u64, Option<i64>)>, String> {
    if !prefix.starts_with(b"RELVERS") {
        return Ok(None);
    }
    let time = match prefix.get(..8) {
        Some(b"RELVERS1") if prefix.len() == 16 => None,
        Some(b"RELVERS2") if prefix.len() == 24 => {
            Some(i64::from_le_bytes(prefix[16..24].try_into().unwrap()))
        }
        _ => return Err("archive_binding_reservation_shape_invalid".into()),
    };
    let version = u64::from_le_bytes(prefix[8..16].try_into().unwrap());
    if version == 0 {
        return Err("archive_binding_reservation_version_zero".into());
    }
    Ok(Some((version, time)))
}

fn parse_binding(
    container: &Container,
    object: ObjectRef,
    prefix: &[u8],
    reservations: &mut BTreeMap<u64, Option<i64>>,
) -> Result<Option<PhysicalArchiveBinding>, String> {
    if prefix.starts_with(ARCHIVE_BINDING_PREFIX) {
        return Err("archive_binding_missing_envelope".into());
    }
    let Some(envelope) =
        decode_publication(prefix).map_err(|_| "archive_binding_envelope_invalid")?
    else {
        return Ok(None);
    };
    let physical_time = if envelope.bound_reservation {
        reservations
            .remove(&envelope.version)
            .ok_or("archive_binding_prior_reservation_missing")?
    } else {
        envelope.transaction_time_ns
    };
    if !envelope.body.starts_with(ARCHIVE_BINDING_PREFIX) {
        return Ok(None);
    }
    let body = envelope.body;
    if !body.starts_with(ARCHIVE_BINDING_MARKER) || body.len() < ARCHIVE_BINDING_HEADER {
        return Err("archive_binding_wrapper_invalid".into());
    }
    let archive_version = u64::from_le_bytes(body[8..16].try_into().unwrap());
    let body_length = object.length - (prefix.len() - body.len()) as u64;
    let first_applying_archive_version = match body[16] {
        0 if body_length >= (ARCHIVE_BINDING_HEADER + 8) as u64 => archive_version,
        1 if body_length == REPEAT_BODY_LENGTH as u64 && body.len() == REPEAT_BODY_LENGTH => {
            let first = u64::from_le_bytes(body[17..25].try_into().unwrap());
            if first == 0 || first >= archive_version {
                return Err("archive_binding_repeat_origin_invalid".into());
            }
            first
        }
        _ => return Err("archive_binding_wrapper_shape_invalid".into()),
    };
    if archive_version == 0 {
        return Err("archive_binding_local_version_zero".into());
    }
    if container.transaction_time_ns(envelope.version) != physical_time {
        return Err("archive_binding_resolved_time_mismatch".into());
    }
    Ok(Some(PhysicalArchiveBinding {
        object,
        archive_version,
        global_version: envelope.version,
        transaction_time_ns: physical_time,
        bound_reservation: envelope.bound_reservation,
        first_applying_archive_version,
    }))
}

fn reconcile_bindings(
    native: &[ArchivePublicationBinding],
    physical: &[PhysicalArchiveBinding],
) -> Result<(), String> {
    if native.len() != physical.len() {
        return Err("archive_binding_physical_count_mismatch".into());
    }
    for (index, (native, physical_row)) in native.iter().zip(physical).enumerate() {
        if physical_row.archive_version != index as u64 + 1 {
            return Err("archive_binding_local_order_invalid".into());
        }
        let first = physical_row
            .first_applying_archive_version
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok())
            .filter(|first_index| *first_index <= index)
            .and_then(|first_index| physical.get(first_index))
            .ok_or("archive_binding_repeat_origin_invalid")?;
        if first.first_applying_archive_version != first.archive_version {
            return Err("archive_binding_repeat_origin_does_not_apply".into());
        }
        if native.object != physical_row.object
            || native.archive_version != physical_row.archive_version
            || native.global_version != physical_row.global_version
            || native.transaction_time_ns != physical_row.transaction_time_ns
            || native.bound_reservation != physical_row.bound_reservation
            || native.first_applying_archive_version != first.archive_version
            || native.first_applying_global_version != first.global_version
        {
            return Err("archive_binding_native_physical_mismatch".into());
        }
    }
    Ok(())
}
