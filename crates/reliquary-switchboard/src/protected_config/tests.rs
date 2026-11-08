use super::{decode_objects, load};
use crate::{ConfiguredRoutes, ConfiguredRuntimeError, EmbeddingNormalization, ModelProvider};
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const KEY: [u8; 32] = [0x79; 32];
const API_KEY: &str = "synthetic-provider-key";
const ACCESS_TOKEN: &str = "synthetic-oauth-access";
const ACCOUNT: &str = "synthetic-oauth-account";
const URL: &str = "https://offline.example.invalid/endpoint";

type Record<'a> = (&'a str, u16, u32, Vec<u8>);

struct Fixture {
    dir: PathBuf,
    config: PathBuf,
    key: PathBuf,
}

impl Fixture {
    fn new(records: &[Record<'_>]) -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/oc02-fixtures")
            .join(format!(
                "protected-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed),
            ));
        fs::create_dir_all(&dir).unwrap();
        let config = dir.join("fixture.cfg");
        let key = dir.join("fixture.master-key.json");
        fs::write(&config, frame(records)).unwrap();
        Self { dir, config, key }
    }

    fn write_key(&self, key: [u8; 32]) {
        fs::write(
            &self.key,
            format!(r#"{{"version":1,"master_key_hex":"{}"}}"#, hex::encode(key)),
        )
        .unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.config);
        let _ = fs::remove_file(&self.key);
        let _ = fs::remove_dir(&self.dir);
    }
}

fn frame(records: &[Record<'_>]) -> Vec<u8> {
    let mut bytes = b"CVCFG\0\r\n\x01\x00\x00\x00".to_vec();
    bytes.extend((records.len() as u32).to_le_bytes());
    for (name, schema, flags, payload) in records {
        bytes.extend((name.len() as u16).to_le_bytes());
        bytes.extend(schema.to_le_bytes());
        bytes.extend(flags.to_le_bytes());
        bytes.extend((payload.len() as u32).to_le_bytes());
        bytes.extend([0; 4]);
        bytes.extend(name.as_bytes());
        bytes.extend(payload);
    }
    bytes
}

fn fields(prefix: &[u8], strings: &[&str]) -> Vec<u8> {
    let mut bytes = prefix.to_vec();
    for string in strings {
        bytes.extend((string.len() as u32).to_le_bytes());
        bytes.extend(string.as_bytes());
    }
    bytes
}

fn encrypted(name: &str, nonce_id: u8, mut plaintext: Vec<u8>) -> Vec<u8> {
    let nonce = [nonce_id; 12];
    let aad = format!("CVCFG-CREDENTIAL-V1\0{name}");
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &KEY).unwrap());
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::from(aad),
        &mut plaintext,
    )
    .unwrap();
    [nonce.as_slice(), &plaintext].concat()
}

#[test]
fn loads_selected_oauth_api_key_and_embedding_routes_offline() {
    let oauth = fields(
        &[2, 0, 0, 0],
        &["id-token", ACCESS_TOKEN, "refresh-token", ACCOUNT],
    );
    let api_key = fields(&[1, 0, 0, 0], &[API_KEY]);
    let fixture = Fixture::new(&[
        (
            "models.general",
            3,
            0,
            fields(&[1, 5, 0, 0], &["private-model", "", "oauth"]),
        ),
        (
            "models.entity_resolution_decision",
            1,
            0,
            fields(&[], &["private-decision", URL, "api"]),
        ),
        (
            "models.embedding",
            2,
            0,
            fields(
                &[2, 1, 0, 0, 3, 0, 0, 0],
                &["private-embedding", URL, "api"],
            ),
        ),
        (
            "credential.oauth",
            1,
            1,
            encrypted("credential.oauth", 1, oauth),
        ),
        (
            "credential.api",
            1,
            1,
            encrypted("credential.api", 2, api_key),
        ),
        ("models.dream", 99, 99, vec![0]),
        ("credential.unused", 1, 1, vec![0]),
    ]);
    fixture.write_key(KEY);
    let (config, credentials) = load(&fixture.config, &fixture.key).ok().unwrap();
    assert_eq!(credentials.entries.len(), 2);
    let general = config.routes.general.unwrap();
    assert_eq!(
        general.url,
        "https://chatgpt.com/backend-api/codex/responses"
    );
    assert_eq!(config.general_reasoning_effort, Some("high"));
    let (token, account) = credentials
        .auth(&general.credential_id, ModelProvider::OpenAiCodex)
        .unwrap();
    assert!(token.0 == ACCESS_TOKEN);
    assert!(account == Some(ACCOUNT));

    let routes = ConfiguredRoutes::from_protected_config(&fixture.config, &fixture.key).unwrap();
    assert_eq!(routes.general_endpoint().unwrap().model(), "private-model");
    let embedding = routes.embedding_endpoint().unwrap();
    assert_eq!(embedding.dimensions(), 3);
    assert_eq!(embedding.normalization(), EmbeddingNormalization::L2);
    assert_eq!(routes.worker_lease_duration_ms(), None);
    let report = routes.readiness();
    assert!(report.contains(
        "chronos=general, entity_resolution=entity_resolution_decision, memory_retrieval=general"
    ));
    for private in [ACCESS_TOKEN, ACCOUNT, API_KEY, URL, "private-", "oauth"] {
        assert!(!report.contains(private));
    }
}

