use crate::*;
use lodestone_packed::{PackedVectors, ScalarType, VectorSchema};
use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&base).unwrap();
        let path = base.join(format!(
            "vectors-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn profile() -> CompatibilityProfile {
    CompatibilityProfile {
        id: CompatibilityProfileId([
            0x42, 0xca, 0x77, 0xfc, 0xc4, 0x5a, 0x7e, 0x7e, 0xce, 0x9c, 0x6a, 0xbf, 0x47, 0xdb,
            0x1c, 0xb2, 0xd3, 0xf3, 0x80, 0x35, 0x5b, 0x80, 0x6e, 0x22, 0x85, 0x5f, 0xed, 0xe1,
            0x5a, 0x82, 0x63, 0x70,
        ]),
        dimensions: 1,
        normalization: VectorNormalization::None,
        probe_suite_version: 1,
        compatibility_policy_version: 2,
        references: [
            EmbeddingMode::Query,
            EmbeddingMode::Query,
            EmbeddingMode::Document,
            EmbeddingMode::Document,
        ]
        .into_iter()
        .map(|mode| CompatibilityProbeReference {
            mode,
            vector: vec![1.5],
        })
        .collect(),
    }
}

#[test]
fn pinned_identity_goldens_match_reference() {
    assert!(crate::codec::encode_format().starts_with(b"RELPVFM1"));
    assert!(profile::encode_format().starts_with(b"RELCPFM1"));
    assert!(
        crate::codec::encode_object(
            PackedVectorId([0; 32]),
            &PackedVectors::from_bytes(VectorSchema::new(1, ScalarType::F32).unwrap(), Vec::new())
                .unwrap()
        )
        .unwrap()
        .starts_with(b"RELPVEC1")
    );
    let packed = PackedVectorId([1; 32]);
    let profile_id = CompatibilityProfileId([2; 32]);
    let fragments = [ArchiveFragmentId([3; 32]), ArchiveFragmentId([4; 32])];
    let bodies = [MemoryBodyId([5; 32]), MemoryBodyId([6; 32])];
    assert_eq!(
        archive_vector_id(packed, &fragments).0,
        [
            0x10, 0x5a, 0xee, 0xc5, 0x0a, 0xa1, 0x30, 0xdc, 0x40, 0x57, 0x2a, 0xb8, 0x29, 0x89,
            0x2c, 0x00, 0x2a, 0xc9, 0xa0, 0xe8, 0xd6, 0x9e, 0xf7, 0xa6, 0x55, 0x02, 0x97, 0xb8,
            0x12, 0xa1, 0x63, 0xc2,
        ]
    );
    assert_eq!(
        memory_vector_id(profile_id, packed, &bodies).0,
        [
            0xbf, 0x92, 0xdf, 0x00, 0x13, 0x09, 0x65, 0x13, 0x7d, 0x2c, 0x3b, 0xb0, 0x47, 0xc4,
            0xe7, 0xd1, 0x2b, 0xbb, 0x43, 0xf6, 0xd8, 0x43, 0xf6, 0x2e, 0x7f, 0x62, 0x8b, 0x2f,
            0xe7, 0x32, 0xbb, 0xba,
        ]
    );
    assert_eq!(
        profile::compatibility_profile_id(&profile()).0,
        profile().id.0
    );
}

#[test]
fn f03_reference_identity_vectors_match() {
    fn hex(bytes: [u8; 32]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
    let packed = packed_vector_id(
        VectorSchema::new(3, ScalarType::F32).unwrap(),
        &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
    );
    assert_eq!(
        hex(packed.0),
        "358743b8ccfdeece4cef22af3df6b76da9b5f474a2595b99fe598efcf47a5419"
    );
    let profile = CompatibilityProfile {
        id: CompatibilityProfileId([0; 32]),
        dimensions: 3,
        normalization: VectorNormalization::L2,
        probe_suite_version: 1,
        compatibility_policy_version: 2,
        references: [
            (EmbeddingMode::Query, vec![0.5, -1.25, 2.0]),
            (EmbeddingMode::Query, vec![-0.25, 0.75, 1.5]),
            (EmbeddingMode::Document, vec![1.0, 0.0, -1.0]),
            (EmbeddingMode::Document, vec![0.125, 0.25, 0.5]),
        ]
        .into_iter()
        .map(|(mode, vector)| CompatibilityProbeReference { mode, vector })
        .collect(),
    };
    let profile = profile::compatibility_profile_id(&profile);
    assert_eq!(
        hex(profile.0),
        "fb87924dad756b01c75e8925a8fefe999727124ef2c81f5349c98116f1f2a37a"
    );
    let archive = archive_vector_id(
        PackedVectorId([0x11; 32]),
        &[ArchiveFragmentId([0x22; 32]), ArchiveFragmentId([0x33; 32])],
    );
    assert_eq!(
        hex(archive.0),
        "957cce787993df75646ed70892f9db461c207c67c55605391e49c23361335833"
    );
    let memory = memory_vector_id(
        profile,
        PackedVectorId([0x11; 32]),
        &[MemoryBodyId([0x44; 32]), MemoryBodyId([0x55; 32])],
    );
    assert_eq!(
        hex(memory.0),
        "c592f25bd556512a331f404469e4a31a48c9da08291ca20b92d2a515f791b9ee"
    );
    assert_eq!(
        hex(vector_generation_id(profile, archive, 0x0102030405060708).0),
        "803cbae7840970444914e0be23cf1faaf4bda07744444200dbc96d29703c4c3e"
    );
}

#[test]
fn malformed_profile_count_is_rejected_before_allocation() {
    let mut bytes = profile::encode_profile(&profile()).unwrap();
    bytes[56..60].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        profile::decode_profile(&bytes),
        Err(ProfileStoreError::CorruptRecord("reference count"))
    ));
}

