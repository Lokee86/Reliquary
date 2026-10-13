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

fn bounded_endpoint(url: String) -> OpenAiReadyEmbeddingEndpoint {
    let id = CredentialId::new("bounded-fixture").unwrap();
    let mut credentials = CredentialsConfig::default();
    credentials
        .insert_api_key(id.clone(), "local-test")
        .unwrap();
    let models = ModelSwitchboardConfig {
        embedding: Some(EmbeddingModelEndpoint {
            provider: ModelProvider::OpenAiReady,
            model: "fixture".into(),
            url: Some(url),
            credential_id: id,
            dimensions: 2,
            normalization: VectorNormalization::L2,
        }),
        ..Default::default()
    };
    OpenAiReadyEmbeddingEndpoint::from_switchboard(
        &ModelSwitchboard::new(models, credentials).unwrap(),
    )
    .unwrap()
}
#[test]
fn historical_embedding_transport_does_not_follow_redirect_or_retry() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let target = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    target.set_nonblocking(true).unwrap();
    let endpoint = bounded_endpoint(format!("http://{}", listener.local_addr().unwrap()));
    let location = format!("http://{}", target.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut block = [0u8; 8192];
        stream.read(&mut block).unwrap();
        write!(stream, "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
    });
    let error = endpoint
        .embed_bounded(crate::EmbeddingMode::Document, &["source".into()], 4096)
        .unwrap_err();
    assert!(error.to_string().contains("HTTP 302"));
    server.join().unwrap();
    assert_eq!(
        target.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
#[test]
fn historical_embedding_transport_bounds_response_and_bypasses_batch_fanout() {
    use std::io::{Read, Write};
    for oversized in [true, false] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = bounded_endpoint(format!("http://{}", listener.local_addr().unwrap()))
            .with_batching(1, 16)
            .unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let end = loop {
                let mut block = [0u8; 8192];
                let n = stream.read(&mut block).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&block[..n]);
                if let Some(n) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break n + 4;
                }
            };
            let header = String::from_utf8_lossy(&bytes[..end]);
            let len: usize = header
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|n| n.trim().parse().unwrap())
                })
                .unwrap();
            while bytes.len() < end + len {
                let mut block = [0u8; 8192];
                let n = stream.read(&mut block).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&block[..n]);
            }
            let request: serde_json::Value =
                serde_json::from_slice(&bytes[end..end + len]).unwrap();
            assert_eq!(request["input"].as_array().unwrap().len(), 16);
            let body = if oversized {
                vec![b' '; 16384]
            } else {
                serde_json::to_vec(&serde_json::json!({"data":(0..16).map(|index| serde_json::json!({"index":index,"embedding":[1.0,0.0]})).collect::<Vec<_>>()})).unwrap()
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            let _ = stream.write_all(&body);
        });
        let result = endpoint.embed_bounded(
            crate::EmbeddingMode::Document,
            &vec!["source".into(); 16],
            4096,
        );
        if oversized {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("bounded response exceeded")
            );
        } else {
            assert_eq!(result.unwrap().len(), 16);
        }
        server.join().unwrap();
    }
}
