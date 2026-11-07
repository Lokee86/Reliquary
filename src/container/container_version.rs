use crate::container::publication_envelope::{self, PublicationClock, PublicationEnvelope};
use crate::{Container, ContainerError, ObjectRef};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION_MAGIC_V1: [u8; 8] = *b"CVAVERS1";
const VERSION_MAGIC_V2: [u8; 8] = *b"CVAVERS2";
const VERSION_RECORD_V1_LEN: usize = 16;
const VERSION_RECORD_V2_LEN: usize = 24;

impl Container {
    /// Append one physical allocation envelope. The caller owns body admission
    /// and sync; this method assigns only global order and transaction time.
    pub(crate) fn append_publication_envelope(
        &mut self,
        body: &[u8],
    ) -> Result<(u64, ObjectRef), ContainerError> {
        self.require_write_admission()?;
        let global_version = self.next_version;
        let next_version = global_version
            .checked_add(1)
            .ok_or(ContainerError::VersionExhausted)?;
        let transaction_time_ns = match self.next_transaction_time_override {
            Some(time) => time,
            None => Some(Self::transaction_time_now_ns()?),
        };
        let payload = publication_envelope::encode(PublicationEnvelope {
            global_version,
            clock: PublicationClock::Allocate {
                transaction_time_ns,
            },
            body,
        })?;
        let object = self.append(&payload)?;
        self.next_version = next_version;
        self.next_transaction_time_override = None;
        if let Some(time) = transaction_time_ns {
            self.observe_transaction_time(global_version, time)?;
        }
        Ok((global_version, object))
    }

    pub(crate) fn allocate_version(&mut self) -> Result<u64, ContainerError> {
        self.require_write_admission()?;
        let transaction_time_ns = match self.next_transaction_time_override.take() {
            Some(value) => value,
            None => Some(Self::transaction_time_now_ns()?),
        };
        self.allocate_version_with_transaction_time(transaction_time_ns)
    }

    pub(crate) fn transaction_time_now_ns() -> Result<i64, ContainerError> {
        current_time_ns()
    }

    pub(crate) fn allocate_version_with_transaction_time(
        &mut self,
        transaction_time_ns: Option<i64>,
    ) -> Result<u64, ContainerError> {
        self.require_write_admission()?;
        let version = self.next_version;
        let next = version
            .checked_add(1)
            .ok_or(ContainerError::VersionExhausted)?;
        self.append(&encode_version(version, transaction_time_ns))?;
        self.next_version = next;
        if let Some(transaction_time_ns) = transaction_time_ns {
            self.observe_transaction_time(version, transaction_time_ns)?;
        }
        Ok(version)
    }

    pub(crate) fn set_next_transaction_time_override(&mut self, transaction_time_ns: Option<i64>) {
        if self.require_write_admission().is_err() {
            return;
        }
        self.next_transaction_time_override = Some(transaction_time_ns);
    }

    pub(crate) fn clear_next_transaction_time_override(&mut self) {
        if self.require_write_admission().is_err() {
            return;
        }
        self.next_transaction_time_override = None;
    }

    pub(crate) fn next_version_candidate(&self) -> u64 {
        self.next_version
    }

    pub(crate) fn commit_embedded_version_range_at(
        &mut self,
        start: u64,
        count: usize,
        transaction_time_ns: Option<i64>,
    ) -> Result<(), ContainerError> {
        self.require_write_admission()?;
        let count = u64::try_from(count).map_err(|_| ContainerError::VersionExhausted)?;
        self.observe_version_range(start, count)?;
        if let Some(transaction_time_ns) = transaction_time_ns {
            self.observe_transaction_time_range(start, count, transaction_time_ns)?;
        }
        Ok(())
    }

    pub fn latest_version(&self) -> u64 {
        self.next_version.saturating_sub(1)
    }

    pub fn transaction_time_ns(&self, version: u64) -> Option<i64> {
        self.transaction_times.get(&version).copied()
    }

