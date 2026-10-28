use crate::ConfigObject as RawConfigObject;
use crate::credential_crypto::{decrypt_credential, encrypt_credential};
use crate::{Credential, CredentialError, CredentialId, MasterKey, SecretString};
use zeroize::Zeroize;

pub(crate) const CREDENTIAL_PREFIX: &str = "credential.";
pub(crate) const CREDENTIAL_SCHEMA_V1: u16 = 1;
pub(crate) const CREDENTIAL_FLAG_ENCRYPTED: u32 = 1;
const API_KEY_TAG: u8 = 1;
const CHATGPT_OAUTH_TAG: u8 = 2;

pub(crate) fn object_key(id: &CredentialId) -> String {
    format!("{CREDENTIAL_PREFIX}{}", id.as_str())
}

pub(crate) fn id_from_object_key(key: &str) -> Result<Option<CredentialId>, CredentialError> {
    let Some(id) = key.strip_prefix(CREDENTIAL_PREFIX) else {
        return Ok(None);
    };
    CredentialId::new(id.to_owned()).map(Some)
}

pub(crate) fn encode_credential_object(
    id: &CredentialId,
    credential: &Credential,
    key: &MasterKey,
) -> Result<RawConfigObject, CredentialError> {
    let object_key = object_key(id);
    let mut plaintext = encode_plaintext(credential)?;
    let payload = encrypt_credential(&object_key, key, &mut plaintext)?;
    Ok(RawConfigObject {
        schema: CREDENTIAL_SCHEMA_V1,
        flags: CREDENTIAL_FLAG_ENCRYPTED,
        payload,
    })
}

pub(crate) fn decode_credential_object(
    id: &CredentialId,
    object: &RawConfigObject,
    key: &MasterKey,
) -> Result<Credential, CredentialError> {
    if object.schema != CREDENTIAL_SCHEMA_V1 || object.flags != CREDENTIAL_FLAG_ENCRYPTED {
        return Err(CredentialError::InvalidEncryptedObject);
    }
    let object_key = object_key(id);
    let mut plaintext = decrypt_credential(&object_key, key, &object.payload)?;
    let result = decode_plaintext(&plaintext);
    plaintext.zeroize();
    result
}

fn encode_plaintext(credential: &Credential) -> Result<Vec<u8>, CredentialError> {
    let mut bytes = Vec::new();
    match credential {
        Credential::ApiKey { api_key } => {
            bytes.push(API_KEY_TAG);
            bytes.extend_from_slice(&[0; 3]);
            encode_string(&mut bytes, api_key.expose())?;
        }
        Credential::ChatGptOAuth {
            id_token,
            access_token,
            refresh_token,
            account_id,
        } => {
            bytes.push(CHATGPT_OAUTH_TAG);
            bytes.extend_from_slice(&[0; 3]);
            encode_string(&mut bytes, id_token.expose())?;
            encode_string(&mut bytes, access_token.expose())?;
            encode_string(&mut bytes, refresh_token.expose())?;
            encode_string(&mut bytes, account_id.as_deref().unwrap_or(""))?;
        }
    }
    Ok(bytes)
}

fn decode_plaintext(bytes: &[u8]) -> Result<Credential, CredentialError> {
    let header = bytes.get(..4).ok_or(CredentialError::InvalidCredential)?;
    if header[1..4] != [0; 3] {
        return Err(CredentialError::InvalidCredential);
    }
    let mut cursor = 4;
    let credential = match header[0] {
        API_KEY_TAG => Credential::ApiKey {
            api_key: SecretString::new(decode_string(bytes, &mut cursor)?)?,
        },
        CHATGPT_OAUTH_TAG => {
            let id_token = SecretString::new(decode_string(bytes, &mut cursor)?)?;
            let access_token = SecretString::new(decode_string(bytes, &mut cursor)?)?;
            let refresh_token = SecretString::new(decode_string(bytes, &mut cursor)?)?;
            let account_id = decode_string(bytes, &mut cursor)?;
            Credential::ChatGptOAuth {
                id_token,
                access_token,
                refresh_token,
                account_id: (!account_id.is_empty()).then_some(account_id),
            }
        }
        _ => return Err(CredentialError::InvalidCredential),
    };
    (cursor == bytes.len())
        .then_some(credential)
        .ok_or(CredentialError::InvalidCredential)
}

fn encode_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), CredentialError> {
    let len = u32::try_from(value.len()).map_err(|_| CredentialError::InvalidCredential)?;
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn decode_string(bytes: &[u8], cursor: &mut usize) -> Result<String, CredentialError> {
    let len_end = cursor
        .checked_add(4)
        .ok_or(CredentialError::InvalidCredential)?;
    let len_bytes = bytes
        .get(*cursor..len_end)
        .ok_or(CredentialError::InvalidCredential)?;
    let len = u32::from_le_bytes(len_bytes.try_into().unwrap()) as usize;
    let end = len_end
        .checked_add(len)
        .ok_or(CredentialError::InvalidCredential)?;
    let value = std::str::from_utf8(
        bytes
            .get(len_end..end)
            .ok_or(CredentialError::InvalidCredential)?,
    )
    .map_err(|_| CredentialError::InvalidCredential)?
    .to_owned();
    *cursor = end;
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{JsonMasterKeyStore, MasterKeyStore};

    #[test]
    fn encrypted_object_is_bound_to_its_credential_id() {
        let path =
            std::env::temp_dir().join(format!("switchboard-aad-{}.json", uuid::Uuid::new_v4()));
        let key = JsonMasterKeyStore::new(&path).load_or_create().unwrap();
        let original = CredentialId::new("original").unwrap();
        let other = CredentialId::new("other").unwrap();
        let credential = Credential::ApiKey {
            api_key: SecretString::new("test-secret").unwrap(),
        };
        let object = encode_credential_object(&original, &credential, &key).unwrap();
        assert_eq!(
            decode_credential_object(&original, &object, &key).unwrap(),
            credential
        );
        assert!(matches!(
            decode_credential_object(&other, &object, &key),
            Err(CredentialError::DecryptionFailed)
        ));
        std::fs::remove_file(path).unwrap();
    }
}
