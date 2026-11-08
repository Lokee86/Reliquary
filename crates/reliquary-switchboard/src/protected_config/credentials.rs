use crate::config_document::{
    ConfigDocument, ConfigObject, persist, read_bounded, read_string, take_array, write_string,
};
use crate::{CodexRefresh, Credential, SecretString};
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use ring::rand::{SecureRandom, SystemRandom};
use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

pub(super) const AAD_DOMAIN: &[u8] = b"CVCFG-CREDENTIAL-V1\0";
pub(super) const PREFIX: &str = "credential.";
pub(crate) type MasterKey = Zeroizing<[u8; 32]>;

#[derive(serde::Deserialize)]
struct KeyFile<'a> {
    version: u64,
    #[serde(borrow)]
    master_key_hex: Cow<'a, str>,
}

pub(super) fn key_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .unwrap_or("reliquary".as_ref())
        .to_os_string();
    name.push(".master-key.json");
    path.with_file_name(name)
}

pub(super) fn load_key(path: &Path) -> Result<MasterKey, &'static str> {
    let bytes = match read_bounded(path, 4096) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err("companion master key is missing");
        }
        Err(_) => return Err("companion master key could not be read"),
    };
    if bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
        return Err("invalid master-key JSON");
    }
    let file: KeyFile<'_> =
        serde_json::from_slice(&bytes).map_err(|_| "invalid master-key JSON")?;
    let encoded = Zeroizing::new(file.master_key_hex.into_owned());
    if file.version != 1 {
        return Err("unsupported master-key version");
    }
    let mut key = MasterKey::new([0; 32]);
    hex::decode_to_slice(encoded.as_bytes(), &mut *key).map_err(|_| "invalid master key")?;
    Ok(key)
}

pub(super) fn key_for_write(
    path: &Path,
    document: &ConfigDocument,
) -> Result<MasterKey, &'static str> {
    match load_key(path) {
        Err("companion master key is missing")
            if !document.0.keys().any(|name| name.starts_with(PREFIX)) =>
        {
            ()
        }
        result => return result,
    }
    let mut key = MasterKey::new([0; 32]);
    SystemRandom::new()
        .fill(&mut *key)
        .map_err(|_| "master-key entropy unavailable")?;
    let mut encoded = Zeroizing::new(*b"{\"version\":1,\"master_key_hex\":\"0000000000000000000000000000000000000000000000000000000000000000\"}\n");
    let start = b"{\"version\":1,\"master_key_hex\":\"".len();
    hex::encode_to_slice(&*key, &mut encoded[start..start + 64])
        .map_err(|_| "master-key encoding failed")?;
    match persist(path, &*encoded, false) {
        Ok(()) => Ok(key),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => load_key(path),
        Err(_) => Err("companion master key could not be created"),
    }
}

fn cipher(key: &MasterKey) -> Result<LessSafeKey, &'static str> {
    UnboundKey::new(&AES_256_GCM, &**key)
        .map(LessSafeKey::new)
        .map_err(|_| "invalid master key")
}

fn aad(name: &str) -> Aad<Vec<u8>> {
    Aad::from([AAD_DOMAIN, name.as_bytes()].concat())
}

pub(crate) fn encode(
    name: &str,
    credential: &Credential,
    key: &MasterKey,
) -> Result<ConfigObject, &'static str> {
    let (tag, fields): (u8, &[&str]) = match credential {
        Credential::ApiKey { secret } => (1, &[&secret.0]),
        Credential::CodexOAuth {
            access_token,
            account_id,
            refresh: Some(refresh),
        } => (
            2,
            &[
                &refresh.id_token.0,
                &access_token.0,
                &refresh.refresh_token.0,
                account_id,
            ],
        ),
        _ => return Err("missing Codex refresh material"),
    };
    let size = 4 + fields.iter().map(|field| 4 + field.len()).sum::<usize>();
    if size > 16 * 1024 * 1024 - 28 {
        return Err("credential exceeds size limit");
    }
    let mut plaintext = Zeroizing::new(Vec::with_capacity(size));
    plaintext.extend_from_slice(&[tag, 0, 0, 0]);
    for field in fields {
        write_string(&mut plaintext, field).ok_or("credential exceeds size limit")?;
    }
    let mut nonce = [0; 12];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| "credential nonce entropy unavailable")?;
    let tag = cipher(key)?
        .seal_in_place_separate_tag(
            Nonce::assume_unique_for_key(nonce),
            aad(name),
            &mut plaintext,
        )
        .map_err(|_| "credential encryption failed")?;
    let mut payload = nonce.to_vec();
    payload.extend_from_slice(&plaintext);
    payload.extend_from_slice(tag.as_ref());
    Ok(ConfigObject {
        schema: 1,
        flags: 1,
        payload,
    })
}

pub(crate) fn decode(
    name: &str,
    object: &ConfigObject,
    key: &MasterKey,
) -> Result<Credential, &'static str> {
    if object.schema != 1 || object.flags != 1 {
        return Err("unsupported protected credential object");
    }
    let (nonce, ciphertext) = object
        .payload
        .split_first_chunk()
        .ok_or("invalid encrypted credential")?;
    let mut buffer = Zeroizing::new(ciphertext.to_vec());
    let plaintext = cipher(key)?
        .open_in_place(Nonce::assume_unique_for_key(*nonce), aad(name), &mut buffer)
        .map_err(|_| "protected credential authentication failed")?;
    decode_plaintext(plaintext).ok_or("invalid protected credential material")
}

fn decode_plaintext(mut bytes: &[u8]) -> Option<Credential> {
    let credential = match take_array(&mut bytes)? {
        [1, 0, 0, 0] => Credential::ApiKey {
            secret: read_secret(&mut bytes)?,
        },
        [2, 0, 0, 0] => {
            let id_token = read_secret(&mut bytes)?;
            let access_token = read_secret(&mut bytes)?;
            let refresh_token = read_secret(&mut bytes)?;
            let account_id = read_string(&mut bytes)?;
            Credential::CodexOAuth {
                access_token,
                account_id: account_id.into(),
                refresh: Some(CodexRefresh {
                    id_token,
                    refresh_token,
                }),
            }
        }
        _ => return None,
    };
    bytes.is_empty().then_some(credential)
}

fn read_secret(bytes: &mut &[u8]) -> Option<SecretString> {
    SecretString::new(read_string(bytes)?).ok()
}
