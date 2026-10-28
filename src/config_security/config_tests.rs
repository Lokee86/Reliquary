use crate::config_codec::{RawConfigObject, decode_config, encode_config};
use crate::{FragmentConfig, ReliquaryConfig, RetrievalConfig, RuntimeConfig};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-config-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn default_config_saves_and_reopens() {
    let path = test_path("reliquary.cfg");
    let config = ReliquaryConfig::new(&path);
    config.save().unwrap();

    let reopened = ReliquaryConfig::open(&path).unwrap();
    assert_eq!(reopened.fragments, FragmentConfig::default());
    assert_eq!(reopened.retrieval, RetrievalConfig::default());
    assert_eq!(reopened.runtime, RuntimeConfig::default());
    assert_eq!(reopened.models, crate::ModelSwitchboardConfig::default());
    assert_eq!(reopened.credentials, crate::CredentialsConfig::default());
    assert_eq!(&fs::read(&path).unwrap()[..8], b"CVCFG\0\r\n");
}

#[test]
fn replacing_config_does_not_accumulate_old_objects() {
    let path = test_path("replace.cfg");
    let mut config = ReliquaryConfig::new(&path);
    config.save().unwrap();
    let first_len = fs::metadata(&path).unwrap().len();

    config.fragments = FragmentConfig {
        turns: 12,
        overlap: 3,
    };
    config.retrieval = RetrievalConfig {
        candidate_limit: 40,
        result_limit: 12,
        lexical_weight: 1.0,
        semantic_weight: 3.0,
    };
    config.save().unwrap();
    let second_len = fs::metadata(&path).unwrap().len();
    config.save().unwrap();
    let third_len = fs::metadata(&path).unwrap().len();

    let reopened = ReliquaryConfig::open(&path).unwrap();
    assert_eq!(reopened.fragments, config.fragments);
    assert_eq!(reopened.retrieval, config.retrieval);
    assert_eq!(second_len, third_len);
    assert_eq!(first_len, second_len);
}

#[test]
fn unknown_objects_survive_known_config_replacement() {
    let path = test_path("future.cfg");
    let mut objects = BTreeMap::new();
    objects.insert(
        "future.object".into(),
        RawConfigObject {
            schema: 7,
            flags: 42,
            payload: b"future payload".to_vec(),
        },
    );
    fs::write(&path, encode_config(&objects).unwrap()).unwrap();

    let mut config = ReliquaryConfig::open(&path).unwrap();
    config.fragments.turns = 10;
    config.save().unwrap();

    let objects = decode_config(&fs::read(&path).unwrap()).unwrap();
    let future = objects.get("future.object").unwrap();
    assert_eq!(future.schema, 7);
    assert_eq!(future.flags, 42);
    assert_eq!(future.payload, b"future payload");
}

#[test]
fn retired_chronos_route_is_rejected_instead_of_preserved_as_unknown() {
    let path = test_path("old-chronos.cfg");
    let mut objects = BTreeMap::new();
    objects.insert(
        "models.chronos".into(),
        RawConfigObject {
            schema: 1,
            flags: 0,
            payload: Vec::new(),
        },
    );
    fs::write(&path, encode_config(&objects).unwrap()).unwrap();
    assert!(matches!(
        ReliquaryConfig::open(&path),
        Err(crate::ConfigError::InvalidModelSwitchboard)
    ));
}

#[test]
fn runtime_dream_concurrency_round_trips() {
    let path = test_path("runtime.cfg");
    let mut config = ReliquaryConfig::new(&path);
    config.runtime.dream_inference_concurrency = 8;
    config.save().unwrap();

    let reopened = ReliquaryConfig::open(&path).unwrap();
    assert_eq!(reopened.runtime.dream_inference_concurrency, 8);
}

#[test]
fn invalid_values_do_not_replace_existing_config() {
    let path = test_path("validation.cfg");
    let mut config = ReliquaryConfig::new(&path);
    config.save().unwrap();
    let original = fs::read(&path).unwrap();

    config.fragments.overlap = config.fragments.turns;
    assert!(config.save().is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn invalid_magic_is_rejected() {
    let path = test_path("invalid.cfg");
    fs::write(&path, b"not a continuity config").unwrap();
    assert!(ReliquaryConfig::open(&path).is_err());
}

#[test]
fn config_creates_temporary_master_key_beside_itself() {
    let path = test_path("reliquary.cfg");
    let config = ReliquaryConfig::new(&path);
    let first = config.load_or_create_master_key().unwrap();
    let second = config.load_or_create_master_key().unwrap();
    assert_eq!(first, second);
    assert!(path.with_file_name("reliquary.master-key.json").exists());
}
