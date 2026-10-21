use crate::{
    ArchiveError, Cva, IncomingAttachment, InteractionRole, InteractionRuntime, ProjectFileRef,
    ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef, StoredFile,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

fn rel_path() -> PathBuf {
    std::env::temp_dir().join(format!("project-file-{}.prj.rel", Uuid::new_v4()))
}

fn project_ref(bytes: &[u8], revision: &str, path: &str) -> ProjectFileRef {
    ProjectFileRef {
        revision: ProjectRevisionRef {
            repository: ProjectRepositoryRef {
                kind: ProjectRepositoryKind::Lore,
                repository_id: "repo-1".into(),
                project_path: ".".into(),
            },
            revision: revision.into(),
        },
        path: path.into(),
        content_hash: Some(Sha256::digest(bytes).into()),
    }
}

fn register(cva: &mut Cva, bytes: &[u8]) -> StoredFile {
    register_at(cva, bytes, "revision-1")
}

fn register_at(cva: &mut Cva, bytes: &[u8], revision: &str) -> StoredFile {
    cva.register_project_file(
        "plan.pdf".into(),
        Some("application/pdf".into()),
        bytes.len() as u64,
        project_ref(bytes, revision, "uploads/plan.pdf"),
    )
    .unwrap()
}

fn revision(cva: &Cva, file: &StoredFile) -> String {
    cva.project_file_ref(file.id).unwrap().revision.revision
}

#[test]
fn project_file_binding_survives_reopen_without_rel_payload() {
    let path = rel_path();
    let bytes = b"repository-owned-pdf";
    let mut cva = Cva::create_project(&path).unwrap();
    let file = register(&mut cva, bytes);
    assert_eq!(cva.stats().files, 1);
    assert_eq!(cva.stats().content_objects, 0);
    assert!(matches!(
        cva.file_bytes(file.id),
        Err(ArchiveError::MissingContent)
    ));
    let reference = cva.project_file_ref(file.id).unwrap();
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open_project(path).unwrap();
    assert_eq!(reopened.project_file_ref(file.id), Some(reference));
    assert_eq!(reopened.stats().content_objects, 0);
    assert!(matches!(
        reopened.file_bytes(file.id),
        Err(ArchiveError::MissingContent)
    ));
}

#[test]
fn project_file_attachment_survives_transcript_and_reopen() {
    let path = rel_path();
    let mut cva = Cva::create_project(&path).unwrap();
    let file = register(&mut cva, b"repository-owned-pdf");
    let mut runtime = InteractionRuntime::new(cva);
    runtime.open_session("session-1".into(), None).unwrap();
    runtime
        .begin_message("session-1", "message-1".into(), InteractionRole::User, 1)
        .unwrap();
    runtime
        .attach_project_file("session-1", "message-1", file.clone())
        .unwrap();
    runtime
        .append_text("session-1", "message-1", "Review this plan")
        .unwrap();
    runtime.complete_message("session-1", "message-1").unwrap();
    let transcript = runtime
        .conversation_transcript("session-1", "message-1")
        .unwrap();
    assert_eq!(transcript[0].attachments, vec![file.clone()]);
    drop(runtime);

    let reopened = Cva::open_project(path).unwrap();
    assert_eq!(
        reopened.files_for_source("session-1", "message-1"),
        vec![file]
    );
}

#[test]
fn unchanged_project_file_at_new_revision_keeps_distinct_historical_identity() {
    let path = rel_path();
    let bytes = b"repository-owned-pdf";
    let mut cva = Cva::create_project(&path).unwrap();
    let first = register_at(&mut cva, bytes, "revision-1");
    let second = register_at(&mut cva, bytes, "revision-2");
    assert_ne!(first.id, second.id);
    assert_eq!(first.content_id, second.content_id);
    assert_eq!(revision(&cva, &first), "revision-1");
    assert_eq!(revision(&cva, &second), "revision-2");
    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open_project(path).unwrap();
    assert_eq!(revision(&reopened, &first), "revision-1");
    assert_eq!(revision(&reopened, &second), "revision-2");
}

#[test]
fn project_attachment_must_be_registered_before_attach() {
    let path = rel_path();
    let cva = Cva::create_project(path).unwrap();
    let mut runtime = InteractionRuntime::new(cva);
    runtime.open_session("session-1".into(), None).unwrap();
    runtime
        .begin_message("session-1", "message-1".into(), InteractionRole::User, 1)
        .unwrap();
    let fake = StoredFile {
        id: crate::FileId([1; 32]),
        content_id: crate::ContentId([2; 32]),
        filename: "fake.pdf".into(),
        mime_type: Some("application/pdf".into()),
        byte_length: 3,
    };
    assert!(
        runtime
            .attach_project_file("session-1", "message-1", fake)
            .is_err()
    );
}

#[test]
fn legacy_embedded_attachment_can_gain_project_backing_without_changing_file_id() {
    let path = rel_path();
    let bytes = b"legacy-embedded-pdf";
    let mut cva = Cva::create_project(&path).unwrap();
    let file = cva
        .store_file("plan.pdf".into(), Some("application/pdf".into()), bytes)
        .unwrap();
    let reference = project_ref(bytes, "revision-legacy", "warlock/uploads/plan.pdf");

    assert!(
        cva.bind_legacy_project_file(file.id, reference.clone())
            .unwrap()
    );
    assert_eq!(cva.project_file_ref(file.id), Some(reference.clone()));
    assert_eq!(cva.file_bytes(file.id).unwrap(), bytes);
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open_project(path).unwrap();
    assert_eq!(reopened.project_file_ref(file.id), Some(reference));
    assert_eq!(reopened.file_bytes(file.id).unwrap(), bytes);
}

#[test]
fn legacy_project_binding_rejects_mismatched_content_hash() {
    let path = rel_path();
    let mut cva = Cva::create_project(path).unwrap();
    let file = cva
        .store_file("plan.pdf".into(), None, b"legacy-bytes")
        .unwrap();
    let reference = project_ref(
        b"different-bytes",
        "revision-legacy",
        "warlock/uploads/plan.pdf",
    );

    assert!(cva.bind_legacy_project_file(file.id, reference).is_err());
    assert!(cva.project_file_ref(file.id).is_none());
}

#[test]
fn project_backed_repack_removes_legacy_attachment_payload_without_replaying_state() {
    let path = rel_path();
    let output = rel_path();
    let bytes = b"legacy-embedded-pdf";
    let mut cva = Cva::create_project(&path).unwrap();
    let ingested = cva
        .ingest_turn(crate::IncomingTurn {
            id: "message-1".into(),
            conversation_id: "session-1".into(),
            parent_id: None,
            role: "user".into(),
            principal_id: None,
            timestamp_ns: 1,
            content: "Review this plan".into(),
            attachments: vec![IncomingAttachment {
                filename: "plan.pdf".into(),
                mime_type: Some("application/pdf".into()),
                bytes: bytes.to_vec(),
            }],
            project_attachments: Vec::new(),
        })
        .unwrap();
    let file = ingested.attachments[0].clone();
    let reference = project_ref(bytes, "revision-legacy", "warlock/uploads/plan.pdf");
    cva.bind_legacy_project_file(file.id, reference.clone())
        .unwrap();
    let native_local = cva.archive_version() + 1;
    let body = crate::archive_codec::publication::encode(
        &crate::archive_codec::publication::ArchivePublication::Record {
            archive_version: native_local,
            record: crate::archive_codec::ArchiveRecord::File(file.clone()),
        },
    )
    .unwrap();
    let (_, native_object) = cva.container.append_publication_envelope(&body).unwrap();
    let repeat = crate::archive_codec::publication::encode(
        &crate::archive_codec::publication::ArchivePublication::Repeat {
            archive_version: native_local + 1,
            first_applying_archive_version: native_local,
        },
    )
    .unwrap();
    let (_, repeat_object) = cva.container.append_publication_envelope(&repeat).unwrap();
    let native_payload = cva.container.read(native_object).unwrap();
    let repeat_payload = cva.container.read(repeat_object).unwrap();
    cva.sync().unwrap();
    drop(cva);
    let mut cva = Cva::open_project(&path).unwrap();
    let owner_uuid = cva.owner_uuid();
    let global_version = cva.latest_global_version();
    let transaction_times = (0..=global_version)
        .map(|v| cva.transaction_time_ns(v))
        .collect::<Vec<_>>();
    let archive_version = cva.archive_version();
    let memory_version = cva.memory_version();
    let graph_version = cva.graph_version();
    let before_size = fs::metadata(&path).unwrap().len();

    let result = cva.repack_project_backed_attachments(&output).unwrap();
    assert_eq!(result.dropped_content_objects, 1);
    assert!(result.output_file_bytes < before_size);
    drop(cva);

    let mut reopened = Cva::open_project(output).unwrap();
    assert_eq!(reopened.owner_uuid(), owner_uuid);
    assert_eq!(reopened.latest_global_version(), global_version);
    assert_eq!(
        (0..=global_version)
            .map(|v| reopened.transaction_time_ns(v))
            .collect::<Vec<_>>(),
        transaction_times
    );
    assert_eq!(reopened.archive_version(), archive_version);
    assert_eq!(reopened.memory_version(), memory_version);
    assert_eq!(reopened.graph_version(), graph_version);
    let native_ref = reopened
        .archive
        .record_version(native_local)
        .unwrap()
        .record;
    let repeat_ref = reopened
        .archive
        .record_version(native_local + 1)
        .unwrap()
        .record;
    assert_eq!(reopened.container.read(native_ref).unwrap(), native_payload);
    assert_eq!(reopened.container.read(repeat_ref).unwrap(), repeat_payload);
    assert_eq!(reopened.project_file_ref(file.id), Some(reference));
    assert_eq!(
        reopened.files_for_source("session-1", "message-1"),
        vec![file.clone()]
    );
    assert!(matches!(
        reopened.file_bytes(file.id),
        Err(ArchiveError::MissingContent)
    ));
}

#[test]
fn project_backed_repack_keeps_content_shared_with_unbound_file() {
    let path = rel_path();
    let output = rel_path();
    let shared = b"shared-content";
    let removable = b"remove-me";
    let mut cva = Cva::create_project(&path).unwrap();
    let backed_shared = cva.store_file("backed.txt".into(), None, shared).unwrap();
    let unbound_shared = cva.store_file("unbound.txt".into(), None, shared).unwrap();
    let backed_unique = cva
        .store_file("unique.txt".into(), None, removable)
        .unwrap();
    cva.bind_legacy_project_file(
        backed_shared.id,
        project_ref(shared, "revision-1", "warlock/uploads/backed.txt"),
    )
    .unwrap();
    cva.bind_legacy_project_file(
        backed_unique.id,
        project_ref(removable, "revision-1", "warlock/uploads/unique.txt"),
    )
    .unwrap();

    let result = cva.repack_project_backed_attachments(&output).unwrap();
    assert_eq!(result.dropped_content_objects, 1);
    let mut reopened = Cva::open_project(output).unwrap();
    assert_eq!(reopened.file_bytes(backed_shared.id).unwrap(), shared);
    assert_eq!(reopened.file_bytes(unbound_shared.id).unwrap(), shared);
    assert!(matches!(
        reopened.file_bytes(backed_unique.id),
        Err(ArchiveError::MissingContent)
    ));
}

#[test]
fn divergent_reconcile_replays_project_file_binding_and_source_attachment() {
    let left = rel_path();
    let right = rel_path();
    let output = rel_path();
    Cva::create_project(&left).unwrap().sync().unwrap();
    fs::copy(&left, &right).unwrap();

    let mut left_rel = Cva::open_project(&left).unwrap();
    left_rel
        .append_node(
            "left".into(),
            "left-c".into(),
            None,
            "user".into(),
            1,
            "left",
        )
        .unwrap();
    left_rel.sync().unwrap();

    let mut right_rel = Cva::open_project(&right).unwrap();
    let file = register(&mut right_rel, b"repository-owned-pdf");
    right_rel
        .ingest_turn(crate::IncomingTurn {
            id: "right".into(),
            conversation_id: "right-c".into(),
            parent_id: None,
            role: "user".into(),
            principal_id: None,
            timestamp_ns: 2,
            content: "attachment".into(),
            attachments: Vec::new(),
            project_attachments: vec![file.clone()],
        })
        .unwrap();
    right_rel.sync().unwrap();

    Cva::reconcile(&left, &right, &output).unwrap();
    let mut merged = Cva::open_project(output).unwrap();
    assert!(merged.project_file_ref(file.id).is_some());
    assert_eq!(
        merged.files_for_source("right-c", "right"),
        vec![file.clone()]
    );
    assert!(matches!(
        merged.file_bytes(file.id),
        Err(ArchiveError::MissingContent)
    ));
}
