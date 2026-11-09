use reliquary_memory::test_support::mock_master_key_store;
use reliquary_memory::{
    CompatibilityProfile, ConfiguredRuntime, ConfiguredRuntimeError, CredentialError, CredentialId,
    CredentialKind, FragmentConfig, GeneralModelEndpoint, GeneralRoute, MasterKeyError,
    ModelProvider, ModelRoute, ModelSlot, ReliquaryConfig, RuntimeSettings, SecretString,
};

#[test]
fn root_only_configured_runtime_loads_and_reports_readiness_offline() {
    let _keys = mock_master_key_store();
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/oc02-fixtures")
        .join(format!("external-runtime-{}", std::process::id()));
    std::fs::create_dir_all(&fixture_dir).unwrap();
    let path = fixture_dir.join("config.cfg");
    let legacy_key = path.with_extension("master-key.json");
    let mut config = ReliquaryConfig::new_blank(&path);
    let id = CredentialId::new("TEST_API_KEY").unwrap();
    config.add_api_key(id.clone(), SecretString::new("synthetic-test-key").unwrap());
    config
        .set_model(
            ModelSlot::General,
            &ModelRoute::General(GeneralRoute {
                endpoint: GeneralModelEndpoint {
                    provider: ModelProvider::OpenAiReady,
                    model: "offline-test-model".into(),
                    url: "https://offline.example.invalid/v1/chat/completions".into(),
                    credential_id: id.clone(),
                },
                reasoning_effort: None,
            }),
        )
        .unwrap();
    let fragments = FragmentConfig {
        turns: 9,
        overlap: 2,
    };
    config.set_fragments(fragments).unwrap();
    let retrieval = reliquary_memory::RetrievalConfig {
        candidate_limit: 42,
        ..config.retrieval()
    };
    config.set_retrieval(retrieval).unwrap();
    let settings = RuntimeSettings {
        dream_inference_concurrency: 3,
    };
    config.set_runtime_settings(settings).unwrap();
    assert!(!path.exists() && !legacy_key.exists());
    config.save().unwrap();
    let config_bytes = std::fs::read(&path).unwrap();
    let reopened = ReliquaryConfig::open(&path).unwrap();
    assert_eq!(reopened.fragments(), fragments);
    assert_eq!(reopened.retrieval(), retrieval);
    assert_eq!(reopened.runtime_settings(), settings);
    assert_eq!(
        reopened.credentials().collect::<Vec<_>>(),
        [(&id, CredentialKind::ApiKey)]
    );
    reopened.validate().unwrap();
    let runtime = ConfiguredRuntime::from_path(&path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), config_bytes);
    assert!(!legacy_key.exists());
    let copy = fixture_dir.join("copied.cfg");
    std::fs::copy(&path, &copy).unwrap();
    assert!(matches!(
        ConfiguredRuntime::from_path(&copy),
        Err(ConfiguredRuntimeError::MasterKey(MasterKeyError::Missing)),
    ));
    std::fs::remove_dir_all(&fixture_dir).unwrap();
    assert!(matches!(
        CredentialId::new("invalid ID"),
        Err(CredentialError::InvalidId)
    ));
    let profile: Result<Option<CompatibilityProfile>, ConfiguredRuntimeError> =
        runtime.probe_embedding_profile();
    assert!(profile.unwrap().is_none());
    let readiness: String = runtime.readiness();
    let debug = format!("{runtime:?}");
    assert!(readiness.contains("chronos=general"));
    assert!(readiness.contains("memory_retrieval=general"));
    assert!(readiness.contains("embedding=false"));
    assert!(readiness.contains(
        "general selection: insomnia=general, dream=general, entity_resolution=unavailable"
    ));
    for private in [
        "TEST_API_KEY",
        "synthetic-test-key",
        "offline-test-model",
        "offline.example.invalid",
    ] {
        assert!(!readiness.contains(private));
        assert!(!debug.contains(private));
    }
}

#[test]
fn root_loads_empty_configuration_without_a_key() {
    let _keys = mock_master_key_store();
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/oc02-fixtures");
    std::fs::create_dir_all(&fixture_dir).unwrap();
    let path = fixture_dir.join(format!("external-protected-{}.cfg", std::process::id()));
    std::fs::write(&path, b"CVCFG\0\r\n\x01\x00\x00\x00\x00\x00\x00\x00").unwrap();
    let key_path = path.with_extension("master-key.json");
    let runtime = ConfiguredRuntime::from_path(&path);
    assert!(!key_path.exists());
    std::fs::remove_file(&path).unwrap();

    assert!(
        runtime
            .unwrap()
            .readiness()
            .contains("memory_retrieval=unavailable")
    );
}
