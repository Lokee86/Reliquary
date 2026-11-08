mod specialty;

use super::{ConfigDocument, ConfigFile, routes};
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
        ("extension.future", 99, 99, vec![0]),
    ]);
    fixture.write_key(KEY);
    let file = ConfigFile::open(&fixture.config).unwrap();
    let config = routes::decode(&file.document).unwrap();
    let credentials = &file.credentials;
    assert_eq!(credentials.entries.len(), 2);
    let general = config.general.unwrap();
    assert_eq!(general.reasoning_effort, Some("high"));
    let general = general.endpoint;
    assert_eq!(
        general.url,
        "https://chatgpt.com/backend-api/codex/responses"
    );
    let (token, account) = credentials
        .auth(&general.credential_id, ModelProvider::OpenAiCodex)
        .unwrap();
    assert!(token.0 == ACCESS_TOKEN);
    assert!(account == Some(ACCOUNT));

    let routes = ConfiguredRoutes::from_path(&fixture.config).unwrap();
    assert_eq!(routes.general_endpoint().unwrap().model(), "private-model");
    let embedding = routes.embedding_endpoint().unwrap();
    assert_eq!(embedding.dimensions(), 3);
    assert_eq!(embedding.normalization(), EmbeddingNormalization::L2);
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
        ConfiguredRoutes::from_path(&fixture.config),
        Err(ConfiguredRuntimeError::Credential {
            route: "credentials",
            reason: "companion master key is missing"
        }),
    ));
    assert!(!fixture.key.exists());
    fixture.write_key([0; 32]);
    assert!(matches!(
        ConfiguredRoutes::from_path(&fixture.config),
        Err(ConfiguredRuntimeError::Credential {
            route: "credentials",
            reason: "protected credential authentication failed"
        }),
    ));

    for (record, route, reason) in [
        (
            ("extension.other", 1, 1, vec![0]),
            "entity_resolution_decision",
            "credential is missing",
        ),
        (
            ("credential.dedicated", 1, 0, cipher.clone()),
            "credentials",
            "unsupported protected credential object",
        ),
        (
            ("credential.dedicated", 1, 1, cipher),
            "credentials",
            "protected credential authentication failed",
        ),
    ] {
        let fixture = Fixture::new(&[general.clone(), api_record.clone(), entity.clone(), record]);
        fixture.write_key(KEY);
        let error = ConfiguredRoutes::from_path(&fixture.config).err().unwrap();
        assert_eq!(error, ConfiguredRuntimeError::Credential { route, reason });
        for private in [API_KEY, "private-entity", URL, "credential.dedicated"] {
            assert!(!format!("{error:?} {error}").contains(private));
        }
    }
}

#[test]
fn skips_unused_key_and_rejects_invalid_routes_and_framing() {
    let fixture = Fixture::new(&[("extension.unused", 99, 99, vec![0])]);
    let routes = ConfiguredRoutes::from_path(&fixture.config).unwrap();
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
        ConfiguredRoutes::from_path(&fixture.config),
        Err(ConfiguredRuntimeError::Invalid {
            route: "decision",
            ..
        }),
    ));

    let object = ("unknown", 1, 0, vec![0]);
    assert!(ConfigDocument::decode(&frame(&[object.clone(), object.clone()])).is_none());
    let mut bytes = frame(&[object]);
    assert!(ConfigDocument::decode(&bytes[..bytes.len() - 1]).is_none());
    bytes.push(0);
    assert!(ConfigDocument::decode(&bytes).is_none());
}

