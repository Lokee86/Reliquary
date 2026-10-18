//! Native importer -> settled generation -> configured bounded fill -> checkpoint.
use super::*;
use crate::imports::*;
use crate::{
    CredentialId, EmbeddingModelEndpoint, InsomniaExtractor, ModelProvider,
    SimulatedGeneralEndpoint, VectorNormalization,
};
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[test]
fn historical_embedding_native_import_configured_runtime_and_intrinsic_live_f8() {
    let path = std::env::temp_dir().join(format!("h8-import-{}.rel", uuid::Uuid::new_v4()));
    let mut cva = Cva::create(&path).unwrap();
    let mut messages = Vec::new();
    let mut parent = None;
    for index in 0..8 {
        for (role, prefix) in [("user", "u"), ("assistant", "a")] {
            let id = format!("{prefix}{index}");
            messages.push(ImportMessage {
                id: id.clone(),
                revision: Some("1".into()),
                parent_id: parent.clone(),
                replaces: None,
                deleted: false,
                role: role.into(),
                timestamp_ns: 10 + messages.len() as i64,
                content: format!("Genuine imported fact {index}."),
                artifacts: vec![],
            });
            parent = Some(id);
        }
    }
    let incoming = NativeImport {
        schema_version: 1,
        source_namespace: "h8/account".into(),
        source_format: "structured-agent-v1".into(),
        conversations: vec![ImportConversation {
            id: "session".into(),
            title: None,
            messages,
            branches: vec![ImportBranch {
                id: "main".into(),
                leaf_message_id: "a7".into(),
                canonical: true,
            }],
        }],
    };
    let receipt = cva.import_native(&incoming, 100).unwrap();
    assert_eq!(cva.rel_turn_count(), 16);
    let episode = cva.episodes().remove(0);
    let turns = cva.episode_turns(episode.id).unwrap();
    let users: Vec<_> = turns.iter().filter(|t| t.role == "user").collect();
    let clauses: serde_json::Map<_, _> = users.iter().enumerate().map(|(index, turn)| {
        (turn.node_id.clone(), json!([if index == 0 { json!({
            "disposition":"retain", "authority_kind":"direct", "category":"decision",
            "type":"project", "lifecycle":"current", "proposition":"Keep the native import invariant.",
            "authority_source_node_id":"", "grounding_source_node_id":"", "reason":"fixture"
        }) } else { json!({
            "disposition":"omit", "authority_kind":"none", "category":"none", "type":"none",
            "lifecycle":"none", "proposition":"", "authority_source_node_id":"",
            "grounding_source_node_id":"", "reason":"fixture omission"
        }) }]))
    }).collect();
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "h8-import",
        vec![
            json!({"turns":clauses,"evidence_requests":[]}),
            json!({"groups":{"g000":{"title":"Native import invariant","content":"Keep the native import invariant."}}}),
        ],
    ));
    let cancel = AtomicBool::new(false);
    let (_id, _endpoint) = cva.with_import_write_scope(|cva| {
        let claim = cva
            .claim_insomnia_episode("worker", 110, 1000)
            .unwrap()
            .unwrap();
        let generated = cva
            .process_claimed_insomnia_episode(&claim, &extractor, "private", 110, 111)
            .unwrap();
        let id = generated.created[0].id;
        let pass = crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), id);
        cva.begin_initial_dream_pass(
            pass.clone(),
            id,
            1,
            cva.rel_turn_count(),
            Default::default(),
            cva.graph_version(),
        )
        .unwrap();
        cva.reconcile_dream_lifecycle(id).unwrap();
        cva.accept_initial_dream_settlement(&pass, cva.rel_turn_count())
            .unwrap();
        crate::dream_freshness::settle_initial_pass(cva, id, true, 300).unwrap();
        let endpoint = super::historical_embedding::Endpoint::new(&cancel);
        cva.establish_compatibility_profile(&endpoint).unwrap();
        cva.materialize_path_fragments(
            &episode.conversation_id,
            &episode.end_node_id,
            crate::FragmentConfig {
                turns: 2,
                overlap: 0,
            },
            true,
        )
        .unwrap();
        (id, endpoint)
    });

    // Real configured HTTP transport; no external provider/account is used.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/embeddings", listener.local_addr().unwrap());
    let observed = Arc::new(Mutex::new(Vec::new()));
    let capture = observed.clone();
    let server = std::thread::spawn(move || {
        for _ in 0..3 {
            let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
            let mut stream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            std::time::Instant::now() < until,
                            "missing bounded HTTP request"
                        );
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            // Accepted sockets can inherit nonblocking mode on Windows.
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(30)))
                .unwrap();
            let mut bytes = Vec::new();
            let end = loop {
                let mut block = [0u8; 4096];
                let n = stream.read(&mut block).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&block[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break end + 4;
                }
                assert!(bytes.len() <= 256 * 1024);
            };
            let header = String::from_utf8_lossy(&bytes[..end]);
            let length: usize = header
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|n| n.trim().parse().unwrap())
                })
                .unwrap();
            assert!(length <= 256 * 1024);
            while bytes.len() < end + length {
                let mut block = [0u8; 4096];
                let n = stream.read(&mut block).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&block[..n]);
            }
            let request: serde_json::Value =
                serde_json::from_slice(&bytes[end..end + length]).unwrap();
            let count = request["input"].as_array().unwrap().len();
            assert!(count <= 16);
            capture.lock().unwrap().push(count);
            let body = serde_json::to_vec(&json!({"data":(0..count).map(|index| json!({"index":index,"embedding":[1.0,0.0]})).collect::<Vec<_>>()})).unwrap();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            stream.write_all(&body).unwrap();
        }
    });
    let config_path = path.with_extension("config");
    let mut config = crate::ReliquaryConfig::new(&config_path);
    let credential = CredentialId::new("h8-fixture").unwrap();
    config
        .credentials
        .insert_api_key(credential.clone(), "local-fixture")
        .unwrap();
    config.models.embedding = Some(EmbeddingModelEndpoint {
        provider: ModelProvider::OpenAiReady,
        model: "fixture".into(),
        url: Some(url),
        credential_id: credential,
        dimensions: 2,
        normalization: VectorNormalization::L2,
    });
    config.save().unwrap();
    let runtime = crate::ConfiguredRuntime::open(&config_path).unwrap();
    let report = runtime
        .reconstruct_historical_freshness(
            &mut cva,
            HistoricalReconstructionOptions {
                fill_missing_embeddings: true,
            },
            Default::default(),
            &cancel,
        )
        .unwrap();
    server.join().unwrap();
    assert_eq!(*observed.lock().unwrap(), [2, 2, 8]);
    assert_eq!(report.embedding.as_ref().unwrap().unavailable_fragments, 0);
    assert!(report.committed);
    assert!(
        cva.active_archive_compatibility_profiles().is_empty(),
        "open import historical fill cannot activate live vectors"
    );
    assert_eq!(receipt, cva.import_native(&incoming, 400).unwrap());
    drop(cva);
    let mut cva = Cva::open(&path).unwrap();
    let bytes = fs::read(&path).unwrap();
    assert!(
        !runtime
            .reconstruct_historical_freshness(
                &mut cva,
                HistoricalReconstructionOptions {
                    fill_missing_embeddings: true
                },
                Default::default(),
                &cancel
            )
            .unwrap()
            .committed
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(cva.semantic_freshness_pending_fragments().unwrap(), 0);
}
