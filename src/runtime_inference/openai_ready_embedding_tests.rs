use crate::openai_ready_embedding_response::decode_response;
use crate::{
    CredentialId, CredentialsConfig, EmbeddingEndpoint, EmbeddingModelEndpoint, ModelProvider,
    ModelSwitchboard, ModelSwitchboardConfig, OpenAiReadyEmbeddingEndpoint, VectorNormalization,
};

#[test]
fn indexed_response_is_reordered_and_l2_normalized() {
    let response = br#"{
        "data": [
            {"index": 1, "embedding": [0.0, 2.0]},
            {"index": 0, "embedding": [3.0, 4.0]}
        ]
    }"#;
    let vectors = decode_response(response, 2, 2, VectorNormalization::L2).unwrap();
    assert!((vectors[0][0] - 0.6).abs() < 1e-6);
    assert!((vectors[0][1] - 0.8).abs() < 1e-6);
    assert!((vectors[1][0] - 0.0).abs() < 1e-6);
    assert!((vectors[1][1] - 1.0).abs() < 1e-6);
}

#[test]
fn response_dimension_mismatch_is_rejected() {
    let response = br#"{"data":[{"index":0,"embedding":[1.0]}]}"#;
    assert!(decode_response(response, 1, 2, VectorNormalization::None).is_err());
}

#[test]
fn live_endpoint_is_constructed_from_validated_switchboard() {
    let id = CredentialId::new("embedding").unwrap();
    let mut credentials = CredentialsConfig::default();
    credentials
        .insert_api_key(id.clone(), "secret-do-not-print")
        .unwrap();
    let config = ModelSwitchboardConfig {
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
            provider: ModelProvider::OpenAiReady,
            model: "qwen/qwen3-embedding-8b".into(),
            url: Some("https://openrouter.ai/api/v1/embeddings".into()),
            credential_id: id,
            dimensions: 1024,
            normalization: VectorNormalization::L2,
        }),
    };
    let switchboard = ModelSwitchboard::new(config, credentials).unwrap();
    let endpoint = OpenAiReadyEmbeddingEndpoint::from_switchboard(&switchboard).unwrap();
    assert_eq!(endpoint.dimensions(), 1024);
    assert_eq!(endpoint.normalization(), VectorNormalization::L2);
}

#[test]
fn zero_batching_limits_are_rejected() {
    let id = CredentialId::new("embedding").unwrap();
    let mut credentials = CredentialsConfig::default();
    credentials.insert_api_key(id.clone(), "secret").unwrap();
    let config = ModelSwitchboardConfig {
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
            provider: ModelProvider::OpenAiReady,
            model: "model".into(),
            url: Some("https://example.invalid/embeddings".into()),
            credential_id: id,
            dimensions: 8,
            normalization: VectorNormalization::None,
        }),
    };
    let switchboard = ModelSwitchboard::new(config, credentials).unwrap();
    let endpoint = OpenAiReadyEmbeddingEndpoint::from_switchboard(&switchboard).unwrap();
    assert!(endpoint.with_batching(0, 16).is_err());
}