#[test]
fn route_changes_preserve_opaque_objects_and_leave_credential_ciphertext_unchanged() {
    use crate::{ModelRoute, ModelSlot};
    let fixture = Fixture::new(&[
        (
            "models.general",
            2,
            0,
            fields(&[2, 0, 0, 0], &["model", URL, "api"]),
        ),
        ("future.object", 42, 99, vec![0, 1, 2, 255]),
        (
            "credential.api",
            1,
            1,
            encrypted("credential.api", 8, fields(&[1, 0, 0, 0], &[API_KEY])),
        ),
    ]);
    fixture.write_key(KEY);
    let key_bytes = fs::read(&fixture.key).unwrap();
    let mut file = ConfigFile::open(&fixture.config).unwrap();
    let credential = file.object("credential.api").unwrap().clone();
    let unknown = file.object("future.object").unwrap().clone();
    let general = file.model(ModelSlot::General).unwrap().unwrap();
    let id = crate::CredentialId::new("api").unwrap();
    assert!(file.remove_credential(&id).is_err());
    file.set_model(ModelSlot::General, &general).unwrap();
    assert_eq!(file.object("models.general").unwrap().schema, 3);
    let ModelRoute::General(ref route) = general else {
        unreachable!()
    };
    let decision = ModelRoute::Decision(crate::DecisionModelEndpoint {
        model: "decision".into(),
        url: URL.into(),
        credential_id: id.clone(),
    });
    let embedding = ModelRoute::Embedding(crate::EmbeddingModelEndpoint {
        provider: ModelProvider::OpenAiReady,
        model: "embed".into(),
        url: URL.into(),
        credential_id: id.clone(),
        dimensions: 3,
        normalization: EmbeddingNormalization::L2,
    });
    for slot in ModelSlot::ALL {
        let model = if slot.is_decision() {
            &decision
        } else if slot == ModelSlot::Embedding {
            &embedding
        } else {
            &general
        };
        file.set_model(slot, model).unwrap();
    }
    assert_eq!(route.reasoning_effort, None);
    file.save().unwrap();
    let mut reopened = ConfigFile::open(&fixture.config).unwrap();
    for slot in ModelSlot::ALL {
        assert!(reopened.model(slot).unwrap().is_some());
    }
    assert!(reopened.object("future.object") == Some(&unknown));
    assert!(reopened.object("credential.api") == Some(&credential));
    assert_eq!(fs::read(&fixture.key).unwrap(), key_bytes);
    let before = fs::read(&fixture.config).unwrap();
    reopened.save().unwrap();
    assert_eq!(fs::read(&fixture.config).unwrap(), before);
    for slot in ModelSlot::ALL {
        reopened.clear_model(slot);
    }
    reopened.remove_credential(&id).unwrap();
    reopened.save().unwrap();
    let reopened = ConfigFile::open(&fixture.config).unwrap();
    assert!(reopened.credentials().next().is_none());
    assert!(reopened.object("future.object") == Some(&unknown));
    assert!(
        reopened
            .bind()
            .unwrap()
            .readiness()
            .contains("memory_retrieval=unavailable")
    );
}

#[test]
fn credential_writes_reuse_the_key_with_fresh_nonces_and_detect_tampering() {
    use crate::{CredentialId, SecretString};
    let fixture = Fixture::new(&[]);
    let before = fs::read(&fixture.config).unwrap();
    let id = CredentialId::new("api").unwrap();
    let mut collision = ConfigFile::new(&fixture.config);
    collision.add_api_key(id.clone(), SecretString::new(API_KEY).unwrap());
    assert_eq!(
        collision.save(),
        Err(ConfiguredRuntimeError::Write(
            std::io::ErrorKind::AlreadyExists
        ))
    );
    assert!(!fixture.key.exists());
    assert_eq!(fs::read(&fixture.config).unwrap(), before);

    let mut file = ConfigFile::open(&fixture.config).unwrap();
    file.add_api_key(id.clone(), SecretString::new(API_KEY).unwrap());
    assert!(!fixture.key.exists());
    file.save().unwrap();
    let first = file.object("credential.api").unwrap().payload.clone();
    let key_bytes = fs::read(&fixture.key).unwrap();
    file.add_api_key(id, SecretString::new(API_KEY).unwrap());
    file.save().unwrap();
    let second = &file.object("credential.api").unwrap().payload;
    assert_ne!(&first[..12], &second[..12]);
    assert_eq!(fs::read(&fixture.key).unwrap(), key_bytes);
    let reopen = ConfigFile::open(&fixture.config).unwrap();
    assert!(!format!("{:?}", reopen.credentials).contains(API_KEY));
    let mut tampered = second.clone();
    *tampered.last_mut().unwrap() ^= 1;
    fs::write(
        &fixture.config,
        frame(&[("credential.api", 1, 1, tampered)]),
    )
    .unwrap();
    assert!(matches!(
        ConfigFile::open(&fixture.config),
        Err(ConfiguredRuntimeError::Credential {
            reason: "protected credential authentication failed",
            ..
        })
    ));
    fs::write(
        &fixture.key,
        br#"{"version":1,"master_key_hex":"not-a-key"}"#,
    )
    .unwrap();
    assert!(matches!(
        ConfigFile::open(&fixture.config),
        Err(ConfiguredRuntimeError::Credential {
            reason: "invalid master key",
            ..
        })
    ));
}
