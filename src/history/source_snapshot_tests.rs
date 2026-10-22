use crate::container::ContainerError;
use crate::owner_lifecycle::HistorySource;
use crate::vector_generation_codec::{
    GenerationPayload, GenerationVersion, encode_generation, encode_version,
};
use crate::{
    ArchiveVectorId, CompatibilityProfileId, Container, ObjectRef, VectorGenerationId,
    graph_store::GraphStore,
};
use std::fs;
use std::path::PathBuf;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("history-source-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn append_vector_generation(container: &mut Container, leading_chunks: &[&[u8]]) -> ObjectRef {
    for chunk in leading_chunks {
        container.append(chunk).unwrap();
    }

    let record = container
        .append(&encode_generation(GenerationPayload {
            id: VectorGenerationId([1; 32]),
            compatibility_profile_id: CompatibilityProfileId([2; 32]),
            archive_vector_id: ArchiveVectorId([3; 32]),
            source_archive_version: 7,
        }))
        .unwrap();
    container.set_next_transaction_time_override(Some(123));
    let global_version = container.allocate_version().unwrap();
    assert_eq!(global_version, 1);
    container
        .append(&encode_version(GenerationVersion {
            global_version,
            vector_version: 1,
            record,
        }))
        .unwrap();
    record
}

#[test]
fn vector_generation_digest_ignores_relocated_record_and_reads_are_immutable() {
    let scratch = Scratch::new();
    let first_path = scratch.file("first.cva");
    let second_path = scratch.file("second.cva");
    let mut first = Container::create(&first_path).unwrap();
    let mut second = Container::create(&second_path).unwrap();
    let first_record = append_vector_generation(&mut first, &[b"unreferenced-prefix"]);
    let second_record = append_vector_generation(
        &mut second,
        &[b"a different unreferenced prefix", b"another ignored frame"],
    );
    assert_ne!(first_record, second_record);
    first.sync().unwrap();
    second.sync().unwrap();

    let first_modified = fs::metadata(&first_path).unwrap().modified().unwrap();
    let second_modified = fs::metadata(&second_path).unwrap().modified().unwrap();
    let first_bytes = fs::read(&first_path).unwrap();
    let second_bytes = fs::read(&second_path).unwrap();
    let first_head = first.latest_version();
    let second_head = second.latest_version();
    let first_time = first.transaction_time_ns(1);
    let second_time = second.transaction_time_ns(1);
    assert_eq!((first_head, second_head), (1, 1));
    assert_eq!((first_time, second_time), (Some(123), Some(123)));

    let graph = GraphStore::empty();
    let first_digest = {
        let source = HistorySource::new(&first).unwrap();
        let initial = crate::history::owner_history_digest(&source, 1, &graph).unwrap();
        assert_eq!(
            crate::history::owner_history_digest(&source, 1, &graph).unwrap(),
            initial
        );
        initial
    };
    let second_digest = {
        let source = HistorySource::new(&second).unwrap();
        let initial = crate::history::owner_history_digest(&source, 1, &graph).unwrap();
        assert_eq!(
            crate::history::owner_history_digest(&source, 1, &graph).unwrap(),
            initial
        );
        initial
    };
    assert_eq!(first_digest, second_digest);
    assert_eq!(fs::read(&first_path).unwrap(), first_bytes);
    assert_eq!(fs::read(&second_path).unwrap(), second_bytes);
    assert_eq!(
        fs::metadata(&first_path).unwrap().modified().unwrap(),
        first_modified
    );
    assert_eq!(
        fs::metadata(&second_path).unwrap().modified().unwrap(),
        second_modified
    );
    assert_eq!(first.latest_version(), first_head);
    assert_eq!(second.latest_version(), second_head);
    assert_eq!(first.transaction_time_ns(1), first_time);
    assert_eq!(second.transaction_time_ns(1), second_time);

    first.set_next_transaction_time_override(Some(456));
    second.set_next_transaction_time_override(Some(789));
    assert_eq!(first.allocate_version().unwrap(), 2);
    assert_eq!(second.allocate_version().unwrap(), 2);
    assert_eq!(first.transaction_time_ns(2), Some(456));
    assert_eq!(second.transaction_time_ns(2), Some(789));
}

#[test]
fn vector_generation_reference_must_name_a_physical_frame_and_digest_is_read_only() {
    let scratch = Scratch::new();
    let path = scratch.file("missing-reference.cva");
    let mut container = Container::create(&path).unwrap();
    container.set_next_transaction_time_override(Some(123));
    let global_version = container.allocate_version().unwrap();
    container
        .append(&encode_version(GenerationVersion {
            global_version,
            vector_version: 1,
            record: ObjectRef::from_legacy_bytes([0; 16]),
        }))
        .unwrap();
    container.sync().unwrap();
    let bytes_before = fs::read(&path).unwrap();
    let graph = GraphStore::empty();

    let source = HistorySource::new(&container).unwrap();
    let error = crate::history::owner_history_digest(&source, 1, &graph).unwrap_err();
    assert_eq!(
        error,
        "Vector generation record points to a missing or not-yet-copied physical object"
    );
    drop(source);
    assert_eq!(fs::read(&path).unwrap(), bytes_before);
    assert_eq!(container.latest_version(), 1);
    assert_eq!(container.transaction_time_ns(1), Some(123));
}

#[test]
fn prefix_cursor_ending_mid_frame_returns_typed_truncation_without_writing() {
    let scratch = Scratch::new();
    let path = scratch.file("mid-frame-prefix.cva");
    let mut container = Container::create(&path).unwrap();
    let object = container.append(b"bounded physical frame payload").unwrap();
    container.sync().unwrap();
    let bytes_before = fs::read(&path).unwrap();
    let object_bytes = object.legacy_bytes();
    let offset = u64::from_le_bytes(object_bytes[..8].try_into().unwrap());
    let length = u64::from_le_bytes(object_bytes[8..].try_into().unwrap());
    let middle = offset + 8 + length / 2;
    assert!(middle > container.header_len);

    let source = HistorySource::new(&container).unwrap();
    let mut cursor = source.prefix_cursor(middle).unwrap();
    let error = match cursor.next_frame() {
        Err(error) => error,
        Ok(_) => panic!("mid-frame prefix unexpectedly produced a complete frame"),
    };
    assert!(matches!(error.0, ContainerError::TruncatedChunk(start) if start == offset));
    drop(cursor);
    assert_eq!(fs::read(&path).unwrap(), bytes_before);

    let mut physical = container.physical_cursor_through(middle).unwrap();
    assert!(matches!(
        physical.next_frame(),
        Err(ContainerError::TruncatedChunk(start)) if start == offset
    ));
    assert_eq!(fs::read(&path).unwrap(), bytes_before);
}