#[test]
fn archive_memory_and_generation_mappings_persist_and_reopen() {
    let temp = TempDir::new();
    let path = temp.0.join("vectors.rel");
    let vectors = PackedVectors::from_bytes(
        VectorSchema::new(1, ScalarType::F32).unwrap(),
        [1.0f32, 2.0f32]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect(),
    )
    .unwrap();
    let packed_id;
    let profile_value = profile();
    let profile_id = profile_value.id;
    let fragments = vec![ArchiveFragmentId([3; 32]), ArchiveFragmentId([4; 32])];
    let bodies = vec![MemoryBodyId([5; 32]), MemoryBodyId([6; 32])];
    let archive_id;
    let memory_id;
    let generation;
    {
        let mut container = Container::create(
            &path,
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            [10; 16],
        )
        .unwrap();
        let mut packed = PackedVectorStore::default();
        packed.initialize(&mut container).unwrap();
        packed_id = packed.put(&mut container, vectors).unwrap();
        let mut profiles = CompatibilityProfileStore::default();
        profiles.initialize(&mut container).unwrap();
        profiles.put(&mut container, profile_value).unwrap();
        let mut archives = ArchiveVectorStore::default();
        archives.initialize(&mut container).unwrap();
        archive_id = archives
            .put(
                &mut container,
                &packed,
                AcceptedArchiveMapping {
                    packed_vector_id: packed_id,
                    packed_rows: 2,
                    fragment_ids: fragments.clone(),
                    max_fragment_archive_version: 9,
                },
            )
            .unwrap();
        let mut memories = MemoryVectorStore::default();
        memories.initialize(&mut container).unwrap();
        memory_id = memories
            .put(
                &mut container,
                &packed,
                &profiles,
                AcceptedMemoryMapping {
                    profile: profile_id,
                    packed: packed_id,
                    packed_rows: 2,
                    body_ids: bodies.clone(),
                },
            )
            .unwrap();
        let mut generations = VectorGenerationStore::default();
        generations.initialize(&mut container).unwrap();
        generation = generations
            .publish(
                &mut container,
                &profiles,
                &archives,
                profile_id,
                archive_id,
                12,
            )
            .unwrap();
        assert_eq!(generation.vector_version, 1);
        profiles.retire(&mut container, profile_id).unwrap();
        let before = container.read_frames().unwrap().len();
        assert!(matches!(
            generations.publish(
                &mut container,
                &profiles,
                &archives,
                profile_id,
                archive_id,
                13
            ),
            Err(GenerationStoreError::MissingProfile)
        ));
        assert_eq!(container.read_frames().unwrap().len(), before);
        container.sync().unwrap();
    }

    let container = Container::open_read_only(&path).unwrap();
    let packed = PackedVectorStore::open(&container).unwrap();
    let profiles = CompatibilityProfileStore::open(&container).unwrap();
    assert!(profiles.is_retired(profile_id));
    let archives = ArchiveVectorStore::open(&container, &packed).unwrap();
    let memories = MemoryVectorStore::open(&container, &packed, &profiles).unwrap();
    let generations = VectorGenerationStore::open(&container, &profiles, &archives).unwrap();
    assert_eq!(archives.get(archive_id).unwrap().fragment_ids, fragments);
    assert_eq!(
        archives
            .info(archive_id)
            .unwrap()
            .max_fragment_archive_version,
        9
    );
    assert_eq!(memories.get(memory_id).unwrap().memory_body_ids, bodies);
    assert_eq!(memories.referenced_body_ids().collect::<Vec<_>>(), bodies);
    assert_eq!(memories.binding_count(profile_id), 2);
    assert_eq!(generations.current_generation(profile_id), Some(generation));
    assert_eq!(generation.source_archive_version, 12);
    assert_eq!(generations.vector_version(), 1);
}

