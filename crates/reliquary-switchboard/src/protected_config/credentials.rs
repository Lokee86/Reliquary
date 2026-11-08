use super::{Object, read_bounded, read_string, take_array};
use crate::CredentialMaterial;
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use std::borrow::Cow;
use std::path::Path;
use zeroize::Zeroizing;

pub(super) const AAD_DOMAIN: &[u8] = b"CVCFG-CREDENTIAL-V1\0";
const MAX_KEY_FILE_BYTES: usize = 4 * 1024;
const KEY_BYTES: usize = 32;

#[derive(serde::Deserialize)]
struct KeyFile<'a> {
    version: u64,
    #[serde(borrow)]
    master_key_hex: Cow<'a, str>,
}

pub(super) fn load_master_key(path: &Path) -> Result<Zeroizing<[u8; KEY_BYTES]>, &'static str> {
    let bytes = read_bounded(path, MAX_KEY_FILE_BYTES)
        .map_err(|_| "protected master key could not be read")?;
    if bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
        return Err("invalid protected master key");
    }
    let file: KeyFile<'_> =
        serde_json::from_slice(&bytes).map_err(|_| "invalid protected master key")?;
    let encoded = Zeroizing::new(file.master_key_hex.into_owned());
    if file.version != 1 {
        return Err("unsupported protected master-key version");
    }
    let mut key = Zeroizing::new([0; KEY_BYTES]);
    hex::decode_to_slice(encoded.as_bytes(), &mut *key)
        .map_err(|_| "invalid protected master key")?;
    Ok(key)
}

pub(super) fn decode(
    name: &str,
    object: &Object<'_>,
    key: &[u8; KEY_BYTES],
) -> Result<CredentialMaterial, &'static str> {
    if object.schema != 1 || object.flags != 1 {
        return Err("unsupported protected credential object");
    }
    let (nonce, ciphertext) = object
        .payload
        .split_first_chunk()
        .ok_or("invalid encrypted credential")?;
    let mut buffer = Zeroizing::new(ciphertext.to_vec());
    let aad = [AAD_DOMAIN, name.as_bytes()].concat();
    let cipher = LessSafeKey::new(
        UnboundKey::new(&AES_256_GCM, key).map_err(|_| "invalid protected master key")?,
    );
    let plaintext = cipher
        .open_in_place(
            Nonce::assume_unique_for_key(*nonce),
            Aad::from(aad),
            &mut *buffer,
        )
        .map_err(|_| "protected credential authentication failed")?;
    decode_plaintext(plaintext).ok_or("invalid protected credential material")
}

fn decode_plaintext(mut bytes: &[u8]) -> Option<CredentialMaterial> {
    match take_array(&mut bytes)? {
        [1, 0, 0, 0] => {
            let key = read_string(&mut bytes)?;
            if key.is_empty() || !bytes.is_empty() {
                return None;
            }
            Some(CredentialMaterial::ApiKey(key.to_owned()))
        }
        [2, 0, 0, 0] => {
            let id_token = read_string(&mut bytes)?;
            let access_token = read_string(&mut bytes)?;
            let refresh_token = read_string(&mut bytes)?;
            let account_id = read_string(&mut bytes)?;
            if [id_token, access_token, refresh_token, account_id].contains(&"")
                || !bytes.is_empty()
            {
                return None;
            }
            Some(CredentialMaterial::CodexOAuth {
                access_token: access_token.to_owned(),
                account_id: account_id.to_owned(),
            })
        }
        _ => None,
    }
}
