use crate::archive_history_codec::encode_archive_format;
use crate::compatibility_profile_codec::encode_format as encode_profile_format;
use crate::insomnia::codec::encode_format as encode_insomnia_format;
use crate::memory_codec::encode_format as encode_memory_format;
use crate::memory_vector_codec::encode_format as encode_memory_vector_format;
use crate::packed_vector_codec::encode_format as encode_packed_format;
use crate::{
    ArchiveVectorError, Cva, CvaError, FragmentConfig, FragmentId, PackedVectorId, PackedVectors,
    ScalarType, VectorSchema,
};
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-archive-vectors-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn fragments(cva: &mut Cva, turns: usize) -> Vec<FragmentId> {
    for index in 0..turns {
        cva.append_node(
            format!("n{index}"),
            "c1".into(),
            (index > 0).then(|| format!("n{}", index - 1)),
            if index % 2 == 0 { "user" } else { "assistant" }.into(),
            index as i64,
            &format!("turn {index}"),
        )
        .unwrap();
    }
    cva.materialize_path_fragments(
        "c1",
        &format!("n{}", turns - 1),
        FragmentConfig::default(),
        false,
    )
    .unwrap()
    .into_iter()
    .map(|fragment| fragment.id)
    .collect()
}

fn packed(cva: &mut Cva, rows: usize) -> PackedVectorId {
    let schema = VectorSchema::new(4, ScalarType::I8).unwrap();
    let matrix = PackedVectors::from_bytes(schema, vec![1; rows * 4]).unwrap();
    cva.put_packed_vectors(matrix).unwrap()
}

#[test]
fn archive_vectors_bind_rows_to_fragments_and_round_trip() {
    let path = test_path("roundtrip.cva");
    let mut cva = Cva::create(&path).unwrap();
    let fragment_ids = fragments(&mut cva, 14);
    assert_eq!(fragment_ids.len(), 2);
    let packed_id = packed(&mut cva, fragment_ids.len());
    let id = cva
        .put_archive_vectors(packed_id, fragment_ids.clone())
        .unwrap();
    assert_eq!(cva.archive_vector_stats().objects, 1);
    assert_eq!(cva.archive_vector_stats().rows, 2);
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(path).unwrap();
    let set = reopened.archive_vectors(id).unwrap();
    assert_eq!(set.packed_vector_id, packed_id);
    assert_eq!(set.fragment_ids, fragment_ids);
}

#[test]
fn identical_archive_vector_binding_is_content_addressed_once() {
    let path = test_path("dedupe.cva");
    let mut cva = Cva::create(path).unwrap();
    let fragment_ids = fragments(&mut cva, 8);
    let packed_id = packed(&mut cva, 1);
    let first = cva
        .put_archive_vectors(packed_id, fragment_ids.clone())
        .unwrap();
    let second = cva.put_archive_vectors(packed_id, fragment_ids).unwrap();
    assert_eq!(first, second);
    assert_eq!(cva.archive_vector_stats().objects, 1);
}

#[test]
fn binding_requires_existing_matrix_and_exact_row_count() {
    let path = test_path("references.cva");
    let mut cva = Cva::create(path).unwrap();
    let fragment_ids = fragments(&mut cva, 8);
    let missing = PackedVectorId([9; 32]);
    assert!(matches!(
        cva.put_archive_vectors(missing, fragment_ids.clone()),
        Err(ArchiveVectorError::MissingPackedVector)
    ));

    let packed_id = packed(&mut cva, 2);
    assert!(matches!(
        cva.put_archive_vectors(packed_id, fragment_ids),
        Err(ArchiveVectorError::RowCountMismatch)
    ));
}