#[test]
fn referenced_memory_body_ids_are_unique_and_stable_after_reopen() {
    let temp = TempDir::new();
    let path = temp.0.join("memory-body-closure.rel");
    let first_profile = profile();
    let mut second_profile = profile();
    second_profile.compatibility_policy_version = 3;
    second_profile.id = profile::compatibility_profile_id(&second_profile);
    assert_ne!(first_profile.id, second_profile.id);
    let body = MemoryBodyId([0x31; 32]);
    let vectors = PackedVectors::from_bytes(
        VectorSchema::new(1, ScalarType::F32).unwrap(),
        1.0f32.to_le_bytes().to_vec(),
    )
    .unwrap();
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [15; 16],
    )
    .unwrap();
    let mut packed = PackedVectorStore::default();
    packed.initialize(&mut container).unwrap();
    let packed_id = packed.put(&mut container, vectors).unwrap();
    let mut memories = MemoryVectorStore::default();
    memories.initialize(&mut container).unwrap();
    let mut profiles = CompatibilityProfileStore::default();
    profiles.insert_rebuilt(first_profile.clone()).unwrap();
    profiles.insert_rebuilt(second_profile.clone()).unwrap();
    for profile_id in [first_profile.id, second_profile.id] {
        memories
            .put(
                &mut container,
                &packed,
                &profiles,
                AcceptedMemoryMapping {
                    profile: profile_id,
                    packed: packed_id,
                    packed_rows: 1,
                    body_ids: vec![body],
                },
            )
            .unwrap();
    }
    container.sync().unwrap();
    drop(container);

    let container = Container::open_read_only(&path).unwrap();
    let packed = PackedVectorStore::open(&container).unwrap();
    let memories = MemoryVectorStore::open(&container, &packed, &profiles).unwrap();
    assert_eq!(memories.referenced_body_ids().collect::<Vec<_>>(), [body]);
}

#[test]
fn mapping_replay_rejects_missing_packed_backing() {
    let temp = TempDir::new();
    let path = temp.0.join("invalid-mapping.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [11; 16],
    )
    .unwrap();
    let packed = PackedVectorStore::default();
    let archives = ArchiveVectorStore::default();
    archives.initialize(&mut container).unwrap();
    let packed_id = PackedVectorId([0xCC; 32]);
    let fragments = [ArchiveFragmentId([1; 32])];
    let archive_id = archive_vector_id(packed_id, &fragments);
    let bytes =
        crate::archive_codec::encode_archive_object(archive_id, packed_id, &fragments, 2).unwrap();
    container.append(&bytes).unwrap();
    container.sync().unwrap();
    drop(container);
    let container = Container::open_read_only(&path).unwrap();
    assert!(matches!(
        ArchiveVectorStore::open(&container, &packed),
        Err(ArchiveVectorError::MissingPackedVector)
    ));
}

