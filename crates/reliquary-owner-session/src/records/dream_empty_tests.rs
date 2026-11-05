use super::*;
#[test]
fn settlement_codec_rejects_every_truncation_and_unknown_version() {
    let record = InitialDreamSettlement {
        owner: [1; 16],
        pass_id: "initial".into(),
        source: MemoryId([2; 32]),
        revision: 1,
        body: MemoryBodyId([3; 32]),
        graph_version: 4,
        accepted_turn: 5,
        community_generation: Some(1),
        roots: [MemoryId([4; 32])].into(),
        freshness_effects: vec![vec![1, 2, 3]],
        memory_delta: None,
    };
    let bytes = record.encode().unwrap();
    assert_eq!(
        InitialDreamSettlement::decode(&bytes).unwrap(),
        Some(record)
    );
    for end in 6..bytes.len() {
        assert!(
            InitialDreamSettlement::decode(&bytes[..end]).is_err(),
            "length {end}"
        );
    }
    let mut malformed = bytes.clone();
    malformed[7] = b'2';
    assert!(InitialDreamSettlement::decode(&malformed).is_err());
    let mut trailing = bytes;
    trailing.push(0);
    assert!(InitialDreamSettlement::decode(&trailing).is_err());
}