    pub fn version_at_or_before(&self, transaction_time_ns: i64) -> Option<u64> {
        let mut expected_version = 1_u64;
        let mut cut = None;
        let mut prefix_max_time = i64::MIN;

        for (&version, &timestamp) in &self.transaction_times {
            if version != expected_version {
                break;
            }
            prefix_max_time = prefix_max_time.max(timestamp);
            if prefix_max_time > transaction_time_ns {
                break;
            }
            cut = Some(version);
            expected_version = expected_version.checked_add(1)?;
        }
        cut
    }

    fn observe_version_range(&mut self, start: u64, count: u64) -> Result<(), ContainerError> {
        observe_version_range(&mut self.next_version, start, count)
    }

    fn observe_transaction_time(
        &mut self,
        version: u64,
        transaction_time_ns: i64,
    ) -> Result<(), ContainerError> {
        observe_transaction_time(
            self.next_version,
            &mut self.transaction_times,
            version,
            transaction_time_ns,
        )
    }

    fn observe_transaction_time_range(
        &mut self,
        start: u64,
        count: u64,
        transaction_time_ns: i64,
    ) -> Result<(), ContainerError> {
        observe_transaction_time_range(
            self.next_version,
            &mut self.transaction_times,
            start,
            count,
            transaction_time_ns,
            None,
        )
    }
}

pub(crate) fn observe_version_payload_state(
    next_version: &mut u64,
    transaction_times: &mut BTreeMap<u64, i64>,
    standalone_reservations: &mut BTreeSet<u64>,
    payload: &[u8],
    max_state_entries: Option<usize>,
) -> Result<(), ContainerError> {
    if let Some((version, transaction_time_ns)) = decode_version(payload)? {
        observe_version_range(next_version, version, 1)?;
        if let Some(transaction_time_ns) = transaction_time_ns {
            ensure_bounded_insert(
                transaction_times.len(),
                transaction_times.contains_key(&version),
                max_state_entries,
            )?;
            observe_transaction_time(
                *next_version,
                transaction_times,
                version,
                transaction_time_ns,
            )?;
        }
        // Only standalone tickets authorize a later physical envelope binding.
        // Existing semantic links are still validated by their concrete owners.
        ensure_bounded_insert(
            standalone_reservations.len(),
            standalone_reservations.contains(&version),
            max_state_entries,
        )?;
        standalone_reservations.insert(version);
        return Ok(());
    }
    if let Some(envelope) = publication_envelope::decode(payload)? {
        match envelope.clock {
            PublicationClock::Allocate {
                transaction_time_ns,
            } => {
                observe_version_range(next_version, envelope.global_version, 1)?;
                if let Some(time) = transaction_time_ns {
                    ensure_bounded_insert(
                        transaction_times.len(),
                        transaction_times.contains_key(&envelope.global_version),
                        max_state_entries,
                    )?;
                    observe_transaction_time(
                        *next_version,
                        transaction_times,
                        envelope.global_version,
                        time,
                    )?;
                }
            }
            PublicationClock::BindReserved => {
                if !standalone_reservations.remove(&envelope.global_version) {
                    return Err(ContainerError::InvalidVersionRecord);
                }
            }
        }
        return Ok(());
    }
    if let Some((start, count, transaction_time_ns)) =
        crate::container::grouped_clock::insomnia_range(payload)
            .map_err(|_| ContainerError::InvalidVersionRecord)?
    {
        let count = u64::from(count);
        observe_version_range(next_version, start, count)?;
        if let Some(transaction_time_ns) = transaction_time_ns {
            observe_transaction_time_range(
                *next_version,
                transaction_times,
                start,
                count,
                transaction_time_ns,
                max_state_entries,
            )?;
        }
    }
    if let Some((start, count, transaction_ns)) =
        crate::container::grouped_clock::external_range(payload)
            .map_err(|_| ContainerError::InvalidVersionRecord)?
    {
        observe_version_range(next_version, start, u64::from(count))?;
        observe_transaction_time_range(
            *next_version,
            transaction_times,
            start,
            u64::from(count),
            transaction_ns,
            max_state_entries,
        )?;
    }
    Ok(())
}

