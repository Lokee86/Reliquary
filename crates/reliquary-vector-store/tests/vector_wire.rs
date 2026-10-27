use reliquary_vector_store::{
    ArchiveFragmentId, ArchiveVectorId, CompatibilityProfileId, MemoryBodyId, MemoryVectorId,
    PackedVectorId, VectorGenerationId, archive_codec, archive_vector_id, codec,
    decode_memory_vector_object, generation, memory_vector_id, profile,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn format_markers_have_stable_golden_bytes_for_all_five_families() {
    assert_eq!(codec::encode_format(), b"CVAPVFM1\x01\0\0\0");
    assert_eq!(profile::encode_format(), *b"CVACPFM1\x01\0\0\0");
    assert_eq!(
        archive_codec::encode_archive_format(),
        b"CVAAVFM1\x01\0\0\0"
    );
    assert_eq!(
        reliquary_vector_store::encode_memory_vector_format(),
        b"CVAMVFM1\x01\0\0\0"
    );
    assert_eq!(generation::encode_format(), *b"CVAVGFM2\x01\0\0\0");
}

#[test]
fn deterministic_ids_match_golden_digests() {
    let packed = PackedVectorId([1; 32]);
    let profile_id = CompatibilityProfileId([2; 32]);
    let fragment = [ArchiveFragmentId([3; 32]), ArchiveFragmentId([4; 32])];
    let body = [MemoryBodyId([5; 32]), MemoryBodyId([6; 32])];
    assert_eq!(
        hex(&archive_vector_id(packed, &fragment).0),
        "105aeec50aa130dc40572ab829892c002ac9a0e8d69ef7a6550297b812a163c2"
    );
    assert_eq!(
        hex(&memory_vector_id(profile_id, PackedVectorId([1; 32]), &body).0),
        "bf92df00130965137d2c3bb047c4e7d12bbb43f6d843f62e7f628b2fe732bbba"
    );
    assert_eq!(
        hex(&generation::vector_generation_id(profile_id, ArchiveVectorId([7; 32]), 9).0),
        "97f5814942a0414c4b0e613c38b0ff3d2b71f545ba7a5390066dbf2e38d095d5"
    );
    let profile_value = reliquary_vector_store::CompatibilityProfile {
        id: CompatibilityProfileId([0; 32]),
        dimensions: 1,
        normalization: reliquary_vector_store::VectorNormalization::None,
        probe_suite_version: 1,
        compatibility_policy_version: 2,
        references: [
            reliquary_vector_store::EmbeddingMode::Query,
            reliquary_vector_store::EmbeddingMode::Query,
            reliquary_vector_store::EmbeddingMode::Document,
            reliquary_vector_store::EmbeddingMode::Document,
        ]
        .into_iter()
        .map(|mode| reliquary_vector_store::CompatibilityProbeReference {
            mode,
            vector: vec![1.5],
        })
        .collect(),
    };
    assert_eq!(
        hex(&profile::compatibility_profile_id(&profile_value).0),
        "42ca77fcc45a7e7ece9c6abf47db1cb2d3f380355b806e22855fede15a826370"
    );
}

#[test]
fn archive_wire_roundtrip_and_truncation_are_checked() {
    let id = ArchiveVectorId([1; 32]);
    let packed = PackedVectorId([2; 32]);
    let fragments = [ArchiveFragmentId([3; 32]), ArchiveFragmentId([4; 32])];
    let mut golden = b"CVAAVEC1".to_vec();
    golden.extend_from_slice(&id.0);
    golden.extend_from_slice(&packed.0);
    golden.extend_from_slice(&2_u64.to_le_bytes());
    golden.extend_from_slice(&fragments[0].0);
    golden.extend_from_slice(&fragments[1].0);
    let decoded = archive_codec::decode_archive_object(&golden)
        .unwrap()
        .unwrap();
    assert_eq!(decoded.id, id);
    assert_eq!(decoded.packed_vector_id, packed);
    assert_eq!(decoded.fragment_ids, fragments);
    assert!(archive_codec::decode_archive_object(&golden[..golden.len() - 1]).is_err());
    assert!(archive_codec::decode_archive_object(&golden[..79]).is_err());
}

#[test]
fn memory_wire_golden_ids_and_bad_lengths_are_checked() {
    let id = MemoryVectorId([1; 32]);
    let profile = CompatibilityProfileId([2; 32]);
    let packed = PackedVectorId([3; 32]);
    let bodies = [MemoryBodyId([4; 32]), MemoryBodyId([5; 32])];
    let mut golden = b"CVAMVEC1".to_vec();
    golden.extend_from_slice(&id.0);
    golden.extend_from_slice(&profile.0);
    golden.extend_from_slice(&packed.0);
    golden.extend_from_slice(&2_u64.to_le_bytes());
    golden.extend_from_slice(&bodies[0].0);
    golden.extend_from_slice(&bodies[1].0);
    let decoded = decode_memory_vector_object(&golden).unwrap().unwrap();
    assert_eq!(decoded.id, id);
    assert_eq!(decoded.compatibility_profile_id, profile);
    assert_eq!(decoded.packed_vector_id, packed);
    assert_eq!(decoded.memory_body_ids, bodies);
    assert!(decode_memory_vector_object(&golden[..golden.len() - 1]).is_err());
    assert!(decode_memory_vector_object(&golden[..111]).is_err());
}

#[test]
fn packed_object_rejects_truncation_and_header_corruption() {
    let mut golden = b"CVAPVEC1".to_vec();
    golden.extend_from_slice(&[1; 32]);
    golden.extend_from_slice(&1_u32.to_le_bytes());
    golden.push(11); // F32
    golden.extend_from_slice(&[0; 3]);
    golden.extend_from_slice(&1_u64.to_le_bytes());
    golden.extend_from_slice(&4_u64.to_le_bytes());
    golden.extend_from_slice(&[0, 0, 128, 63]); // 1.0f32
    let decoded = codec::decode_object(&golden).unwrap().unwrap();
    assert_eq!(decoded.id, PackedVectorId([1; 32]));
    assert_eq!(decoded.count, 1);
    assert_eq!(decoded.bytes, &[0, 0, 128, 63]);
    assert!(codec::decode_object(&golden[..golden.len() - 1]).is_err());
    let mut reserved = golden.clone();
    reserved[45] = 1;
    assert!(codec::decode_object(&reserved).is_err());
}

#[test]
fn profile_record_roundtrip_and_truncation_are_checked() {
    use reliquary_vector_store::{
        CompatibilityProbeReference, CompatibilityProfile, EmbeddingMode, VectorNormalization,
    };
    let mut value = CompatibilityProfile {
        id: CompatibilityProfileId([9; 32]),
        dimensions: 1,
        normalization: VectorNormalization::None,
        probe_suite_version: 1,
        compatibility_policy_version: 2,
        references: vec![CompatibilityProbeReference {
            mode: EmbeddingMode::Query,
            vector: vec![1.5],
        }],
    };
    let bytes = profile::encode_profile(&value).unwrap();
    assert_eq!(&bytes[..8], b"CVACPRO1");
    assert_eq!(profile::decode_profile(&bytes).unwrap().unwrap(), value);
    assert!(profile::decode_profile(&bytes[..bytes.len() - 1]).is_err());
    let mut corrupt = bytes.clone();
    corrupt[45] = 1;
    assert!(profile::decode_profile(&corrupt).is_err());
    value.id = CompatibilityProfileId([0; 32]);
}

#[test]
fn generation_payload_and_version_have_golden_bytes_and_reject_truncation() {
    let payload = generation::GenerationPayload {
        id: VectorGenerationId([1; 32]),
        compatibility_profile_id: CompatibilityProfileId([2; 32]),
        archive_vector_id: ArchiveVectorId([3; 32]),
        source_archive_version: 0x0807_0605_0403_0201,
    };
    let bytes = generation::encode_generation(payload);
    assert_eq!(&bytes[..8], b"CVAVGEN2");
    assert_eq!(&bytes[104..], &[1, 2, 3, 4, 5, 6, 7, 8]);
    let decoded = generation::decode_generation(&bytes).unwrap().unwrap();
    assert_eq!(decoded.id, payload.id);
    assert_eq!(
        decoded.compatibility_profile_id,
        payload.compatibility_profile_id
    );
    assert_eq!(decoded.archive_vector_id, payload.archive_vector_id);
    assert_eq!(
        decoded.source_archive_version,
        payload.source_archive_version
    );
    assert!(generation::decode_generation(&bytes[..111]).is_err());
}
