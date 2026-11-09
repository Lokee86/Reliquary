use super::*;
use crate::{CredentialId, SecretString};
use zeroize::Zeroizing;

fn add_credential(config: &mut ConfigFile) {
    config.add_api_key(
        CredentialId::new("api").unwrap(),
        SecretString::new(API_KEY).unwrap(),
    );
}

fn api_record() -> Record<'static> {
    (
        "credential.api",
        1,
        1,
        encrypted("credential.api", 20, fields(&[1, 0, 0, 0], &[API_KEY])),
    )
}

fn write_legacy_key(fixture: &Fixture) -> Zeroizing<String> {
    let hex = Zeroizing::new(hex::encode(KEY));
    let legacy = Zeroizing::new(format!(
        r#"{{"version":1,"master_key_hex":"{}"}}"#,
        hex.as_str()
    ));
    fs::write(&fixture.legacy_key, legacy.as_bytes()).unwrap();
    legacy
}

fn assert_migration_failed(result: Result<ConfigFile, ConfiguredRuntimeError>) {
    assert!(
        matches!(
            &result,
            Err(ConfiguredRuntimeError::MasterKey(
                MasterKeyError::Migration(_)
            )),
        ),
        "{result:?}"
    );
}

#[test]
fn first_credential_write_uses_a_key_bound_to_the_config_path() {
    let keys = mock_master_key_store();
    let first = Fixture::new(&[]);
    let second = Fixture::new(&[]);
    let copied = Fixture::new(&[]);
    let id = CredentialId::new("api").unwrap();
    for fixture in [&first, &second] {
        let mut file = ConfigFile::open(&fixture.config).unwrap();
        add_credential(&mut file);
        file.save().unwrap();
        assert!(!fixture.legacy_key.exists());

        let key = keys.key(&fixture.config).unwrap();
        let bytes = fs::read(&fixture.config).unwrap();
        assert!(!bytes.windows(key.len()).any(|window| window == &key[..]));
        assert!(bytes.starts_with(b"CVCFG\0\r\n"));

        let alias = fixture
            .config
            .parent()
            .unwrap()
            .join(".")
            .join("fixture.cfg");
        let reopened = ConfigFile::open(&alias).unwrap();
        assert_eq!(reopened.credentials().next().unwrap().0, &id);
    }
    let first_key = keys.key(&first.config).unwrap();
    let second_key = keys.key(&second.config).unwrap();
    assert!(*first_key != *second_key);

    fs::copy(&first.config, &copied.config).unwrap();
    assert!(matches!(
        ConfigFile::open(&copied.config),
        Err(ConfiguredRuntimeError::MasterKey(MasterKeyError::Missing)),
    ));
    assert!(keys.key(&copied.config).is_none());

    fs::copy(&first.config, &second.config).unwrap();
    assert!(matches!(
        ConfigFile::open(&second.config),
        Err(ConfiguredRuntimeError::Credential {
            reason: "protected credential authentication failed",
            ..
        }),
    ));
    assert!(*keys.key(&second.config).unwrap() == *second_key);
}

#[test]
fn key_store_errors_fail_closed_and_allow_a_failed_first_write_to_retry() {
    let keys = mock_master_key_store();
    let fixture = Fixture::new(&[]);
    let before = fs::read(&fixture.config).unwrap();
    keys.fail_load(true);
    let mut config = ConfigFile::open(&fixture.config).unwrap();
    config.bind().unwrap();
    config.save().unwrap();
    assert_eq!(fs::read(&fixture.config).unwrap(), before);

    add_credential(&mut config);
    assert_eq!(
        config.save(),
        Err(ConfiguredRuntimeError::MasterKey(
            MasterKeyError::StoreUnavailable
        )),
    );
    keys.fail_load(false);
    keys.fail_write(true);
    assert_eq!(
        config.save(),
        Err(ConfiguredRuntimeError::MasterKey(
            MasterKeyError::WriteRejected
        )),
    );
    assert!(keys.key(&fixture.config).is_none());
    assert!(!fixture.legacy_key.exists());
    assert_eq!(fs::read(&fixture.config).unwrap(), before);

    keys.fail_write(false);
    config.save().unwrap();
    let before = fs::read(&fixture.config).unwrap();
    keys.remove(&fixture.config);
    add_credential(&mut config);
    assert!(matches!(
        config.save(),
        Err(ConfiguredRuntimeError::MasterKey(MasterKeyError::Missing)),
    ));
    assert!(keys.key(&fixture.config).is_none());
    assert_eq!(fs::read(&fixture.config).unwrap(), before);
}

