use reliquary_memory::{
    CompatibilityProfile, ConfiguredRuntime, ConfiguredRuntimeError, CredentialError, CredentialId,
    CredentialMaterial, CredentialResolver, EnvironmentCredentialResolver,
};

struct Resolver;

impl CredentialResolver for Resolver {
    fn resolve(&self, id: &CredentialId) -> Result<CredentialMaterial, String> {
        assert_eq!(id.to_string(), "TEST_API_KEY");
        Ok(CredentialMaterial::ApiKey("synthetic-test-key".into()))
    }
}

#[test]
fn root_only_configured_runtime_loads_and_reports_readiness_offline() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/oc02-fixtures");
    std::fs::create_dir_all(&fixture_dir).unwrap();
    let path = fixture_dir.join(format!("external-{}.json", std::process::id()));
    std::fs::write(
        &path,
        r#"{
          "general": {
            "model": "offline-test-model",
            "url": "https://offline.example.invalid/v1/chat/completions",
            "credential_id": "TEST_API_KEY"
          },
          "worker": { "lease_duration_ms": 150000 }
        }"#,
    )
    .unwrap();
    let runtime = ConfiguredRuntime::from_path(&path, &Resolver);
    std::fs::remove_file(&path).unwrap();
    let runtime = runtime.unwrap();
    let _: &dyn CredentialResolver = &EnvironmentCredentialResolver;
    assert!(matches!(
        CredentialId::new("invalid ID"),
        Err(CredentialError::InvalidId)
    ));
    assert_eq!(runtime.worker_lease_duration_ns(), 150_000_000_000);
    let profile: Result<Option<CompatibilityProfile>, ConfiguredRuntimeError> =
        runtime.probe_embedding_profile();
    assert!(profile.unwrap().is_none());
    let readiness: String = runtime.readiness();
    let debug = format!("{runtime:?}");
    assert!(readiness.contains("chronos=general"));
    assert!(readiness.contains("embedding=false"));
    assert!(
        readiness.contains(
            "general selection: insomnia=general, dream=general, entity_resolution=general"
        )
    );
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
fn root_protected_retrieval_override_requires_a_dedicated_insomnia_route() {
    let fixture_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/oc02-fixtures");
    std::fs::create_dir_all(&fixture_dir).unwrap();
    let path = fixture_dir.join(format!("external-protected-{}.cfg", std::process::id()));
    std::fs::write(&path, b"CVCFG\0\r\n\x01\x00\x00\x00\x00\x00\x00\x00").unwrap();
    let key_path = path.with_extension("unused.json");
    let standard = ConfiguredRuntime::from_protected_config(&path, &key_path);
    let overridden =
        ConfiguredRuntime::from_protected_config_with_retrieval_from_insomnia(&path, &key_path);
    std::fs::remove_file(&path).unwrap();

    assert!(
        standard
            .unwrap()
            .readiness()
            .contains("memory_retrieval=unavailable")
    );
    assert!(matches!(
        overridden,
        Err(ConfiguredRuntimeError::Invalid {
            route: "insomnia",
            ..
        })
    ));
}
