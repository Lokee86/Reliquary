use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::fs::File;

const MAGIC: &[u8; 8] = b"CVCFG\0\r\n";
const MAJOR: u16 = 1;
const MINOR: u16 = 0;
const HEADER_LEN: usize = 16;
const OBJECT_HEADER_LEN: usize = 16;
const MAX_OBJECTS: usize = 4096;
const MAX_PAYLOAD_LEN: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigObject {
    pub schema: u16,
    pub flags: u32,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigDocument {
    objects: BTreeMap<String, ConfigObject>,
}

impl ConfigDocument {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn objects(&self) -> &BTreeMap<String, ConfigObject> {
        &self.objects
    }
    pub fn objects_mut(&mut self) -> &mut BTreeMap<String, ConfigObject> {
        &mut self.objects
    }
    pub fn get(&self, key: &str) -> Option<&ConfigObject> {
        self.objects.get(key)
    }
    pub fn insert(&mut self, key: impl Into<String>, object: ConfigObject) -> Option<ConfigObject> {
        self.objects.insert(key.into(), object)
    }
    pub fn remove(&mut self, key: &str) -> Option<ConfigObject> {
        self.objects.remove(key)
    }
}

#[derive(Debug)]
pub enum ConfigDocumentError {
    Io(std::io::Error),
    InvalidMagic,
    UnsupportedVersion { major: u16, minor: u16 },
    Truncated,
    InvalidObject,
    DuplicateObject(String),
}

impl fmt::Display for ConfigDocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "configuration I/O error: {e}"),
            Self::InvalidMagic => f.write_str("not a Reliquary configuration file"),
            Self::UnsupportedVersion { major, minor } => write!(
                f,
                "unsupported Reliquary configuration format {major}.{minor}"
            ),
            Self::Truncated => f.write_str("truncated Reliquary configuration file"),
            Self::InvalidObject => f.write_str("invalid Reliquary configuration object"),
            Self::DuplicateObject(key) => write!(f, "duplicate configuration object: {key}"),
        }
    }
}
impl std::error::Error for ConfigDocumentError {}
impl From<std::io::Error> for ConfigDocumentError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub fn decode_config_document(bytes: &[u8]) -> Result<ConfigDocument, ConfigDocumentError> {
    if bytes.len() < HEADER_LEN {
        return Err(ConfigDocumentError::Truncated);
    }
    if &bytes[..8] != MAGIC {
        return Err(ConfigDocumentError::InvalidMagic);
    }
    let major = u16::from_le_bytes(bytes[8..10].try_into().unwrap());
    let minor = u16::from_le_bytes(bytes[10..12].try_into().unwrap());
    if major != MAJOR || minor != MINOR {
        return Err(ConfigDocumentError::UnsupportedVersion { major, minor });
    }
    let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if count > MAX_OBJECTS {
        return Err(ConfigDocumentError::InvalidObject);
    }
    let mut cursor = HEADER_LEN;
    let mut document = ConfigDocument::new();
    for _ in 0..count {
        let header = bytes
            .get(cursor..cursor + OBJECT_HEADER_LEN)
            .ok_or(ConfigDocumentError::Truncated)?;
        let key_len = u16::from_le_bytes(header[0..2].try_into().unwrap()) as usize;
        let schema = u16::from_le_bytes(header[2..4].try_into().unwrap());
        let flags = u32::from_le_bytes(header[4..8].try_into().unwrap());
        let payload_len = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        let reserved = u32::from_le_bytes(header[12..16].try_into().unwrap());
        if reserved != 0 || payload_len > MAX_PAYLOAD_LEN {
            return Err(ConfigDocumentError::InvalidObject);
        }
        cursor += OBJECT_HEADER_LEN;
        let key_end = cursor
            .checked_add(key_len)
            .ok_or(ConfigDocumentError::InvalidObject)?;
        let payload_end = key_end
            .checked_add(payload_len)
            .ok_or(ConfigDocumentError::InvalidObject)?;
        let key = std::str::from_utf8(
            bytes
                .get(cursor..key_end)
                .ok_or(ConfigDocumentError::Truncated)?,
        )
        .map_err(|_| ConfigDocumentError::InvalidObject)?
        .to_owned();
        let payload = bytes
            .get(key_end..payload_end)
            .ok_or(ConfigDocumentError::Truncated)?
            .to_vec();
        if document
            .objects
            .insert(
                key.clone(),
                ConfigObject {
                    schema,
                    flags,
                    payload,
                },
            )
            .is_some()
        {
            return Err(ConfigDocumentError::DuplicateObject(key));
        }
        cursor = payload_end;
    }
    if cursor != bytes.len() {
        return Err(ConfigDocumentError::InvalidObject);
    }
    Ok(document)
}