#[test]
fn generation_digest_matches_pinned_reference_vector() {
    let payload = GenerationPayload {
        id: VectorGenerationId([1; 32]),
        compatibility_profile_id: CompatibilityProfileId([2; 32]),
        archive_vector_id: ArchiveVectorId([7; 32]),
        source_archive_version: 9,
    };
    assert!(generation::encode_format().starts_with(b"RELVGFM1"));
    assert!(encode_generation(payload).starts_with(b"RELVGEN1"));
    assert_eq!(
        vector_generation_id(CompatibilityProfileId([2; 32]), ArchiveVectorId([7; 32]), 9).0,
        [
            0x97, 0xf5, 0x81, 0x49, 0x42, 0xa0, 0x41, 0x4c, 0x4b, 0x0e, 0x61, 0x3c, 0x38, 0xb0,
            0xff, 0x3d, 0x2b, 0x71, 0xf5, 0x45, 0xba, 0x7a, 0x53, 0x90, 0x06, 0x6d, 0xbf, 0x2e,
            0x38, 0xd0, 0x95, 0xd5,
        ]
    );
}

#[test]
fn memory_mapping_replay_rejects_missing_packed_backing() {
    let temp = TempDir::new();
    let path = temp.0.join("invalid-memory-mapping.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [12; 16],
    )
    .unwrap();
    container.append(&memory_codec::encode_format()).unwrap();
    let profile_id = profile().id;
    let packed_id = PackedVectorId([0xDD; 32]);
    let bodies = [MemoryBodyId([7; 32])];
    let vector_id = memory_vector_id(profile_id, packed_id, &bodies);
    container
        .append(&memory_codec::encode_object(vector_id, profile_id, packed_id, &bodies).unwrap())
        .unwrap();
    container.sync().unwrap();
    drop(container);

    let mut profiles = CompatibilityProfileStore::default();
    profiles.insert_rebuilt(profile()).unwrap();
    let packed = PackedVectorStore::default();
    let container = Container::open_read_only(&path).unwrap();
    assert!(matches!(
        MemoryVectorStore::open(&container, &packed, &profiles),
        Err(MemoryVectorError::MissingPackedVector)
    ));
}

#[test]
fn memory_mapping_replay_preserves_pruned_packed_backing() {
    let temp = TempDir::new();
    let path = temp.0.join("pruned-memory-mapping.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [13; 16],
    )
    .unwrap();
    let packed_id = PackedVectorId([0xDE; 32]);
    let packed_info = PackedVectorInfo {
        id: packed_id,
        schema: VectorSchema::new(1, ScalarType::F32).unwrap(),
        count: 1,
        byte_len: 4,
    };
    container.append(&crate::codec::encode_format()).unwrap();
    container
        .append(&crate::codec::encode_pruned_marker(packed_info, [1; 32]).unwrap())
        .unwrap();
    container.append(&memory_codec::encode_format()).unwrap();
    let profile_id = profile().id;
    let bodies = [MemoryBodyId([8; 32])];
    let vector_id = memory_vector_id(profile_id, packed_id, &bodies);
    container
        .append(&memory_codec::encode_object(vector_id, profile_id, packed_id, &bodies).unwrap())
        .unwrap();
    container.sync().unwrap();
    drop(container);

    let mut profiles = CompatibilityProfileStore::default();
    profiles.insert_rebuilt(profile()).unwrap();
    let mut container = Container::open_read_only(&path).unwrap();
    let packed = PackedVectorStore::open(&container).unwrap();
    let mut memories = MemoryVectorStore::open(&container, &packed, &profiles).unwrap();
    assert_eq!(memories.get(vector_id).unwrap().memory_body_ids, bodies);
    assert_eq!(memories.referenced_body_ids().collect::<Vec<_>>(), bodies);
    assert!(matches!(
        packed.get(&mut container, packed_id),
        Err(PackedVectorError::PayloadUnavailable(
            PayloadUnavailable::Pruned
        ))
    ));
    assert!(matches!(
        memories.put(
            &mut container,
            &packed,
            &profiles,
            AcceptedMemoryMapping {
                profile: profile_id,
                packed: packed_id,
                packed_rows: 1,
                body_ids: vec![MemoryBodyId([9; 32])],
            },
        ),
        Err(MemoryVectorError::PackedVector(
            PackedVectorError::PayloadUnavailable(PayloadUnavailable::Pruned)
        ))
    ));
}

