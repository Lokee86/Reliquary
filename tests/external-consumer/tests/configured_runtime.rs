use reliquary_memory::test_support::mock_master_key_store;
use reliquary_memory::{
    CompatibilityProfile, ConfiguredRuntime, ConfiguredRuntimeError, CredentialError, CredentialId,
    CredentialKind, EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingNormalization,
    FragmentConfig, GeneralEndpoint, GeneralEndpointError, GeneralModelEndpoint, GeneralRoute,
    HostedEmbeddingInputSemantics, HostedInferenceIdentity, MasterKeyError, ModelProvider,
    ModelRoute, ModelSlot, ReliquaryConfig, RuntimeSettings, SecretString,
    SwitchboardEmbeddingMode,
};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct PublicGeneral {
    calls: AtomicUsize,
}

impl GeneralEndpoint for PublicGeneral {
    fn model(&self) -> &str {
        "general-model"
    }

    fn complete_json(
        &self,
        _system_prompt: &str,
        _user_payload: &str,
        _schema_name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(json!({"ok": true}))
    }
}

struct PublicEmbedding {
    calls: AtomicUsize,
}

impl EmbeddingEndpoint for PublicEmbedding {
    fn model(&self) -> Option<&str> {
        Some("embedding-model")
    }

    fn dimensions(&self) -> u32 {
        2
    }

    fn normalization(&self) -> EmbeddingNormalization {
        EmbeddingNormalization::L2
    }

    fn supports_bounded_embedding(&self) -> bool {
        true
    }

    fn embed_bounded(
        &self,
        _mode: SwitchboardEmbeddingMode,
        inputs: &[String],
        _max_response_bytes: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(inputs.iter().map(|_| vec![1.0, 0.0]).collect())
    }

    fn embed(
        &self,
        mode: SwitchboardEmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.embed_bounded(mode, inputs, usize::MAX)
    }
}

#[test]
fn root_only_hosted_runtime_uses_public_endpoint_contract_without_cvcfg() {
    let general = Arc::new(PublicGeneral {
        calls: AtomicUsize::new(0),
    });
    let embedding = Arc::new(PublicEmbedding {
        calls: AtomicUsize::new(0),
    });
    let identity = HostedInferenceIdentity::new(
        "hermes",
        "general-model",
        "hermes",
        "embedding-model",
        2,
        HostedEmbeddingInputSemantics::Shared,
        EmbeddingNormalization::L2,
    )
    .unwrap();
    let runtime = ConfiguredRuntime::hosted(general.clone(), embedding.clone(), identity).unwrap();

    let readiness = runtime.readiness();
    assert!(readiness.contains("general=true, decision=true, embedding=true"));
    assert!(readiness.contains("chronos=general"));
    assert!(readiness.contains("memory_retrieval=general"));
    assert!(readiness.contains("entity_resolution=general"));
    assert!(runtime.probe_embedding_profile().unwrap().is_some());
    runtime.online_probe().unwrap();
    assert_eq!(general.calls.load(Ordering::SeqCst), 2);
    assert_eq!(embedding.calls.load(Ordering::SeqCst), 3);
}

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
