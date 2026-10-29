use reliquary_dream::{
    DREAM_COMMUNITY_NAMING_RETRY_LIMIT, DREAM_COMMUNITY_NAMING_SYSTEM_PROMPT,
    dream_community_naming_schema, parse_community_name,
};
use serde_json::json;

#[test]
fn external_consumer_uses_naming_contract_and_validation() {
    assert_eq!(DREAM_COMMUNITY_NAMING_RETRY_LIMIT, 2);
    assert!(DREAM_COMMUNITY_NAMING_SYSTEM_PROMPT.contains("Do not invent entities"));
    assert_eq!(
        dream_community_naming_schema()["properties"]["name"]["type"],
        "string"
    );
    assert_eq!(
        parse_community_name(&json!({"name": "  Project Atlas  "})).unwrap(),
        "Project Atlas"
    );
    assert!(parse_community_name(&json!({"name": "Bad\u{0007}Name"})).is_err());
}
#[test]
fn representatives_include_two_per_subcentroid() {
    use reliquary_dream::community_representative_ids;
    use reliquary_memory_core::MemoryId;
    use std::collections::HashMap;
    let members: Vec<_> = (1_u8..=10).map(|id| MemoryId([id; 32])).collect();
    let samples = [
        [1.0, 0.0],
        [1.0, 0.0],
        [0.0, 1.0],
        [0.0, 1.0],
        [-1.0, 0.0],
        [-1.0, 0.0],
        [0.0, -1.0],
        [0.0, -1.0],
        [1.0, 1.0],
        [-1.0, -1.0],
    ];
    let vectors: HashMap<_, _> = members
        .iter()
        .copied()
        .zip(samples.into_iter().map(Vec::from))
        .collect();
    let centroids = vec![
        vec![1.0, 0.0],
        vec![0.0, 1.0],
        vec![-1.0, 0.0],
        vec![0.0, -1.0],
    ];
    assert_eq!(
        community_representative_ids(&members, &centroids, &vectors),
        members[..8]
    );
}