#[test]
fn archive_mapping_replay_preserves_pruned_packed_backing() {
    let temp = TempDir::new();
    let path = temp.0.join("pruned-archive-mapping.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [14; 16],
    )
    .unwrap();
    let packed_id = PackedVectorId([0xDF; 32]);
    let packed_info = PackedVectorInfo {
        id: packed_id,
        schema: VectorSchema::new(1, ScalarType::F32).unwrap(),
        count: 1,
        byte_len: 4,
    };
    container.append(&crate::codec::encode_format()).unwrap();
    container
        .append(&crate::codec::encode_pruned_marker(packed_info, [2; 32]).unwrap())
        .unwrap();
    container
        .append(&crate::archive_codec::encode_archive_format())
        .unwrap();
    let fragments = [ArchiveFragmentId([8; 32])];
    let archive_id = archive_vector_id(packed_id, &fragments);
    container
        .append(
            &crate::archive_codec::encode_archive_object(archive_id, packed_id, &fragments, 1)
                .unwrap(),
        )
        .unwrap();
    container.sync().unwrap();
    drop(container);

    let mut container = Container::open_read_only(&path).unwrap();
    let packed = PackedVectorStore::open(&container).unwrap();
    let mut archives = ArchiveVectorStore::open(&container, &packed).unwrap();
    assert_eq!(archives.get(archive_id).unwrap().fragment_ids, fragments);
    assert!(matches!(
        packed.get(&mut container, packed_id),
        Err(PackedVectorError::PayloadUnavailable(
            PayloadUnavailable::Pruned
        ))
    ));
    assert!(matches!(
        archives.put(
            &mut container,
            &packed,
            AcceptedArchiveMapping {
                packed_vector_id: packed_id,
                packed_rows: 1,
                fragment_ids: vec![ArchiveFragmentId([9; 32])],
                max_fragment_archive_version: 2,
            },
        ),
        Err(ArchiveVectorError::PackedVector(
            PackedVectorError::PayloadUnavailable(PayloadUnavailable::Pruned)
        ))
    ));
}

#[test]
fn packed_payload_and_profile_persist_and_reopen() {
    let temp = TempDir::new();
    let packed_path = temp.0.join("packed.rel");
    let vectors = PackedVectors::from_bytes(
        VectorSchema::new(2, ScalarType::F32).unwrap(),
        [1.0f32, -2.5f32]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect(),
    )
    .unwrap();
    let expected = vectors.clone();
    let id;
    {
        let mut container = Container::create(
            &packed_path,
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            [7; 16],
        )
        .unwrap();
        let mut store = PackedVectorStore::default();
        store.initialize(&mut container).unwrap();
        id = store.put(&mut container, vectors).unwrap();
        container.sync().unwrap();
    }
    let mut container = Container::open_read_only(&packed_path).unwrap();
    let store = PackedVectorStore::open(&container).unwrap();
    assert_eq!(store.get(&mut container, id).unwrap(), expected);
    assert_eq!(store.read_f32_row(id, 0).unwrap(), vec![1.0, -2.5]);
    assert!(store.read_f32_row(id, 1).is_err());

    let profile_path = temp.0.join("profiles.rel");
    let expected_profile = profile();
    {
        let mut container = Container::create(
            &profile_path,
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            [8; 16],
        )
        .unwrap();
        let mut profiles = CompatibilityProfileStore::default();
        profiles.initialize(&mut container).unwrap();
        profiles
            .put(&mut container, expected_profile.clone())
            .unwrap();
        container.sync().unwrap();
    }
    let container = Container::open_read_only(&profile_path).unwrap();
    let profiles = CompatibilityProfileStore::open(&container).unwrap();
    assert_eq!(profiles.get(expected_profile.id), Some(&expected_profile));
}

