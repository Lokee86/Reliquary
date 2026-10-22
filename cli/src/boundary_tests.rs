const PRODUCTION_SOURCES: &[(&str, &str)] = &[
    ("archive_cmd.rs", include_str!("archive_cmd.rs")),
    ("args.rs", include_str!("args.rs")),
    ("config_args.rs", include_str!("config_args.rs")),
    ("config_cmd.rs", include_str!("config_cmd.rs")),
    ("dev_cmd.rs", include_str!("dev_cmd.rs")),
    ("import_cmd.rs", include_str!("import_cmd.rs")),
    ("insomnia_cmd.rs", include_str!("insomnia_cmd.rs")),
    (
        "conversation_titles_cmd.rs",
        include_str!("conversation_titles_cmd.rs"),
    ),
    ("phy_cmd.rs", include_str!("phy_cmd.rs")),
    ("rel_cmd.rs", include_str!("rel_cmd.rs")),
    ("util.rs", include_str!("util.rs")),
    ("vectors_cmd.rs", include_str!("vectors_cmd.rs")),
];

#[test]
fn cli_does_not_own_runtime_composition_or_retrieval_policy() {
    let forbidden = [
        "ConfiguredGeneralEndpoint",
        "InsomniaExtractor",
        "ModelSwitchboard",
        "OpenAiReadyEmbeddingEndpoint",
        "RetrievalConfig",
        "DEFAULT_SEARCH_CANDIDATE_LIMIT",
        "MAX_SEMANTIC_SEARCH_LIMIT",
    ];
    for (path, source) in PRODUCTION_SOURCES {
        for symbol in forbidden {
            assert!(
                !source.contains(symbol),
                "{path} must not own library runtime composition or retrieval policy via {symbol}"
            );
        }
    }
}
