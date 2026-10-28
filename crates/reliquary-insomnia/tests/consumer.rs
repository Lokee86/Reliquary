use reliquary_insomnia::{
    INSOMNIA_EXTRACTOR_CONTRACT_VERSION, MAX_INSOMNIA_CANDIDATES, insomnia_schema,
};

#[test]
fn external_consumer_can_use_the_compiled_contract() {
    assert_eq!(INSOMNIA_EXTRACTOR_CONTRACT_VERSION, "v3-4");
    assert_eq!(MAX_INSOMNIA_CANDIDATES, 64);
    assert!(insomnia_schema().is_object());
}
