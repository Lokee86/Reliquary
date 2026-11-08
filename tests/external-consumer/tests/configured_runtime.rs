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
