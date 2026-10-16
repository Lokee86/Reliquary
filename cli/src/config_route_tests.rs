use super::*;
use reliquary_memory::CredentialId;

#[test]
fn memory_retrieval_decision_cli_persists_shows_and_clears_only_its_route() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "reliquary-cli-memory-route-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("test.cfg");
    let credential = CredentialId::new("memory-test").unwrap();
    let mut config = ReliquaryConfig::new(&path);
    config
        .credentials
        .insert_api_key(credential.clone(), "test-only-api-key")
        .unwrap();
    let entity = DecisionModelEndpoint {
        model: "entity-router".into(),
        url: "https://example.test/entity-decisions".into(),
        credential_id: credential,
    };
    config.models.entity_resolution_decision = Some(entity.clone());
    config.save().unwrap();

    run(
        &path,
        ConfigCommand::Model {
            command: ModelCommand::SetMemoryRetrievalDecision {
                model: "memory-router".into(),
                credential: "memory-test".into(),
                url: "https://example.test/memory-decisions".into(),
            },
        },
    )
    .unwrap();
    let configured = ReliquaryConfig::open(&path).unwrap();
    assert_eq!(
        configured
            .models
            .memory_retrieval_decision
            .as_ref()
            .unwrap()
            .model,
        "memory-router"
    );
    assert_eq!(
        configured.models.entity_resolution_decision.as_ref(),
        Some(&entity)
    );
    assert!(configured.models.general.is_none());
    assert!(
        run(
            &path,
            ConfigCommand::Model {
                command: ModelCommand::SetMemoryRetrievalDecision {
                    model: "invalid".into(),
                    credential: "memory-test".into(),
                    url: "https://".into(),
                },
            }
        )
        .is_err()
    );
    assert_eq!(
        ReliquaryConfig::open(&path)
            .unwrap()
            .models
            .memory_retrieval_decision
            .as_ref()
            .unwrap()
            .model,
        "memory-router"
    );
    run(&path, ConfigCommand::Show).unwrap();
    run(
        &path,
        ConfigCommand::Model {
            command: ModelCommand::ClearMemoryRetrievalDecision,
        },
    )
    .unwrap();
    let cleared = ReliquaryConfig::open(&path).unwrap();
    assert!(cleared.models.memory_retrieval_decision.is_none());
    assert_eq!(
        cleared.models.entity_resolution_decision.as_ref(),
        Some(&entity)
    );
    std::fs::remove_dir_all(directory).unwrap();
}
