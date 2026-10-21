use super::*;

fn from_hex(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |byte| match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => panic!("invalid test hex"),
            };
            digit(pair[0]) * 16 + digit(pair[1])
        })
        .collect()
}

fn base(population: BeginPopulation) -> BeginRecord {
    BeginRecord {
        owner_uuid: [0xAA; 16],
        pass_id: "p".into(),
        source_id: MemoryId([0x11; 32]),
        turn: 2,
        graph_version_before: 3,
        population,
    }
}

#[test]
fn compact_cut_encoder_matches_independent_golden_bytes() {
    let record = base(BeginPopulation::CompactCut {
        memory_cut: 4,
        digest: [0xBB; 32],
    });
    let expected = from_hex(concat!(
        "4356445245414d32",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "00",
        "01000000",
        "70",
        "1111111111111111111111111111111111111111111111111111111111111111",
        "0200000000000000",
        "0300000000000000",
        "0400000000000000",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    ));
    assert_eq!(expected.len(), 118);
    assert_eq!(encode(&record).unwrap(), expected);
}

#[test]
fn explicit_nonempty_set_encoder_matches_independent_golden_bytes() {
    let mut record = base(BeginPopulation::ExplicitSet(BTreeSet::from([
        MemoryId([0x33; 32]),
        MemoryId([0x22; 32]),
    ])));
    record.pass_id = "ok".into();
    let expected = from_hex(concat!(
        "4356445245414d32",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "01",
        "02000000",
        "6f6b",
        "1111111111111111111111111111111111111111111111111111111111111111",
        "0200000000000000",
        "0300000000000000",
        "02000000",
        "2222222222222222222222222222222222222222222222222222222222222222",
        "3333333333333333333333333333333333333333333333333333333333333333"
    ));
    assert_eq!(expected.len(), 147);
    assert_eq!(encode(&record).unwrap(), expected);
}

#[test]
fn explicit_empty_set_encoder_matches_independent_golden_bytes() {
    let mut record = base(BeginPopulation::ExplicitSet(BTreeSet::new()));
    record.pass_id = "e".into();
    let expected = from_hex(concat!(
        "4356445245414d32",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "01",
        "01000000",
        "65",
        "1111111111111111111111111111111111111111111111111111111111111111",
        "0200000000000000",
        "0300000000000000",
        "00000000"
    ));
    assert_eq!(expected.len(), 82);
    assert_eq!(encode(&record).unwrap(), expected);
}

#[test]
fn begin_encoder_rejects_empty_pass_zero_cut_and_source_in_explicit_set() {
    let mut record = base(BeginPopulation::CompactCut {
        memory_cut: 1,
        digest: [0; 32],
    });
    record.pass_id.clear();
    assert_eq!(encode(&record).unwrap_err(), "empty Dream pass ID");

    let mut record = base(BeginPopulation::CompactCut {
        memory_cut: 0,
        digest: [0; 32],
    });
    assert_eq!(encode(&record).unwrap_err(), "zero Dream Memory prefix cut");

    let record = base(BeginPopulation::ExplicitSet(BTreeSet::from([MemoryId(
        [0x11; 32],
    )])));
    assert_eq!(
        encode(&record).unwrap_err(),
        "Dream pass source cannot be preexisting"
    );
}
