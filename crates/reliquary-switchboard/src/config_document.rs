use crate::{ConfiguredRuntimeError, model_config::invalid};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use zeroize::Zeroizing;

const HEADER: &[u8; 12] = b"CVCFG\0\r\n\x01\x00\x00\x00";
const MAX_CONFIG_BYTES: usize = 32 * 1024 * 1024;
const MAX_OBJECTS: usize = 4096;
const MAX_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Eq, PartialEq)]
pub struct ConfigObject {
    pub schema: u16,
    pub flags: u32,
    pub payload: Vec<u8>,
}

#[derive(Clone, Default, Eq, PartialEq)]
pub(super) struct ConfigDocument(pub BTreeMap<String, ConfigObject>);

impl ConfigDocument {
    pub fn read(path: &Path) -> Result<Self, ConfiguredRuntimeError> {
        let bytes = read_bounded(path, MAX_CONFIG_BYTES)
            .map_err(|error| ConfiguredRuntimeError::Io(error.kind()))?;
        Self::decode(&bytes)
            .ok_or_else(|| invalid("configuration", "invalid CVCFG 1.0 object table"))
    }

    pub(super) fn decode(mut bytes: &[u8]) -> Option<Self> {
        if take(&mut bytes, HEADER.len())? != HEADER {
            return None;
        }
        let count = u32::from_le_bytes(*take_array(&mut bytes)?) as usize;
        if count > MAX_OBJECTS {
            return None;
        }
        let mut objects = BTreeMap::new();
        for _ in 0..count {
            let key_len = u16::from_le_bytes(*take_array(&mut bytes)?) as usize;
            let schema = u16::from_le_bytes(*take_array(&mut bytes)?);
            let flags = u32::from_le_bytes(*take_array(&mut bytes)?);
            let len = u32::from_le_bytes(*take_array(&mut bytes)?) as usize;
            if take_array::<4>(&mut bytes)? != &[0; 4] || len > MAX_PAYLOAD_BYTES {
                return None;
            }
            let key = std::str::from_utf8(take(&mut bytes, key_len)?).ok()?;
            let payload = take(&mut bytes, len)?.to_vec();
            if objects
                .insert(
                    key.to_owned(),
                    ConfigObject {
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
        bytes.is_empty().then_some(Self(objects))
    }

    fn encode(&self) -> Option<Vec<u8>> {
        if self.0.len() > MAX_OBJECTS {
            return None;
        }
        let mut bytes = HEADER.to_vec();
        bytes.extend_from_slice(&(self.0.len() as u32).to_le_bytes());
        for (name, object) in &self.0 {
            if object.payload.len() > MAX_PAYLOAD_BYTES
                || bytes.len() + 16 + name.len() + object.payload.len() > MAX_CONFIG_BYTES
            {
                return None;
            }
            bytes.extend_from_slice(&u16::try_from(name.len()).ok()?.to_le_bytes());
            bytes.extend_from_slice(&object.schema.to_le_bytes());
            bytes.extend_from_slice(&object.flags.to_le_bytes());
            bytes.extend_from_slice(&(object.payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&[0; 4]);
            bytes.extend_from_slice(name.as_bytes());
            bytes.extend_from_slice(&object.payload);
        }
        Some(bytes)
    }

    pub fn save(&self, path: &Path, replace: bool) -> Result<(), ConfiguredRuntimeError> {
        let bytes = self
            .encode()
            .ok_or_else(|| invalid("configuration", "object limits exceeded"))?;
        persist(path, &bytes, replace).map_err(|error| ConfiguredRuntimeError::Write(error.kind()))
    }
}

pub(super) fn persist(path: &Path, bytes: &[u8], replace: bool) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::Builder::new()
        .prefix(".reliquary-config-")
        .tempfile_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    if replace {
        temp.persist(path)?;
    } else {
        temp.persist_noclobber(path)?;
    }
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}

pub(super) fn read_bounded(path: &Path, limit: usize) -> io::Result<Zeroizing<Vec<u8>>> {
    let mut bytes = Zeroizing::new(Vec::with_capacity((limit + 1).min(4097)));
    File::open(path)?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(bytes)
}

pub(super) fn take<'a>(bytes: &mut &'a [u8], len: usize) -> Option<&'a [u8]> {
    let (head, rest) = bytes.split_at_checked(len)?;
    *bytes = rest;
    Some(head)
}

pub(super) fn take_array<'a, const N: usize>(bytes: &mut &'a [u8]) -> Option<&'a [u8; N]> {
    let (head, rest) = bytes.split_first_chunk()?;
    *bytes = rest;
    Some(head)
}

pub(super) fn read_string<'a>(bytes: &mut &'a [u8]) -> Option<&'a str> {
    let len = u32::from_le_bytes(*take_array(bytes)?) as usize;
    std::str::from_utf8(take(bytes, len)?).ok()
}

pub(super) fn write_string(bytes: &mut Vec<u8>, value: &str) -> Option<()> {
    let len = u32::try_from(value.len()).ok()?;
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Some(())
}
