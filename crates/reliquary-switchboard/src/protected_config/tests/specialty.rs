use super::*;
use crate::DecisionConsumer::{Chronos, EntityResolution, MemoryRetrieval};

#[test]
fn loads_named_specialties_without_substituting_decision_routes() {
    let keys = mock_master_key_store();
    let oauth = fields(
        &[2, 0, 0, 0],
        &["id-token", ACCESS_TOKEN, "refresh-token", ACCOUNT],
    );
    let mut records = vec![
        (
            "models.insomnia",
            3,
            0,
            fields(&[1, 5, 0, 0], &["private-insomnia", "", "oauth"]),
        ),
        (
            "models.dream",
            2,
            0,
            fields(&[2, 0, 0, 0], &["private-dream", URL, "api"]),
        ),
        (
            "models.entity_resolution",
            3,
            0,
            fields(&[2, 3, 0, 0], &["private-entity", URL, "api"]),
        ),
        (
            "models.entity_resolution_decision",
            1,
            0,
            fields(&[], &["private-entity-decision", URL, "api"]),
        ),
        (
            "models.embedding",
            2,
            0,
            fields(
                &[2, 1, 0, 0, 3, 0, 0, 0],
                &["private-embedding", URL, "api"],
            ),
        ),
        (
            "models.insomnia_metadata",
            2,
            0,
            fields(&[2, 0, 0, 0], &["private-ownership", URL, "api"]),
        ),
        (
            "models.entity_extraction",
            3,
            0,
            fields(&[2, 0, 0, 0], &["private-extraction", URL, "api"]),
        ),
        ("models.chronos", 99, 0, vec![0]),
        (
            "credential.oauth",
            1,
            1,
            encrypted("credential.oauth", 4, oauth),
        ),
        (
            "credential.api",
            1,
            1,
            encrypted("credential.api", 5, fields(&[1, 0, 0, 0], &[API_KEY])),
        ),
    ];
    let fixture = Fixture::new(&records);
    keys.replace(&fixture.config, KEY.to_vec());
    let file = ConfigFile::open(&fixture.config).unwrap();
    let config = routes::decode(&file.document).unwrap();
    assert_eq!(
        config.entity_extraction.unwrap().endpoint.model,
        "private-extraction"
    );
    let insomnia = config.insomnia.unwrap();
    assert_eq!(insomnia.endpoint.provider, ModelProvider::OpenAiCodex);
    assert_eq!(insomnia.reasoning_effort, Some("high"));
    assert_eq!(
        config.entity_resolution.unwrap().reasoning_effort,
        Some("low")
    );

    let standard = ConfiguredRoutes::from_path(&fixture.config).unwrap();
    assert!(standard.general_endpoint().is_none());
    assert_eq!(
        standard.insomnia_endpoint().unwrap().model(),
        "private-insomnia"
    );
    assert_eq!(standard.dream_endpoint().unwrap().model(), "private-dream");
    assert_eq!(
        standard.insomnia_ownership_endpoint().unwrap().model(),
        "private-ownership"
    );
    assert_eq!(
        standard.entity_endpoint().unwrap().model(),
        "private-entity"
    );
    assert!(standard.decision_endpoint(Chronos).is_none());
    assert!(standard.decision_endpoint(MemoryRetrieval).is_none());
    assert_eq!(standard.embedding_endpoint().unwrap().dimensions(), 3);
    assert_eq!(
        standard
            .decision_endpoint(EntityResolution)
            .unwrap()
            .model(),
        "private-entity-decision"
    );
    let report = standard.readiness();
    assert!(report.contains("chronos=unavailable"));
    assert!(report.contains("memory_retrieval=unavailable"));
    assert!(report.contains(
        "general selection: insomnia=insomnia, dream=dream, entity_resolution=entity_resolution"
    ));
    for private in [API_KEY, ACCESS_TOKEN, ACCOUNT, URL, "private-", "oauth"] {
        assert!(!report.contains(private));
    }

    records.extend([
        (
            "models.general_decision",
            1,
            0,
            fields(&[], &["private-shared", URL, "api"]),
        ),
        (
            "models.memory_retrieval_decision",
            1,
            0,
            fields(&[], &["private-retrieval", URL, "api"]),
        ),
    ]);
    let fixture = Fixture::new(&records);
    keys.replace(&fixture.config, KEY.to_vec());
    let routes = ConfiguredRoutes::from_path(&fixture.config).unwrap();
    assert_eq!(
        routes.decision_endpoint(Chronos).unwrap().model(),
        "private-shared"
    );
    assert_eq!(
        routes.decision_endpoint(MemoryRetrieval).unwrap().model(),
        "private-retrieval"
    );
}

#[test]
fn malformed_named_specialty_fails_instead_of_selecting_general() {
    let keys = mock_master_key_store();
    let fixture = Fixture::new(&[
        (
            "models.general",
            2,
            0,
            fields(&[2, 0, 0, 0], &["private-general", URL, "api"]),
        ),
        ("models.dream", 99, 0, vec![0]),
        (
            "credential.api",
            1,
            1,
            encrypted("credential.api", 6, fields(&[1, 0, 0, 0], &[API_KEY])),
        ),
    ]);
    keys.replace(&fixture.config, KEY.to_vec());
    assert!(matches!(
        ConfiguredRoutes::from_path(&fixture.config),
        Err(ConfiguredRuntimeError::Invalid { route: "dream", .. })
    ));
}
