use crate::{
    ConfiguredDecisionEndpoint, ConfiguredGeneralEndpoint, CredentialId, CredentialsConfig,
    DecisionEndpoint, DecisionModelEndpoint, EmbeddingModelEndpoint, GeneralEndpoint,
    GeneralModelEndpoint, ModelAuthKind, ModelCapability, ModelProvider, ModelSwitchboard,
    ModelSwitchboardConfig, ReliquaryConfig, VectorNormalization,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-switchboard-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn id(value: &str) -> CredentialId {
    CredentialId::new(value).unwrap()
}

#[test]
fn provider_auth_and_capabilities_are_explicit() {
    assert_eq!(
        ModelProvider::OpenAiCodex.auth_kind(),
        ModelAuthKind::ChatGptDeviceCode
    );
    assert_eq!(
        ModelProvider::OpenAiReady.auth_kind(),
        ModelAuthKind::ApiKey
    );
    assert!(ModelProvider::OpenAiCodex.supports(ModelCapability::General));
    assert!(ModelProvider::OpenAiCodex.supports(ModelCapability::Insomnia));
    assert!(ModelProvider::OpenAiCodex.supports(ModelCapability::EntityExtraction));
    assert!(ModelProvider::OpenAiCodex.supports(ModelCapability::EntityResolution));
    assert!(ModelProvider::OpenAiCodex.supports(ModelCapability::Chronos));
    assert!(ModelProvider::OpenAiCodex.supports(ModelCapability::Dream));
    assert!(!ModelProvider::OpenAiCodex.supports(ModelCapability::Embedding));
    assert!(ModelProvider::OpenAiReady.supports(ModelCapability::Insomnia));
    assert!(ModelProvider::OpenAiReady.supports(ModelCapability::EntityExtraction));
    assert!(ModelProvider::OpenAiReady.supports(ModelCapability::EntityResolution));
    assert!(ModelProvider::OpenAiReady.supports(ModelCapability::Chronos));
    assert!(ModelProvider::OpenAiReady.supports(ModelCapability::Dream));
    assert!(ModelProvider::OpenAiReady.supports(ModelCapability::Embedding));
}

#[test]
fn routes_and_credentials_round_trip_and_attach_auth_headers() {
    let path = test_path("models.cfg");
    let mut config = ReliquaryConfig::new(&path);
    config.models = configured_models();
    config.credentials = configured_credentials();
    config.save().unwrap();

    let reopened = ReliquaryConfig::open(&path).unwrap();
    assert_eq!(reopened.models, config.models);
    assert_eq!(reopened.credentials, config.credentials);

    let switchboard = ModelSwitchboard::new(reopened.models, reopened.credentials).unwrap();
    let mut general = BTreeMap::new();
    switchboard.general_auth().unwrap().apply_to(&mut general);
    assert_eq!(general["Authorization"], "Bearer codex-access");
    assert_eq!(general["ChatGPT-Account-ID"], "account-123");

    let mut insomnia = BTreeMap::new();
    switchboard.insomnia_auth().unwrap().apply_to(&mut insomnia);
    assert_eq!(insomnia["Authorization"], "Bearer ready-key");
    assert!(!insomnia.contains_key("ChatGPT-Account-ID"));

    let mut metadata = BTreeMap::new();
    switchboard
        .insomnia_metadata_auth()
        .unwrap()
        .apply_to(&mut metadata);
    assert_eq!(metadata["Authorization"], "Bearer ready-key");
    assert!(!metadata.contains_key("ChatGPT-Account-ID"));

    let mut entity_extraction = BTreeMap::new();
    switchboard
        .entity_extraction_auth()
        .unwrap()
        .apply_to(&mut entity_extraction);
    assert_eq!(entity_extraction["Authorization"], "Bearer ready-key");
    assert!(!entity_extraction.contains_key("ChatGPT-Account-ID"));

    let mut entity_resolution_decision = BTreeMap::new();
    switchboard
        .entity_resolution_decision_auth()
        .unwrap()
        .apply_to(&mut entity_resolution_decision);
    assert_eq!(
        entity_resolution_decision["Authorization"],
        "Bearer ready-key"
    );
    assert!(!entity_resolution_decision.contains_key("ChatGPT-Account-ID"));

    let mut entity_resolution = BTreeMap::new();
    switchboard
        .entity_resolution_auth()
        .unwrap()
        .apply_to(&mut entity_resolution);
    assert_eq!(entity_resolution["Authorization"], "Bearer ready-key");
    assert!(!entity_resolution.contains_key("ChatGPT-Account-ID"));

    let mut chronos = BTreeMap::new();
    switchboard.chronos_auth().unwrap().apply_to(&mut chronos);
    assert_eq!(chronos["Authorization"], "Bearer ready-key");
    assert!(!chronos.contains_key("ChatGPT-Account-ID"));

    let mut dream = BTreeMap::new();
    switchboard.dream_auth().unwrap().apply_to(&mut dream);
    assert_eq!(dream["Authorization"], "Bearer ready-key");
    assert!(!dream.contains_key("ChatGPT-Account-ID"));

    let mut embedding = BTreeMap::new();
    switchboard
        .embedding_auth()
        .unwrap()
        .apply_to(&mut embedding);
    assert_eq!(embedding["Authorization"], "Bearer ready-key");
    assert!(!embedding.contains_key("ChatGPT-Account-ID"));
}

#[test]
fn clearing_model_routes_removes_them_from_current_config() {
    let path = test_path("clear-models.cfg");
    let mut config = ReliquaryConfig::new(&path);
    config.models = configured_models();
    config.save().unwrap();
    let configured_len = fs::metadata(&path).unwrap().len();
    config.models = ModelSwitchboardConfig::default();
    config.save().unwrap();
    assert!(fs::metadata(&path).unwrap().len() < configured_len);
    assert_eq!(
        ReliquaryConfig::open(&path).unwrap().models,
        ModelSwitchboardConfig::default()
    );
}

#[test]
fn invalid_provider_routes_are_rejected() {
    let codex_embedding = ModelSwitchboardConfig {
        general: None,
        insomnia: None,
        insomnia_metadata: None,
        entity_extraction: None,
        entity_resolution_decision: None,
        memory_retrieval_decision: None,
        entity_resolution: None,
        chronos: None,
        dream: None,
        embedding: Some(EmbeddingModelEndpoint {
            provider: ModelProvider::OpenAiCodex,
            model: "codex".into(),
            url: None,
            credential_id: id("codex"),
            dimensions: 1024,
            normalization: VectorNormalization::L2,
        }),
    };
    assert!(ModelSwitchboard::new(codex_embedding, CredentialsConfig::default()).is_err());

    let missing_url = ModelSwitchboardConfig {
        general: Some(GeneralModelEndpoint {
            provider: ModelProvider::OpenAiReady,
            model: "model".into(),
            url: None,
            credential_id: id("ready"),
            reasoning_effort: None,
        }),
        insomnia: None,
        insomnia_metadata: None,
        entity_extraction: None,
        entity_resolution_decision: None,
        memory_retrieval_decision: None,
        entity_resolution: None,
        chronos: None,
        dream: None,
        embedding: None,
    };
    assert!(ModelSwitchboard::new(missing_url, CredentialsConfig::default()).is_err());
}

#[test]
fn openai_ready_routes_allow_explicit_reasoning_effort() {
    let mut models = configured_models();
    models.insomnia.as_mut().unwrap().reasoning_effort = Some(crate::ModelReasoningEffort::Low);
    models.insomnia_metadata.as_mut().unwrap().reasoning_effort =
        Some(crate::ModelReasoningEffort::Low);
    models.dream.as_mut().unwrap().reasoning_effort = Some(crate::ModelReasoningEffort::Low);
    assert!(ModelSwitchboard::new(models, configured_credentials()).is_ok());
}

#[test]
fn insomnia_route_falls_back_to_general_when_unset() {
    let mut models = configured_models();
    models.insomnia = None;
    let switchboard = ModelSwitchboard::new(models, configured_credentials()).unwrap();
    assert_eq!(switchboard.insomnia(), switchboard.general());

    let mut auth = BTreeMap::new();
    switchboard.insomnia_auth().unwrap().apply_to(&mut auth);
    assert_eq!(auth["Authorization"], "Bearer codex-access");
    assert_eq!(auth["ChatGPT-Account-ID"], "account-123");
}

#[test]
fn insomnia_ownership_prefers_metadata_then_main() {
    let models = configured_models();
    let switchboard = ModelSwitchboard::new(models.clone(), configured_credentials()).unwrap();
    assert_eq!(
        switchboard.insomnia_ownership(),
        switchboard.insomnia_metadata()
    );

    let mut without_metadata = models;
    without_metadata.insomnia_metadata = None;
    let switchboard = ModelSwitchboard::new(without_metadata, configured_credentials()).unwrap();
    assert_eq!(switchboard.insomnia_ownership(), switchboard.insomnia());
}

#[test]
fn entity_routes_are_explicit_and_do_not_fall_back() {
    let mut models = configured_models();
    models.entity_extraction = None;
    models.entity_resolution_decision = None;
    models.entity_resolution = None;
    let switchboard = ModelSwitchboard::new(models, configured_credentials()).unwrap();
    assert!(switchboard.entity_extraction().is_none());
    assert!(switchboard.entity_resolution_decision().is_none());
    assert!(switchboard.entity_resolution().is_none());
}

#[test]
fn configured_entity_endpoints_use_dedicated_routes() {
    let switchboard = ModelSwitchboard::new(configured_models(), configured_credentials()).unwrap();
    let extraction =
        ConfiguredGeneralEndpoint::from_entity_extraction_switchboard(&switchboard).unwrap();
    let decision =
        ConfiguredDecisionEndpoint::from_entity_resolution_decision_switchboard(&switchboard)
            .unwrap();
    let resolution =
        ConfiguredGeneralEndpoint::from_entity_resolution_switchboard(&switchboard).unwrap();
    assert_eq!(extraction.model(), "entity-extraction-model");
    assert_eq!(decision.model(), "typesafe/jev-1.13");
    assert_eq!(resolution.model(), "entity-resolution-model");
}

#[test]
fn chronos_route_is_explicit_and_does_not_fall_back() {
    let mut models = configured_models();
    models.chronos = None;
    let switchboard = ModelSwitchboard::new(models, configured_credentials()).unwrap();

    assert!(switchboard.chronos().is_none());
    assert!(switchboard.chronos_auth().is_none());
    assert!(ConfiguredGeneralEndpoint::from_chronos_switchboard(&switchboard).is_err());
}

#[test]
fn configured_chronos_endpoint_uses_dedicated_route() {
    let switchboard = ModelSwitchboard::new(configured_models(), configured_credentials()).unwrap();
    let endpoint = ConfiguredGeneralEndpoint::from_chronos_switchboard(&switchboard).unwrap();
    assert_eq!(endpoint.model(), "chronos-model");
}

#[test]
fn dream_route_falls_back_to_general_when_unset() {
    let mut models = configured_models();
    models.dream = None;
    let switchboard = ModelSwitchboard::new(models, configured_credentials()).unwrap();
    assert_eq!(switchboard.dream(), switchboard.general());

    let mut auth = BTreeMap::new();
    switchboard.dream_auth().unwrap().apply_to(&mut auth);
    assert_eq!(auth["Authorization"], "Bearer codex-access");
    assert_eq!(auth["ChatGPT-Account-ID"], "account-123");
}

#[test]
fn configured_dream_endpoint_uses_dedicated_route() {
    let switchboard = ModelSwitchboard::new(configured_models(), configured_credentials()).unwrap();
    let endpoint = ConfiguredGeneralEndpoint::from_dream_switchboard(&switchboard).unwrap();
    assert_eq!(endpoint.model(), "dream-model");
}

#[test]
fn legacy_general_schema_decodes_without_reasoning() {
    let mut bytes = vec![ModelProvider::OpenAiCodex.tag(), 0, 0, 0];
    for value in ["legacy-codex", "", "codex"] {
        bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
        bytes.extend_from_slice(value.as_bytes());
    }
    let endpoint = crate::model_switchboard_codec::decode_general_v2(&bytes).unwrap();
    assert_eq!(endpoint.provider, ModelProvider::OpenAiCodex);
    assert_eq!(endpoint.model, "legacy-codex");
    assert_eq!(endpoint.reasoning_effort, None);
}

#[test]
fn missing_or_wrong_credential_kind_is_rejected() {
    let models = configured_models();
    assert!(ModelSwitchboard::new(models.clone(), CredentialsConfig::default()).is_err());

    let mut wrong = CredentialsConfig::default();
    wrong.insert_api_key(id("codex"), "wrong-kind").unwrap();
    wrong.insert_api_key(id("ready"), "ready-key").unwrap();
    assert!(ModelSwitchboard::new(models, wrong).is_err());
}

fn configured_models() -> ModelSwitchboardConfig {
    ModelSwitchboardConfig {
        general: Some(GeneralModelEndpoint {
            provider: ModelProvider::OpenAiCodex,
            model: "gpt-codex".into(),
            url: None,
            credential_id: id("codex"),
            reasoning_effort: Some(crate::ModelReasoningEffort::Low),
        }),
        insomnia: Some(GeneralModelEndpoint {
            provider: ModelProvider::OpenAiReady,
            model: "insomnia-model".into(),
            url: Some("https://example.test/v1/chat/completions".into()),
            credential_id: id("ready"),
            reasoning_effort: None,
        }),
        insomnia_metadata: Some(GeneralModelEndpoint {
            provider: ModelProvider::OpenAiReady,
            model: "metadata-model".into(),
            url: Some("https://example.test/v1/chat/completions".into()),
            credential_id: id("ready"),
            reasoning_effort: None,
        }),
        entity_extraction: Some(GeneralModelEndpoint {
            provider: ModelProvider::OpenAiReady,
            model: "entity-extraction-model".into(),
            url: Some("https://example.test/v1/chat/completions".into()),
            credential_id: id("ready"),
            reasoning_effort: Some(crate::ModelReasoningEffort::Low),
        }),
        entity_resolution_decision: Some(DecisionModelEndpoint {
            model: "typesafe/jev-1.13".into(),
            url: "https://openrouter.ai/api/alpha/decisions".into(),
            credential_id: id("ready"),
        }),
        memory_retrieval_decision: None,
        entity_resolution: Some(GeneralModelEndpoint {
            provider: ModelProvider::OpenAiReady,
            model: "entity-resolution-model".into(),
            url: Some("https://example.test/v1/chat/completions".into()),
            credential_id: id("ready"),
            reasoning_effort: Some(crate::ModelReasoningEffort::Low),
        }),
        chronos: Some(GeneralModelEndpoint {
            provider: ModelProvider::OpenAiReady,
            model: "chronos-model".into(),
            url: Some("https://example.test/v1/chat/completions".into()),
            credential_id: id("ready"),
            reasoning_effort: Some(crate::ModelReasoningEffort::Low),
        }),
        dream: Some(GeneralModelEndpoint {
            provider: ModelProvider::OpenAiReady,
            model: "dream-model".into(),
            url: Some("https://example.test/v1/chat/completions".into()),
            credential_id: id("ready"),
            reasoning_effort: None,
        }),
        embedding: Some(EmbeddingModelEndpoint {
            provider: ModelProvider::OpenAiReady,
            model: "qwen/qwen3-embedding-8b".into(),
            url: Some("https://example.test/v1/embeddings".into()),
            credential_id: id("ready"),
            dimensions: 1024,
            normalization: VectorNormalization::L2,
        }),
    }
}

fn configured_credentials() -> CredentialsConfig {
    let mut credentials = CredentialsConfig::default();
    credentials
        .insert_api_key(id("ready"), "ready-key")
        .unwrap();
    credentials
        .insert_chatgpt_oauth(
            id("codex"),
            "codex-id",
            "codex-access",
            "codex-refresh",
            Some("account-123".into()),
        )
        .unwrap();
    credentials
}
