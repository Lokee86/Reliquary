use crate::master_key_entropy::fill_random;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const MASTER_KEY_BYTES: usize = 32;
const KEY_FILE_VERSION: u64 = 1;

#[derive(Clone, Eq, PartialEq)]
pub struct MasterKey([u8; MASTER_KEY_BYTES]);

impl MasterKey {
    pub(crate) fn bytes(&self) -> &[u8; MASTER_KEY_BYTES] {
        &self.0
    }
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MasterKey([REDACTED])")
    }
}

#[derive(Debug)]
pub enum MasterKeyError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidKey,
    UnsupportedVersion(u64),
    EntropyUnavailable,
}

impl fmt::Display for MasterKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "master-key I/O error: {error}"),
            Self::Json(error) => write!(f, "invalid master-key JSON: {error}"),
            Self::InvalidKey => write!(f, "invalid Reliquary master key"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported master-key JSON version {version}")
            }
            Self::EntropyUnavailable => write!(f, "operating-system entropy unavailable"),
        }
    }
}

impl std::error::Error for MasterKeyError {}

impl From<std::io::Error> for MasterKeyError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for MasterKeyError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub trait MasterKeyStore {
    fn load_or_create(&self) -> Result<MasterKey, MasterKeyError>;
}

#[derive(Clone, Debug)]
pub struct JsonMasterKeyStore {
    path: PathBuf,
}

impl JsonMasterKeyStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load_existing(&self) -> Result<MasterKey, MasterKeyError> {
        decode_key_file(&fs::read(&self.path)?)
    }
}

impl MasterKeyStore for JsonMasterKeyStore {
    fn load_or_create(&self) -> Result<MasterKey, MasterKeyError> {
        match self.load_existing() {
            Ok(key) => Ok(key),
            Err(MasterKeyError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                self.create_key()
            }
            Err(error) => Err(error),
        }
    }
}

impl JsonMasterKeyStore {
    fn create_key(&self) -> Result<MasterKey, MasterKeyError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut bytes = [0_u8; MASTER_KEY_BYTES];
        fill_random(&mut bytes)?;
        let key = MasterKey(bytes);
        let encoded = encode_key_file(&key)?;
        match create_private_file(&self.path, &encoded) {
            Ok(()) => Ok(key),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                decode_key_file(&fs::read(&self.path)?)
            }
            Err(error) => Err(error.into()),
        }
    }
}

fn encode_key_file(key: &MasterKey) -> Result<Vec<u8>, MasterKeyError> {
    let value = serde_json::json!({
        "version": KEY_FILE_VERSION,
        "master_key_hex": encode_hex(key.bytes()),
    });
    Ok(serde_json::to_vec_pretty(&value)?)
}

fn decode_key_file(bytes: &[u8]) -> Result<MasterKey, MasterKeyError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    let version = value
        .get("version")
        .and_then(|v| v.as_u64())
        .ok_or(MasterKeyError::InvalidKey)?;
    if version != KEY_FILE_VERSION {
        return Err(MasterKeyError::UnsupportedVersion(version));
    }
    let encoded = value
        .get("master_key_hex")
        .and_then(|v| v.as_str())
        .ok_or(MasterKeyError::InvalidKey)?;
    decode_hex_key(encoded)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_hex_key(encoded: &str) -> Result<MasterKey, MasterKeyError> {
    if encoded.len() != MASTER_KEY_BYTES * 2 {
        return Err(MasterKeyError::InvalidKey);
    }
    let mut bytes = [0_u8; MASTER_KEY_BYTES];
    for (index, pair) in encoded.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (hex_value(pair[0])? << 4) | hex_value(pair[1])?;
    }
    Ok(MasterKey(bytes))
}

fn hex_value(value: u8) -> Result<u8, MasterKeyError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(MasterKeyError::InvalidKey),
    }
}

fn create_private_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()
    })();
    if result.is_err() {
        drop(file);
        let _ = fs::remove_file(path);
    }
    result
}

/// Returns the machine-local key file path paired with a configuration file.
pub fn master_key_path(config_path: &Path) -> PathBuf {
    let stem = config_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("reliquary");
    config_path.with_file_name(format!("{stem}.master-key.json"))
}
