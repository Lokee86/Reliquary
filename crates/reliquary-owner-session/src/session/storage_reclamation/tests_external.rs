use super::*;
use crate::records::external_index::{
    ExternalChunkRef, ExternalIndexStore, ExternalMessageDescriptor, ExternalMessageRef,
    IndexedChunk, IndexedMessage, SourceState,
};
use reliquary_archive::{Archive, Node, TurnInput};
use reliquary_container::ReliquaryScope;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Temp(PathBuf);
impl Temp {
    fn new(name: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".owner-session-test-tmp");
        fs::create_dir_all(&dir).unwrap();
        Self(dir.join(format!(
            "reclamation-index-{name}-{}-{}.rel",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn index_state(
    namespace: &str,
    revision: u64,
    matrix: [u8; 32],
    node: &Node,
    body_len: u64,
) -> SourceState {
    let reference = ExternalMessageRef {
        source_namespace: namespace.into(),
        conversation_id: node.conversation_id.clone(),
        message_id: node.id.clone(),
        content_hash: node.content_id.0,
    };
    SourceState {
        namespace: namespace.into(),
        contract: 1,
        profile: [1; 32],
        dimensions: 128,
        generation: 1,
        revision,
        cursor: revision,
        messages: vec![IndexedMessage {
            descriptor: ExternalMessageDescriptor {
                reference: reference.clone(),
                role: node.role.clone(),
                principal_id: node.principal_id.clone(),
                timestamp_ns: node.timestamp_ns,
                active: true,
            },
            chunks: vec![IndexedChunk {
                reference: ExternalChunkRef {
                    message: reference,
                    start: 0,
                    end: body_len,
                },
                matrix,
                row: 0,
                terms: vec![],
            }],
        }],
        last_base: 0,
        last_digest: [2; 32],
        rebuild: false,
    }
}

#[test]
fn superseded_snapshot_and_exclusive_matrix_go_but_current_namespaces_survive() {
    let source = Temp::new("heads-source");
    let output = Temp::new("heads-output");
    let owner = [0x81; 16];
    drop(RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap());
    let mut writer = Container::open_writer_as(&source.0, FileKind::Reliquary).unwrap();
    let mut archive = Archive::open(&writer).unwrap();
    let body = "canonical external source";
    let node = archive
        .accept_turn(
            &mut writer,
            TurnInput {
                id: "message".into(),
                conversation_id: "conversation".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("principal".into()),
                timestamp_ns: 12,
                content: body.into(),
                attachments: vec![],
                project_attachments: vec![],
            },
        )
        .unwrap()
        .value;
    let mut vectors = PackedVectorStore::open(&writer).unwrap();
    let stale = vectors
        .put_f32_rows(&mut writer, &[vec![0.1; 128]])
        .unwrap();
    let active = vectors
        .put_f32_rows(&mut writer, &[vec![0.2; 128]])
        .unwrap();
    let shared = vectors
        .put_f32_rows(&mut writer, &[vec![0.3; 128]])
        .unwrap();
    let mut index = ExternalIndexStore::open(&writer).unwrap();
    index
        .install_admitted(
            &mut writer,
            &archive,
            index_state("source/a", 1, stale.0, &node, body.len() as u64),
            None,
        )
        .unwrap();
    index
        .install_admitted(
            &mut writer,
            &archive,
            index_state("source/a", 2, active.0, &node, body.len() as u64),
            Some(1),
        )
        .unwrap();
    index
        .install_admitted(
            &mut writer,
            &archive,
            index_state("source/b", 1, shared.0, &node, body.len() as u64),
            None,
        )
        .unwrap();
    writer.sync().unwrap();
    drop(writer);

    let before = fs::read(&source.0).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let report = reclaim_storage_copy(&plan, &source.0, &output.0, &admission).unwrap();
    assert_eq!(report.removed_frames, 2);
    assert_eq!(fs::read(&source.0).unwrap(), before);
    RelSession::open_path(&output.0, &admission, [], []).unwrap();

    let original = Container::open_read_only(&source.0).unwrap();
    let copy = Container::open_read_only(&output.0).unwrap();
    let original_index = ExternalIndexStore::open(&original).unwrap();
    let copy_index = ExternalIndexStore::open(&copy).unwrap();
    assert_eq!(copy_index.sources, original_index.sources);
    assert_eq!(copy_index.damaged, original_index.damaged);
    let vectors = PackedVectorStore::open(&copy).unwrap();
    assert_eq!(vectors.infos().len(), 2);
    assert_eq!(vectors.read_f32_row(active, 0).unwrap(), vec![0.2; 128]);
    assert_eq!(vectors.read_f32_row(shared, 0).unwrap(), vec![0.3; 128]);
}

#[test]
fn discard_barrier_survives_and_drops_old_snapshots_and_backing() {
    let source = Temp::new("barrier-source");
    let output = Temp::new("barrier-output");
    let owner = [0x82; 16];
    drop(RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap());
    let mut writer = Container::open_writer_as(&source.0, FileKind::Reliquary).unwrap();
    let mut archive = Archive::open(&writer).unwrap();
    let body = "canonical message";
    let node = archive
        .accept_turn(
            &mut writer,
            TurnInput {
                id: "message".into(),
                conversation_id: "conversation".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: None,
                timestamp_ns: 14,
                content: body.into(),
                attachments: vec![],
                project_attachments: vec![],
            },
        )
        .unwrap()
        .value;
    let mut packed = PackedVectorStore::open(&writer).unwrap();
    let obsolete = packed.put_f32_rows(&mut writer, &[vec![0.4; 128]]).unwrap();
    let current = packed.put_f32_rows(&mut writer, &[vec![0.5; 128]]).unwrap();
    let mut index = ExternalIndexStore::open(&writer).unwrap();
    index
        .install_admitted(
            &mut writer,
            &archive,
            index_state("source/a", 1, obsolete.0, &node, body.len() as u64),
            None,
        )
        .unwrap();
    index.discard_all(&mut writer).unwrap();
    index
        .install_admitted(
            &mut writer,
            &archive,
            index_state("source/b", 1, current.0, &node, body.len() as u64),
            None,
        )
        .unwrap();
    writer.sync().unwrap();
    drop(writer);

    let before = fs::read(&source.0).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let report = reclaim_storage_copy(&plan, &source.0, &output.0, &admission).unwrap();
    assert_eq!(report.removed_frames, 2);
    assert_eq!(fs::read(&source.0).unwrap(), before);
    RelSession::open_path(&output.0, &admission, [], []).unwrap();
    let original = Container::open_read_only(&source.0).unwrap();
    let copy = Container::open_read_only(&output.0).unwrap();
    let actual = ExternalIndexStore::open(&copy).unwrap();
    let expected = ExternalIndexStore::open(&original).unwrap();
    assert_eq!(actual.sources, expected.sources);
    assert!(actual.damaged);
    let vectors = PackedVectorStore::open(&copy).unwrap();
    assert_eq!(vectors.infos().len(), 1);
    assert_eq!(vectors.read_f32_row(current, 0).unwrap(), vec![0.5; 128]);
}
#[test]
fn corrupted_derived_snapshot_is_retained_as_damage_barrier() {
    let source = Temp::new("damage-source");
    let output = Temp::new("damage-output");
    let owner = [0x83; 16];
    drop(RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap());
    let mut writer = Container::open_writer_as(&source.0, FileKind::Reliquary).unwrap();
    let body = vec![0x66; 512];
    let mut orphan = b"RELFILE1".to_vec();
    orphan.extend_from_slice(&Sha256::digest(&body));
    orphan.extend_from_slice(&(body.len() as u32).to_le_bytes());
    orphan.extend_from_slice(&body);
    writer.append(&orphan).unwrap();
    writer.append(b"RELXIDX2damaged-record").unwrap();
    writer.sync().unwrap();
    drop(writer);

    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let original = Container::open_read_only(&source.0).unwrap();
    assert!(ExternalIndexStore::open(&original).unwrap().damaged);
    RelSession::open_path(&source.0, &admission, [], []).unwrap();
    let before = fs::read(&source.0).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let report = reclaim_storage_copy(&plan, &source.0, &output.0, &admission).unwrap();
    assert_eq!(report.removed_frames, 1);
    assert_eq!(fs::read(&source.0).unwrap(), before);
    RelSession::open_path(&output.0, &admission, [], []).unwrap();
    let result = Container::open_read_only(&output.0).unwrap();
    let index = ExternalIndexStore::open(&result).unwrap();
    assert!(index.sources.is_empty());
    assert!(index.damaged);
}
