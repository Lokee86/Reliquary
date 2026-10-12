use crate::{Container, ContainerError};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION_MAGIC_V1: [u8; 8] = *b"CVAVERS1";
const VERSION_MAGIC_V2: [u8; 8] = *b"CVAVERS2";
const VERSION_RECORD_V1_LEN: usize = 16;
const VERSION_RECORD_V2_LEN: usize = 24;

impl Container {
    pub(crate) fn allocate_version(&mut self) -> Result<u64, ContainerError> {
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
        self.next_transaction_time_override = Some(transaction_time_ns);
    }

    pub(crate) fn clear_next_transaction_time_override(&mut self) {
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
        )
    }
}

pub(crate) fn observe_version_payload_state(
    next_version: &mut u64,
    transaction_times: &mut BTreeMap<u64, i64>,
    payload: &[u8],
) -> Result<(), ContainerError> {
    if let Some((version, transaction_time_ns)) = decode_version(payload)? {
        observe_version_range(next_version, version, 1)?;
        if let Some(transaction_time_ns) = transaction_time_ns {
            observe_transaction_time(
                *next_version,
                transaction_times,
                version,
                transaction_time_ns,
            )?;
        }
        return Ok(());
    }
    if let Some((start, count, transaction_time_ns)) =
        crate::insomnia::completion::decode_embedded_version_range(payload)
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
            )?;
        }
    }
    if let Some((start, count, transaction_ns)) =
        crate::external_source::publication::decode_embedded_version_range(payload)
            .map_err(|_| ContainerError::InvalidVersionRecord)?
    {
        observe_version_range(next_version, start, u64::from(count))?;
        observe_transaction_time_range(
            *next_version,
            transaction_times,
            start,
            u64::from(count),
            transaction_ns,
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

fn observe_transaction_time_range(
    next_version: u64,
    transaction_times: &mut BTreeMap<u64, i64>,
    start: u64,
    count: u64,
    transaction_time_ns: i64,
) -> Result<(), ContainerError> {
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
