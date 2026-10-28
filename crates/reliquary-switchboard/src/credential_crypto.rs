use crate::CredentialError;
use crate::MasterKey;
use crate::master_key_entropy::fill_random;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use zeroize::Zeroize;

const NONCE_BYTES: usize = 12;
const AAD_DOMAIN: &[u8] = b"CVCFG-CREDENTIAL-V1\0";

pub(crate) fn encrypt_credential(
    object_key: &str,
    key: &MasterKey,
    plaintext: &mut Vec<u8>,
) -> Result<Vec<u8>, CredentialError> {
    let cipher =
        Aes256Gcm::new_from_slice(key.bytes()).map_err(|_| CredentialError::EncryptionFailed)?;
    let mut nonce_bytes = [0_u8; NONCE_BYTES];
    fill_random(&mut nonce_bytes).map_err(|_| CredentialError::EncryptionFailed)?;
    let aad = aad(object_key);
    let encrypted = cipher.encrypt(
        Nonce::from_slice(&nonce_bytes),
        Payload {
            msg: plaintext,
            aad: &aad,
        },
    );
    plaintext.zeroize();
    let ciphertext = encrypted.map_err(|_| CredentialError::EncryptionFailed)?;

    let mut payload = Vec::with_capacity(NONCE_BYTES + ciphertext.len());
    payload.extend_from_slice(&nonce_bytes);
    payload.extend_from_slice(&ciphertext);
    Ok(payload)
}

pub(crate) fn decrypt_credential(
    object_key: &str,
    key: &MasterKey,
    payload: &[u8],
) -> Result<Vec<u8>, CredentialError> {
    if payload.len() <= NONCE_BYTES {
        return Err(CredentialError::InvalidEncryptedObject);
    }
    let (nonce, ciphertext) = payload.split_at(NONCE_BYTES);
    let cipher =
        Aes256Gcm::new_from_slice(key.bytes()).map_err(|_| CredentialError::DecryptionFailed)?;
    let aad = aad(object_key);
    cipher
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| CredentialError::DecryptionFailed)
}

fn aad(object_key: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(AAD_DOMAIN.len() + object_key.len());
    aad.extend_from_slice(AAD_DOMAIN);
    aad.extend_from_slice(object_key.as_bytes());
    aad
}
