use super::*;
use crate::{
    commit_prepared_native_import_with_routed_receipts, commit_prepared_native_ingest,
    prepare_native_ingest_from_claim,
};
use reliquary_insomnia::{InsomniaCandidate, InsomniaExtraction, InsomniaOwnership};
use reliquary_owner_session::OwnerScope;
use reliquary_owner_session::{
    CompletionCheck, RelSession,
    model::{ImportedBranch, ImportedConversation, ImportedMessage, InsomniaEpisodeId},
};
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(1);

#[test]
fn prepared_real_import_publication_keeps_epoch_fence_and_reopens() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    fs::create_dir_all(&root).unwrap();
    let path = root.join(format!(
        "prepared-import-{}-{}.rel",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let owner = [0x6f; 16];
    let text = "The release uses a staged rollout.";
    let mut writer = RelWriter::create_current_path(&path, OwnerScope::Project, owner).unwrap();
    let source = ImportReceipt {
        schema_version: 1,
        owner_uuid: owner,
        import_key: [0x72; 32],
        source_namespace: "test/source".into(),
        source_format: "normalized-v1".into(),
        source_digest: [0x72; 32],
        receipt_digest: [0; 32],
        conversations: vec![ImportedConversation {
            source_id: "c".into(),
            conversation_id: "c".into(),
            title: None,
            messages: vec![ImportedMessage {
                source_author: None,
                source_id: "n".into(),
                revision: "1".into(),
                source_parent_id: None,
                replaces: None,
                deleted: false,
                node_id: Some("n".into()),
                fingerprint: [0x73; 32],
                artifacts: vec![],
            }],
            branches: vec![ImportedBranch {
                source_id: "main".into(),
                branch_id: "main".into(),
                leaf_node_id: Some("n".into()),
                canonical: true,
            }],
        }],
    };
    let batch = NativeImportBatch {
        source,
        started_at_ns: 10,
        turns: vec![(
            TurnInput {
                id: "n".into(),
                conversation_id: "c".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("alice".into()),
                timestamp_ns: 1,
                content: text.into(),
                attachments: vec![],
                project_attachments: vec![],
            },
            Branch {
                id: "main".into(),
                conversation_id: "c".into(),
                leaf_node_id: "n".into(),
                canonical: true,
            },
        )],
        episodes: vec![NativeImportEpisode {
            conversation_id: "c".into(),
            start_node_id: "n".into(),
            end_node_id: "n".into(),
            finalized_at_ns: 11,
        }],
    };
    let staged = stage_native_import(&mut writer, batch).unwrap();
    let episode = staged.episodes[0];
    let claim = writer
        .claim_native_import_episode(&staged.import_id, episode, "worker", 20, 1000)
        .unwrap();
    let prepared = prepare_native_ingest_from_claim(
        &claim,
        InsomniaExtraction {
            candidates: vec![InsomniaCandidate {
                authority_kind: "direct".into(),
                category: "fact".into(),
                memory_type: "project".into(),
                temporal_status: "current".into(),
                ownership: InsomniaOwnership::Project,
                title: "Release rollout".into(),
                content: text.into(),
                source_node_id: "n".into(),
                source_quote: text.into(),
                authority_source: None,
                grounding_source: None,
                routing_mentions: vec![],
            }],
            rejected: vec![],
        },
    )
    .unwrap();
    assert_eq!(prepared.project_memories().count(), 1);
    drop(writer);
    let before = fs::read(&path).unwrap();
    writer = RelWriter::open_path_current(&path).unwrap();
    assert!(
        commit_prepared_native_ingest(&mut writer, &prepared, &claim.work, "test", "v1", 21)
            .is_err()
    );
    assert!(
        commit_prepared_native_import_with_routed_receipts(
            &mut writer,
            "wrong-import",
            &prepared,
            &claim.work,
            "test",
            "v1",
            21,
            &[]
        )
        .is_err()
    );
    drop(writer);
    assert_eq!(fs::read(&path).unwrap(), before);
    writer = RelWriter::open_path_current(&path).unwrap();
    assert_eq!(
        commit_prepared_native_import_with_routed_receipts(
            &mut writer,
            &staged.import_id,
            &prepared,
            &claim.work,
            "test",
            "v1",
            21,
            &[]
        )
        .unwrap(),
        CompletionCheck::Apply
    );
    drop(writer);
    let accepted = fs::read(&path).unwrap();
    writer = RelWriter::open_path_current(&path).unwrap();
    assert!(
        commit_prepared_native_import_with_routed_receipts(
            &mut writer,
            "wrong-import",
            &prepared,
            &claim.work,
            "test",
            "v1",
            21,
            &[]
        )
        .is_err()
    );
    drop(writer);
    assert_eq!(fs::read(&path).unwrap(), accepted);
    writer = RelWriter::open_path_current(&path).unwrap();
    assert_eq!(
        commit_prepared_native_import_with_routed_receipts(
            &mut writer,
            &staged.import_id,
            &prepared,
            &claim.work,
            "test",
            "v1",
            21,
            &[]
        )
        .unwrap(),
        CompletionCheck::AlreadyAccepted
    );
    drop(writer);
    assert_eq!(fs::read(&path).unwrap(), accepted);
    writer = RelWriter::open_path_current(&path).unwrap();
    let memory_id = writer
        .read_session()
        .insomnia_receipt(InsomniaEpisodeId(episode.0), claim.work.attempt_count)
        .unwrap()
        .memory_ids[0];
    let record = writer
        .read_session()
        .current_memory(reliquary_owner_session::model::MemoryId(memory_id.0))
        .unwrap()
        .record;
    let cut = reliquary_owner_session::DreamMemoryCut {
        id: record.id,
        revision: record.revision,
        body_id: record.body_id,
    };
    let turn = writer.read_session().rel_turn_count();
    let admission = writer
        .begin_native_import_initial_dream_pass(
            &staged.import_id,
            "only-source-memory".into(),
            cut,
            turn,
        )
        .unwrap();
    assert!(admission.preexisting_ids.is_empty());
    writer
        .commit_native_import_initial_dream_pass(
            &staged.import_id,
            "only-source-memory",
            cut,
            writer.read_session().graph_version(),
            turn,
            Default::default(),
            true,
        )
        .unwrap();
    settle_native_import(&mut writer, &staged.import_id).unwrap();
    writer
        .complete_native_import(&staged.import_id, 22)
        .unwrap();
    drop(writer);
    let reopened = RelSession::open_path_current(&path).unwrap();
    let receipt = reopened
        .insomnia_receipt(InsomniaEpisodeId(episode.0), claim.work.attempt_count)
        .unwrap();
    assert_eq!(receipt.memory_ids.len(), 1);
    assert!(matches!(
        reopened.import_status(&staged.import_id),
        Some(reliquary_owner_session::records::ImportEpochStatus::Completed { .. })
    ));
    drop(reopened);
    fs::remove_file(path).unwrap();
}