#[test]
fn binding_requires_real_unique_fragments() {
    let path = test_path("fragments.cva");
    let mut cva = Cva::create(path).unwrap();
    let fragment_id = fragments(&mut cva, 8)[0];
    let packed_id = packed(&mut cva, 1);
    assert!(matches!(
        cva.put_archive_vectors(packed_id, vec![FragmentId([7; 32])]),
        Err(ArchiveVectorError::MissingFragment)
    ));

    let packed_id = packed(&mut cva, 2);
    assert!(matches!(
        cva.put_archive_vectors(packed_id, vec![fragment_id, fragment_id]),
        Err(ArchiveVectorError::DuplicateFragment)
    ));
}

#[test]
fn archive_vector_owner_accepts_typed_binding_while_composition_rejects_missing_fragment_atomically()
 {
    let path = test_path("owner-local-binding.cva");
    let mut cva = Cva::create(&path).unwrap();
    let packed_id = packed(&mut cva, 1);
    let missing_fragment = FragmentId([0xf2; 32]);
    let archive_version = cva.archive_version();

    let id = cva
        .owner
        .archive_vectors
        .put(
            &mut cva.owner.container,
            &cva.owner.packed_vectors,
            archive_version,
            packed_id,
            vec![missing_fragment],
        )
        .unwrap();
    assert_eq!(cva.archive_vectors.stats().objects, 1);
    assert!(
        cva.archive_vectors
            .has_exact_fragment_coverage(id, &[missing_fragment])
    );
    let accepted_version = cva.container.latest_version();

    assert!(matches!(
        cva.put_archive_vectors(packed_id, vec![missing_fragment]),
        Err(ArchiveVectorError::MissingFragment)
    ));
    assert_eq!(cva.container.latest_version(), accepted_version);
    assert_eq!(cva.archive_vectors.stats().objects, 1);
}

#[test]
fn archive_vector_objects_do_not_advance_semantic_clocks() {
    let path = test_path("clocks.cva");
    let mut cva = Cva::create(path).unwrap();
    let fragment_ids = fragments(&mut cva, 8);
    let packed_id = packed(&mut cva, 1);
    let global_before = cva.container.latest_version();
    let archive_before = cva.archive_version();
    cva.put_archive_vectors(packed_id, fragment_ids).unwrap();
    assert_eq!(cva.container.latest_version(), global_before);
    assert_eq!(cva.archive_version(), archive_before);
}

#[test]
fn archive_vector_store_requires_its_format_marker() {
    let path = test_path("missing-format.cva");
    let mut container = crate::Container::create_with_identity(
        &path,
        crate::ContainerIdentity {
            file_kind: crate::FileKind::Reliquary,
            scope: Some(crate::ReliquaryScopeKind::Project),
        },
    )
    .unwrap();
    container.append(&encode_archive_format()).unwrap();
    container.append(&encode_memory_format()).unwrap();
    container.append(&encode_insomnia_format()).unwrap();
    container.append(&encode_packed_format()).unwrap();
    container.append(&encode_memory_vector_format()).unwrap();
    container.append(&encode_profile_format()).unwrap();
    container.sync().unwrap();
    drop(container);
    assert!(matches!(
        Cva::open(path),
        Err(CvaError::ArchiveVectors(ArchiveVectorError::MissingFormat))
    ));
}

#[test]
fn corrupt_archive_vector_mapping_is_rejected_on_reopen() {
    let path = test_path("corrupt.cva");
    let mut cva = Cva::create(&path).unwrap();
    let fragment_ids = fragments(&mut cva, 8);
    let packed_id = packed(&mut cva, 1);
    cva.put_archive_vectors(packed_id, fragment_ids).unwrap();
    cva.sync().unwrap();
    drop(cva);

    let mut bytes = fs::read(&path).unwrap();
    let object = bytes
        .windows(8)
        .position(|window| window == b"CVAAVEC1")
        .unwrap();
    bytes[object + 80] ^= 0xff;
    fs::write(&path, bytes).unwrap();
    assert!(matches!(
        Cva::open(path),
        Err(CvaError::ArchiveVectors(ArchiveVectorError::HashCollision))
    ));
}
