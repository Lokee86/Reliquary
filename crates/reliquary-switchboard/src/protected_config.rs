//! Read-only projection of the existing CVCFG 1.0 provider configuration.

mod credentials;
mod routes;

use super::runtime_config::{RuntimeConfig, invalid};
use super::{ConfiguredRuntimeError, CredentialsConfig};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use zeroize::Zeroizing;

const MAX_CONFIG_BYTES: usize = 32 * 1024 * 1024;
const MAX_OBJECTS: u32 = 4096;
const MAX_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

pub(super) fn load(
    path: &Path,
    key_path: &Path,
) -> Result<(RuntimeConfig, CredentialsConfig), ConfiguredRuntimeError> {
    let bytes = read_bounded(path, MAX_CONFIG_BYTES)
        .map_err(|error| ConfiguredRuntimeError::Io(error.kind()))?;
    let objects = decode_objects(&bytes)
        .ok_or_else(|| invalid("protected configuration", "invalid CVCFG 1.0 object table"))?;
    let config = routes::decode(&objects)?;
    let mut master_key = None;
    let credentials = config.credentials(|id| {
        let name = format!("credential.{id}");
        let object = objects
            .get(name.as_str())
            .ok_or("protected credential is missing")?;
        let key = match &master_key {
            Some(key) => key,
            None => master_key.insert(credentials::load_master_key(key_path)?),
        };
        credentials::decode(&name, object, key)
    })?;
    Ok((config, credentials))
}

struct Object<'a> {
    schema: u16,
    flags: u32,
    payload: &'a [u8],
}

type Objects<'a> = BTreeMap<&'a str, Object<'a>>;

fn decode_objects(mut bytes: &[u8]) -> Option<Objects<'_>> {
    let header = take_array::<16>(&mut bytes)?;
    if &header[..12] != b"CVCFG\0\r\n\x01\x00\x00\x00" {
        return None;
    }
    let count = u32::from_le_bytes(header[12..16].try_into().unwrap());
    if count > MAX_OBJECTS {
        return None;
    }

    let mut objects = BTreeMap::new();
    for _ in 0..count {
        let header = take_array::<16>(&mut bytes)?;
        let key_len = u16::from_le_bytes(header[..2].try_into().unwrap()) as usize;
        let schema = u16::from_le_bytes(header[2..4].try_into().unwrap());
        let flags = u32::from_le_bytes(header[4..8].try_into().unwrap());
        let payload_len = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        if header[12..] != [0; 4] || payload_len > MAX_PAYLOAD_BYTES {
            return None;
        }
        let name = std::str::from_utf8(take(&mut bytes, key_len)?).ok()?;
        let payload = take(&mut bytes, payload_len)?;
        if objects
            .insert(
                name,
                Object {
                    schema,
                    flags,
                    payload,
                },
            )
            .is_some()
        {
            return None;
        }
    }
    bytes.is_empty().then_some(objects)
}

fn read_bounded(path: &Path, limit: usize) -> io::Result<Zeroizing<Vec<u8>>> {
    let mut bytes = Zeroizing::new(Vec::new());
    File::open(path)?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(bytes)
}

fn take<'a>(bytes: &mut &'a [u8], len: usize) -> Option<&'a [u8]> {
    let (head, rest) = bytes.split_at_checked(len)?;
    *bytes = rest;
    Some(head)
}

fn take_array<'a, const N: usize>(bytes: &mut &'a [u8]) -> Option<&'a [u8; N]> {
    let (head, rest) = bytes.split_first_chunk()?;
    *bytes = rest;
    Some(head)
}

fn read_string<'a>(bytes: &mut &'a [u8]) -> Option<&'a str> {
    let len = u32::from_le_bytes(*take_array(bytes)?) as usize;
    std::str::from_utf8(take(bytes, len)?).ok()
}

#[cfg(test)]
mod tests;