#[test]
fn open_rejects_packed_payload_with_invalid_identity_backing() {
    let temp = TempDir::new();
    let path = temp.0.join("invalid.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [9; 16],
    )
    .unwrap();
    let store = PackedVectorStore::default();
    store.initialize(&mut container).unwrap();
    let vectors = PackedVectors::from_bytes(
        VectorSchema::new(1, ScalarType::F32).unwrap(),
        1.0f32.to_le_bytes().to_vec(),
    )
    .unwrap();
    let wrong_id = PackedVectorId([0xa5; 32]);
    container
        .append(&crate::codec::encode_object(wrong_id, &vectors).unwrap())
        .unwrap();
    container.sync().unwrap();
    drop(container);
    let container = Container::open_read_only(&path).unwrap();
    assert!(matches!(
        PackedVectorStore::open(&container),
        Err(PackedVectorError::HashCollision)
    ));
}

#[test]
fn rebuilt_packed_vector_requires_consistent_payload_backing() {
    let schema = VectorSchema::new(1, ScalarType::F32).unwrap();
    let bytes = 1.0f32.to_le_bytes().to_vec();
    let chunk = reliquary_container::ObjectRef {
        offset: 12,
        length: 4,
    };
    let valid_id = packed_vector_id(schema, &bytes);
    let mut store = PackedVectorStore::default();

    let wrong_identity = PackedVectorInfo {
        id: PackedVectorId([0xa5; 32]),
        schema,
        count: 1,
        byte_len: 4,
    };
    assert!(matches!(
        store.insert_rebuilt(wrong_identity, chunk, bytes.clone()),
        Err(PackedVectorError::HashCollision)
    ));

    let wrong_metadata = PackedVectorInfo {
        id: valid_id,
        schema,
        count: 2,
        byte_len: 4,
    };
    assert!(matches!(
        store.insert_rebuilt(wrong_metadata, chunk, bytes.clone()),
        Err(PackedVectorError::CorruptRecord(
            "packed-vector backing metadata"
        ))
    ));

    store
        .insert_rebuilt(
            PackedVectorInfo {
                id: valid_id,
                schema,
                count: 1,
                byte_len: 4,
            },
            chunk,
            bytes,
        )
        .unwrap();
    assert_eq!(store.info(valid_id).unwrap().count, 1);
}

#[test]
fn vector_formats_cannot_be_initialized_twice() {
    let temp = TempDir::new();
    let path = temp.0.join("formats.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [17; 16],
    )
    .unwrap();
    let packed = PackedVectorStore::default();
    let profiles = CompatibilityProfileStore::default();
    let archives = ArchiveVectorStore::default();
    let memories = MemoryVectorStore::default();
    let generations = VectorGenerationStore::default();
    packed.initialize(&mut container).unwrap();
    profiles.initialize(&mut container).unwrap();
    archives.initialize(&mut container).unwrap();
    memories.initialize(&mut container).unwrap();
    generations.initialize(&mut container).unwrap();
    let before = container.read_frames().unwrap().len();
    assert!(matches!(
        packed.initialize(&mut container),
        Err(PackedVectorError::ConflictingFormat)
    ));
    assert!(matches!(
        profiles.initialize(&mut container),
        Err(ProfileStoreError::ConflictingFormat)
    ));
    assert!(matches!(
        archives.initialize(&mut container),
        Err(ArchiveVectorError::ConflictingFormat)
    ));
    assert!(matches!(
        memories.initialize(&mut container),
        Err(MemoryVectorError::ConflictingFormat)
    ));
    assert!(matches!(
        generations.initialize(&mut container),
        Err(GenerationStoreError::ConflictingFormat)
    ));
    assert_eq!(container.read_frames().unwrap().len(), before);
    let packed = PackedVectorStore::open(&container).unwrap();
    let profiles = CompatibilityProfileStore::open(&container).unwrap();
    let archives = ArchiveVectorStore::open(&container, &packed).unwrap();
    MemoryVectorStore::open(&container, &packed, &profiles).unwrap();
    VectorGenerationStore::open(&container, &profiles, &archives).unwrap();
}
