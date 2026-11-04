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
    assert_eq!(decode(&expected).unwrap(), record);
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
    assert_eq!(decode(&expected).unwrap(), record);
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
    assert_eq!(decode(&expected).unwrap(), record);
}

#[test]
fn decoder_round_trips_full_width_fields_and_explicit_population() {
    let compact = BeginRecord {
        owner_uuid: [0; 16],
        pass_id: "λ-pass".into(),
        source_id: MemoryId([0; 32]),
        turn: u64::MAX,
        graph_version_before: u64::MAX,
        population: BeginPopulation::CompactCut {
            memory_cut: u64::MAX,
            digest: [0xFF; 32],
        },
    };
    assert_eq!(decode(&encode(&compact).unwrap()).unwrap(), compact);

    let explicit = BeginRecord {
        owner_uuid: [0xFF; 16],
        pass_id: "manual-例外".into(),
        source_id: MemoryId([0xFF; 32]),
        turn: 0,
        graph_version_before: 0,
        population: BeginPopulation::ExplicitSet(BTreeSet::from([
            MemoryId([0; 32]),
            MemoryId([0x7F; 32]),
        ])),
    };
    assert_eq!(decode(&encode(&explicit).unwrap()).unwrap(), explicit);
}

#[test]
fn every_proper_golden_prefix_is_rejected() {
    for record in [
        base(BeginPopulation::CompactCut {
            memory_cut: 1,
            digest: [0xBB; 32],
        }),
        base(BeginPopulation::ExplicitSet(BTreeSet::from([MemoryId(
            [0x22; 32],
        )]))),
        base(BeginPopulation::ExplicitSet(BTreeSet::new())),
    ] {
        let encoded = encode(&record).unwrap();
        for end in 0..encoded.len() {
            assert!(decode(&encoded[..end]).is_err(), "prefix length {end}");
        }
    }
}

#[test]
fn decoder_rejects_unknown_framing_trailers_and_invalid_pass_id() {
    let compact = encode(&base(BeginPopulation::CompactCut {
        memory_cut: 1,
        digest: [0; 32],
    }))
    .unwrap();
    let mut trailing = compact.clone();
    trailing.push(0);
    assert!(decode(&trailing).is_err());

    let mut wrong_magic = compact.clone();
    wrong_magic[0] ^= 1;
    assert!(decode(&wrong_magic).is_err());
    let mut foreign_magic = compact.clone();
    foreign_magic[..8].copy_from_slice(b"OLDWIRE1");
    assert!(decode(&foreign_magic).is_err());
    let mut unknown_variant = compact.clone();
    unknown_variant[24] = 2;
    assert!(decode(&unknown_variant).is_err());

    let mut empty_pass_id = compact.clone();
    empty_pass_id[25..29].copy_from_slice(&0_u32.to_le_bytes());
    assert!(decode(&empty_pass_id).is_err());
    let mut invalid_utf8 = compact;
    invalid_utf8[29] = 0xFF;
    assert!(decode(&invalid_utf8).is_err());
}

#[test]
fn decoder_rejects_zero_cut_and_noncanonical_explicit_ids() {
    let compact = encode(&base(BeginPopulation::CompactCut {
        memory_cut: 1,
        digest: [0; 32],
    }))
    .unwrap();
    let mut zero_cut = compact;
    zero_cut[78..86].copy_from_slice(&0_u64.to_le_bytes());
    assert!(decode(&zero_cut).is_err());

    let mut explicit_record = base(BeginPopulation::ExplicitSet(BTreeSet::from([
        MemoryId([0x22; 32]),
        MemoryId([0x33; 32]),
    ])));
    explicit_record.pass_id = "ok".into();
    let explicit = encode(&explicit_record).unwrap();

    let mut unsorted = explicit.clone();
    for index in 0..32 {
        unsorted.swap(83 + index, 115 + index);
    }
    assert!(decode(&unsorted).is_err());

    let mut duplicate = explicit.clone();
    duplicate.copy_within(83..115, 115);
    assert!(decode(&duplicate).is_err());

    let mut source_member = explicit;
    source_member[83..115].copy_from_slice(&[0x11; 32]);
    assert!(decode(&source_member).is_err());
}

#[test]
fn forged_u32_max_lengths_fail_before_large_allocation() {
    let mut compact = encode(&base(BeginPopulation::CompactCut {
        memory_cut: 1,
        digest: [0; 32],
    }))
    .unwrap();
    compact[25..29].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode(&compact).is_err());

    let mut explicit = encode(&base(BeginPopulation::ExplicitSet(BTreeSet::new()))).unwrap();
    explicit[78..82].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode(&explicit).is_err());
}

#[test]
fn begin_encoder_rejects_empty_pass_zero_cut_and_source_in_explicit_set() {
    let mut record = base(BeginPopulation::CompactCut {
        memory_cut: 1,
        digest: [0; 32],
    });
    record.pass_id.clear();
    assert_eq!(encode(&record).unwrap_err(), "empty Dream pass ID");

    let record = base(BeginPopulation::CompactCut {
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