pub fn encode_config_document(document: &ConfigDocument) -> Result<Vec<u8>, ConfigDocumentError> {
    let objects = &document.objects;
    if objects.len() > MAX_OBJECTS {
        return Err(ConfigDocumentError::InvalidObject);
    }
    let mut bytes = Vec::with_capacity(HEADER_LEN + objects.len() * 32);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&MAJOR.to_le_bytes());
    bytes.extend_from_slice(&MINOR.to_le_bytes());
    bytes.extend_from_slice(&(objects.len() as u32).to_le_bytes());
    for (key, object) in objects {
        let key_bytes = key.as_bytes();
        let key_len =
            u16::try_from(key_bytes.len()).map_err(|_| ConfigDocumentError::InvalidObject)?;
        let payload_len =
            u32::try_from(object.payload.len()).map_err(|_| ConfigDocumentError::InvalidObject)?;
        if object.payload.len() > MAX_PAYLOAD_LEN {
            return Err(ConfigDocumentError::InvalidObject);
        }
        bytes.extend_from_slice(&key_len.to_le_bytes());
        bytes.extend_from_slice(&object.schema.to_le_bytes());
        bytes.extend_from_slice(&object.flags.to_le_bytes());
        bytes.extend_from_slice(&payload_len.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(key_bytes);
        bytes.extend_from_slice(&object.payload);
    }
    Ok(bytes)
}

pub fn read_config_document(path: impl AsRef<Path>) -> Result<ConfigDocument, ConfigDocumentError> {
    decode_config_document(&fs::read(path)?)
}

pub fn replace_config_document(
    path: impl AsRef<Path>,
    document: &ConfigDocument,
) -> Result<(), ConfigDocumentError> {
    let path = path.as_ref();
    let bytes = encode_config_document(document)?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("reliquary.cfg");
    let temp: PathBuf =
        path.with_file_name(format!(".{name}.{}.{}.tmp", std::process::id(), stamp));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        atomic_replace(&temp, path)?;
        sync_parent(parent)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(not(windows))]
fn atomic_replace(from: &Path, to: &Path) -> Result<(), ConfigDocumentError> {
    fs::rename(from, to)?;
    Ok(())
}
#[cfg(windows)]
fn atomic_replace(from: &Path, to: &Path) -> Result<(), ConfigDocumentError> {
    use std::os::windows::ffi::OsStrExt;
    const REPLACE: u32 = 1;
    const WRITE_THROUGH: u32 = 8;
    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    let ok = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), REPLACE | WRITE_THROUGH) };
    if ok == 0 {
        return Err(ConfigDocumentError::Io(std::io::Error::last_os_error()));
    }
    Ok(())
}
#[cfg(unix)]
fn sync_parent(parent: &Path) -> Result<(), ConfigDocumentError> {
    File::open(parent)?.sync_all()?;
    Ok(())
}
#[cfg(not(unix))]
fn sync_parent(_parent: &Path) -> Result<(), ConfigDocumentError> {
    Ok(())
}
