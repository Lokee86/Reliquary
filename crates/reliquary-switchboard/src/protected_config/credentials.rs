use super::master_key::MasterKey;
use crate::config_document::{ConfigObject, read_string, take_array, write_string};
use crate::{CodexRefresh, Credential, SecretString};
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use ring::rand::{SecureRandom, SystemRandom};
use zeroize::Zeroizing;

pub(super) const AAD_DOMAIN: &[u8] = b"CVCFG-CREDENTIAL-V1\0";
pub(super) const PREFIX: &str = "credential.";

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