#[test]
fn a_stale_empty_handle_cannot_replace_the_missing_key_of_an_encrypted_config() {
    let keys = mock_master_key_store();
    let fixture = Fixture::new(&[]);
    let mut stale = ConfigFile::open(&fixture.config).unwrap();
    let mut writer = ConfigFile::open(&fixture.config).unwrap();
    add_credential(&mut writer);
    writer.save().unwrap();
    let encrypted = fs::read(&fixture.config).unwrap();
    keys.remove(&fixture.config);

    add_credential(&mut stale);
    assert_eq!(
        stale.save(),
        Err(ConfiguredRuntimeError::MasterKey(MasterKeyError::Missing)),
    );
    assert!(keys.key(&fixture.config).is_none());
    assert_eq!(fs::read(&fixture.config).unwrap(), encrypted);
}

#[test]
fn legacy_migration_imports_the_exact_key_and_preserves_config_bytes() {
    let keys = mock_master_key_store();
    let fixture = Fixture::new(&[api_record(), ("future.object", 99, 0, vec![42])]);
    write_legacy_key(&fixture);
    let before = fs::read(&fixture.config).unwrap();

    assert_eq!(
        ConfigFile::open(&fixture.config)
            .unwrap()
            .credentials()
            .count(),
        1
    );
    assert!(*keys.key(&fixture.config).unwrap() == KEY);
    assert!(!fixture.legacy_key.exists());
    assert_eq!(fs::read(&fixture.config).unwrap(), before);

    keys.fail_write(true);
    assert_eq!(
        ConfigFile::open(&fixture.config)
            .unwrap()
            .credentials()
            .count(),
        1
    );
    assert_eq!(fs::read(&fixture.config).unwrap(), before);
}

#[test]
fn failed_legacy_migrations_preserve_the_companion_and_ciphertext() {
    let keys = mock_master_key_store();
    let fixture = Fixture::new(&[api_record()]);
    let mut config = fs::read(&fixture.config).unwrap();
    let legacy = write_legacy_key(&fixture);

    keys.fail_write(true);
    assert_migration_failed(ConfigFile::open(&fixture.config));
    assert!(fs::read(&fixture.legacy_key).unwrap() == legacy.as_bytes());
    assert_eq!(fs::read(&fixture.config).unwrap(), config);
    assert!(keys.key(&fixture.config).is_none());

    keys.fail_write(false);
    let mut second_payload = encrypted("credential.z", 21, fields(&[1, 0, 0, 0], &[API_KEY]));
    *second_payload.last_mut().unwrap() ^= 1;
    config = frame(&[api_record(), ("credential.z", 1, 1, second_payload)]);
    fs::write(&fixture.config, &config).unwrap();
    assert_migration_failed(ConfigFile::open(&fixture.config));
    assert!(fs::read(&fixture.legacy_key).unwrap() == legacy.as_bytes());
    assert_eq!(fs::read(&fixture.config).unwrap(), config);
    assert!(keys.key(&fixture.config).is_none());

    fs::write(&fixture.legacy_key, b"invalid legacy data").unwrap();
    assert_migration_failed(ConfigFile::open(&fixture.config));
    assert_eq!(
        fs::read(&fixture.legacy_key).unwrap(),
        b"invalid legacy data"
    );
    assert_eq!(fs::read(&fixture.config).unwrap(), config);
    assert!(keys.key(&fixture.config).is_none());
}

#[test]
fn interrupted_legacy_migration_retires_the_companion_on_reopen() {
    let keys = mock_master_key_store();
    let fixture = Fixture::new(&[api_record()]);
    write_legacy_key(&fixture);
    let before = fs::read(&fixture.config).unwrap();
    keys.replace(&fixture.config, KEY.to_vec());
    keys.fail_write(true);

    assert_eq!(
        ConfigFile::open(&fixture.config)
            .unwrap()
            .credentials()
            .count(),
        1
    );
    assert!(*keys.key(&fixture.config).unwrap() == KEY);
    assert!(!fixture.legacy_key.exists());
    assert_eq!(fs::read(&fixture.config).unwrap(), before);
}

#[test]
fn a_legacy_companion_for_a_different_config_cannot_override_a_valid_os_key() {
    let _keys = mock_master_key_store();
    let legacy = Fixture::new(&[api_record()]);
    write_legacy_key(&legacy);
    let other_path = legacy.config.with_extension("other");
    let mut other = ConfigFile::new(&other_path);
    add_credential(&mut other);
    other.save().unwrap();

    assert_eq!(
        ConfigFile::open(&other_path).unwrap().credentials().count(),
        1
    );
    assert!(legacy.legacy_key.is_file());
    ConfigFile::open(&legacy.config).unwrap();
    assert!(!legacy.legacy_key.exists());
}