#[test]
fn selected_credential_failures_stop_route_binding_and_are_redacted() {
    let api_key = fields(&[1, 0, 0, 0], &[API_KEY]);
    let cipher = encrypted("credential.api", 3, api_key);
    let general = (
        "models.general",
        2,
        0,
        fields(&[2, 0, 0, 0], &["private-model", URL, "api"]),
    );
    let entity = (
        "models.entity_resolution_decision",
        1,
        0,
        fields(&[], &["private-entity", URL, "dedicated"]),
    );
    let api_record = ("credential.api", 1, 1, cipher.clone());
    let fixture = Fixture::new(&[general.clone(), api_record.clone()]);
    assert!(matches!(
        ConfiguredRoutes::from_protected_config(&fixture.config, &fixture.key),
        Err(ConfiguredRuntimeError::Credential {
            route: "general",
            reason: "protected master key could not be read"
        }),
    ));
    assert!(!fixture.key.exists());
    fixture.write_key([0; 32]);
    assert!(matches!(
        ConfiguredRoutes::from_protected_config(&fixture.config, &fixture.key),
        Err(ConfiguredRuntimeError::Credential {
            route: "general",
            reason: "protected credential authentication failed"
        }),
    ));

    for (record, reason) in [
        (
            ("credential.other", 1, 1, vec![0]),
            "protected credential is missing",
        ),
        (
            ("credential.dedicated", 1, 0, cipher.clone()),
            "unsupported protected credential object",
        ),
        (
            ("credential.dedicated", 1, 1, cipher),
            "protected credential authentication failed",
        ),
    ] {
        let fixture = Fixture::new(&[general.clone(), api_record.clone(), entity.clone(), record]);
        fixture.write_key(KEY);
        let error = ConfiguredRoutes::from_protected_config(&fixture.config, &fixture.key)
            .err()
            .unwrap();
        assert_eq!(
            error,
            ConfiguredRuntimeError::Credential {
                route: "entity_resolution_decision",
                reason,
            }
        );
        for private in [API_KEY, "private-entity", URL, "credential.dedicated"] {
            assert!(!format!("{error:?} {error}").contains(private));
        }
    }
}

#[test]
fn skips_unused_key_and_rejects_invalid_routes_and_framing() {
    let fixture = Fixture::new(&[("credential.unused", 99, 99, vec![0])]);
    let routes = ConfiguredRoutes::from_protected_config(&fixture.config, &fixture.key).unwrap();
    assert!(routes.general_endpoint().is_none());
    assert!(!fixture.key.exists());

    let fixture = Fixture::new(&[(
        "models.general_decision",
        1,
        0,
        fields(
            &[],
            &[
                "private-model",
                "http://offline.example.invalid/unsafe",
                "api",
            ],
        ),
    )]);
    assert!(matches!(
        ConfiguredRoutes::from_protected_config(&fixture.config, &fixture.key),
        Err(ConfiguredRuntimeError::Invalid {
            route: "decision",
            ..
        }),
    ));

    let object = ("unknown", 1, 0, vec![0]);
    assert!(decode_objects(&frame(&[object.clone(), object.clone()])).is_none());
    let mut bytes = frame(&[object]);
    assert!(decode_objects(&bytes[..bytes.len() - 1]).is_none());
    bytes.push(0);
    assert!(decode_objects(&bytes).is_none());
}