fn observe_version_range(
    next_version: &mut u64,
    start: u64,
    count: u64,
) -> Result<(), ContainerError> {
    if count == 0 {
        return Ok(());
    }
    if start != *next_version {
        return Err(ContainerError::InvalidVersionRecord);
    }
    *next_version = start
        .checked_add(count)
        .ok_or(ContainerError::VersionExhausted)?;
    Ok(())
}

fn observe_transaction_time(
    next_version: u64,
    transaction_times: &mut BTreeMap<u64, i64>,
    version: u64,
    transaction_time_ns: i64,
) -> Result<(), ContainerError> {
    if version == 0 || version >= next_version {
        return Err(ContainerError::InvalidVersionRecord);
    }
    if let Some(existing) = transaction_times.insert(version, transaction_time_ns)
        && existing != transaction_time_ns
    {
        return Err(ContainerError::InvalidVersionRecord);
    }
    Ok(())
}

fn ensure_bounded_insert(
    current_entries: usize,
    already_present: bool,
    limit: Option<usize>,
) -> Result<(), ContainerError> {
    if let Some(limit) = limit {
        let required = if already_present {
            current_entries
        } else {
            current_entries.saturating_add(1)
        };
        if required > limit {
            return Err(ContainerError::ScanResourceLimitExceeded);
        }
    }
    Ok(())
}

fn observe_transaction_time_range(
    next_version: u64,
    transaction_times: &mut BTreeMap<u64, i64>,
    start: u64,
    count: u64,
    transaction_time_ns: i64,
    max_state_entries: Option<usize>,
) -> Result<(), ContainerError> {
    if let Some(limit) = max_state_entries {
        let additional = usize::try_from(count).unwrap_or(usize::MAX);
        if transaction_times.len().saturating_add(additional) > limit {
            return Err(ContainerError::ScanResourceLimitExceeded);
        }
    }
    for offset in 0..count {
        let version = start
            .checked_add(offset)
            .ok_or(ContainerError::VersionExhausted)?;
        observe_transaction_time(
            next_version,
            transaction_times,
            version,
            transaction_time_ns,
        )?;
    }
    Ok(())
}

fn encode_version(version: u64, transaction_time_ns: Option<i64>) -> Vec<u8> {
    match transaction_time_ns {
        Some(transaction_time_ns) => {
            let mut out = vec![0_u8; VERSION_RECORD_V2_LEN];
            out[..8].copy_from_slice(&VERSION_MAGIC_V2);
            out[8..16].copy_from_slice(&version.to_le_bytes());
            out[16..24].copy_from_slice(&transaction_time_ns.to_le_bytes());
            out
        }
        None => {
            let mut out = vec![0_u8; VERSION_RECORD_V1_LEN];
            out[..8].copy_from_slice(&VERSION_MAGIC_V1);
            out[8..16].copy_from_slice(&version.to_le_bytes());
            out
        }
    }
}

fn decode_version(payload: &[u8]) -> Result<Option<(u64, Option<i64>)>, ContainerError> {
    if payload.len() < 8 {
        return Ok(None);
    }
    if payload[..8] == VERSION_MAGIC_V1 {
        if payload.len() != VERSION_RECORD_V1_LEN {
            return Err(ContainerError::InvalidVersionRecord);
        }
        return Ok(Some((
            u64::from_le_bytes(payload[8..16].try_into().unwrap()),
            None,
        )));
    }
    if payload[..8] == VERSION_MAGIC_V2 {
        if payload.len() != VERSION_RECORD_V2_LEN {
            return Err(ContainerError::InvalidVersionRecord);
        }
        return Ok(Some((
            u64::from_le_bytes(payload[8..16].try_into().unwrap()),
            Some(i64::from_le_bytes(payload[16..24].try_into().unwrap())),
        )));
    }
    Ok(None)
}

fn current_time_ns() -> Result<i64, ContainerError> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ContainerError::InvalidTransactionTime)?
        .as_nanos();
    i64::try_from(nanos).map_err(|_| ContainerError::InvalidTransactionTime)
}

#[cfg(test)]
#[path = "publication_envelope_clock_tests.rs"]
mod envelope_clock_tests;
