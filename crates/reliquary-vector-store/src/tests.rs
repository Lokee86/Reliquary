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
        container.sync().unwrap();
    }

    let container = Container::open_read_only(&path).unwrap();
    let packed = PackedVectorStore::open(&container).unwrap();
    let profiles = CompatibilityProfileStore::open(&container).unwrap();
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
    assert_eq!(memories.binding_count(profile_id), 2);
    assert_eq!(generations.current_generation(profile_id), Some(generation));
    assert_eq!(generation.source_archive_version, 12);
    assert_eq!(generations.vector_version(), 1);
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
