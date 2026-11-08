//! Additional G4-T03 evidence for native import epoch normalization and retry.
//!
//! The fixture enters through the accepted Application import workflow. It
//! never writes Archive, Episode, or import receipt tables directly.

#[cfg(test)]
use super::*;
use reliquary_application::native_import_workflow::{
    NativeImportBatch, NativeImportEpisode, settle_native_import, stage_native_import,
};
use reliquary_container::ReliquaryScope;
use reliquary_owner_session::model::{
    Branch, ImportReceipt, ImportedBranch, ImportedConversation, ImportedMessage, TurnInput,
};
use reliquary_owner_session::{
    CompletionCheck, GraphAdmission,
    records::{GroupedInsomniaCompletion, ImportEpochStatus},
};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

struct ImportTemp(PathBuf);

impl ImportTemp {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("g4-t03-import-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for ImportTemp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn native_batch(owner: [u8; 16]) -> NativeImportBatch {
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/g4-native-rel21-v1.json"
    ))
    .unwrap();
    let script = manifest["conversation_scripts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|script| script["id"] == "C-03-import")
        .unwrap();
    let conversation_id = script["conversation_id"].as_str().unwrap();
    let turns = script["turns"].as_array().unwrap();
    let digest = [0x5a; 32];
    let messages = turns
        .iter()
        .map(|turn| ImportedMessage {
            source_author: None,
            source_id: turn["id"].as_str().unwrap().to_owned(),
            revision: "1".into(),
            source_parent_id: None,
            replaces: None,
            deleted: false,
            node_id: Some(turn["id"].as_str().unwrap().to_owned()),
            fingerprint: *blake3::hash(turn["content"].as_str().unwrap().as_bytes()).as_bytes(),
            artifacts: vec![],
        })
        .collect();
    let imported_turns = turns
        .iter()
        .enumerate()
        .map(|(index, turn)| {
            let id = turn["id"].as_str().unwrap().to_owned();
            let parent_id = index
                .checked_sub(1)
                .map(|parent| turns[parent]["id"].as_str().unwrap().to_owned());
            (
                TurnInput {
                    id: id.clone(),
                    conversation_id: conversation_id.into(),
                    parent_id,
                    role: turn["speaker"].as_str().unwrap().into(),
                    principal_id: Some("principal-b".into()),
                    timestamp_ns: 41 + index as i64,
                    content: turn["content"].as_str().unwrap().into(),
                    attachments: vec![],
                    project_attachments: vec![],
                },
                Branch {
                    id: "main".into(),
                    conversation_id: conversation_id.into(),
                    leaf_node_id: id,
                    canonical: true,
                },
            )
        })
        .collect();
    NativeImportBatch {
        source: ImportReceipt {
            schema_version: 1,
            owner_uuid: owner,
            import_key: digest,
            source_namespace: "g4/cascade/native".into(),
            source_format: "normalized-v1".into(),
            source_digest: digest,
            receipt_digest: [0; 32],
            conversations: vec![ImportedConversation {
                source_id: "C-03-import".into(),
                conversation_id: conversation_id.into(),
                title: Some("Cascade native import".into()),
                messages,
                branches: vec![ImportedBranch {
                    source_id: "source-main".into(),
                    branch_id: "main".into(),
                    leaf_node_id: turns
                        .last()
                        .and_then(|turn| turn["id"].as_str())
                        .map(str::to_owned),
                    canonical: true,
                }],
            }],
        },
        turns: imported_turns,
        episodes: vec![NativeImportEpisode {
            conversation_id: conversation_id.into(),
            start_node_id: turns[0]["id"].as_str().unwrap().into(),
            end_node_id: turns.last().unwrap()["id"].as_str().unwrap().into(),
            finalized_at_ns: 46,
        }],
        started_at_ns: 40,
    }
}

pub(crate) fn complete_authored_native_import(instance: &Instance, cascade: OwnerId) -> Value {
    let mut batch = native_batch(cascade.as_bytes());
    let imported_conversation = "conv-c03-additive-import".to_owned();
    batch.source.conversations[0].conversation_id = imported_conversation.clone();
    for (turn, branch) in &mut batch.turns {
        turn.conversation_id = imported_conversation.clone();
        branch.conversation_id = imported_conversation.clone();
    }
    batch.episodes[0].conversation_id = imported_conversation.clone();
    let retry_batch = batch.clone();
    let staged = instance
        .with_rel_writer(cascade, |writer| stage_native_import(writer, batch))
        .unwrap()
        .unwrap();
    assert!(!staged.already_completed);
    instance
        .with_rel_writer(cascade, |writer| {
            let before = writer.read_session().current_historical_cut();
            assert!(
                settle_native_import(writer, &staged.import_id).is_err(),
                "an unfinished imported Episode cannot settle"
            );
            assert_eq!(writer.read_session().current_historical_cut(), before);
            let mut incompatible = retry_batch.clone();
            incompatible.source.source_format = "unsupported-native-profile".into();
            assert!(
                stage_native_import(writer, incompatible).is_err(),
                "an epoch cannot change its admitted native input profile"
            );
            assert_eq!(writer.read_session().current_historical_cut(), before);
        })
        .unwrap();
    let (claim, settlement) = instance
        .with_rel_writer(cascade, |writer| {
            assert!(
                writer
                    .complete_native_import(&staged.import_id, 199_999)
                    .is_err()
            );
            let claim = writer.claim_native_import_episode(
                &staged.import_id,
                staged.episodes[0],
                "g4-authored-import",
                200_000,
                100,
            )?;
            let completion = GroupedInsomniaCompletion {
                episode_id: staged.episodes[0].0,
                attempt: claim.work.attempt_count,
                started_at_ns: claim.started_at_ns,
                completed_at_ns: 200_001,
                transaction_time_ns: 200_001,
                extractor_model: "g4-authored-import".into(),
                extractor_version: "v1".into(),
                rejected_count: 0,
                memory_ids: vec![],
                external_memory_refs: vec![],
                global_version_start: writer.next_global_version()?,
                bodies: vec![],
                memory_records: vec![],
                routing_metadata: vec![],
                freshness_birth_turn: None,
                freshness_policy_version: None,
                freshness_birth_memory_ids: vec![],
            };
            writer.commit_native_import_completion_with_routed_receipts(
                &staged.import_id,
                claim.work.lease_token.as_ref().unwrap().clone(),
                completion,
                200_001,
                &[],
            )?;
            let settlement = settle_native_import(writer, &staged.import_id)?;
            let (_, did_complete) = writer.complete_native_import(&staged.import_id, 200_002)?;
            assert!(did_complete);
            assert!(!writer.complete_native_import(&staged.import_id, 200_003)?.1);
            Ok::<_, reliquary_owner_session::OwnerSessionError>((claim, settlement))
        })
        .unwrap()
        .unwrap();
    let retry = instance
        .with_rel_writer(cascade, |writer| {
            stage_native_import(writer, retry_batch.clone())
        })
        .unwrap()
        .unwrap();
    assert!(retry.already_completed);
    let mut conflicting = retry_batch;
    conflicting.turns[0].0.content = "Conflicting native import retry".into();
    assert!(
        instance
            .with_rel_writer(cascade, |writer| stage_native_import(writer, conflicting))
            .unwrap()
            .is_err()
    );
    assert_eq!(
        instance
            .with_writable_reliquary(cascade, |rel| rel
                .resolved_archive_turn(&imported_conversation, "C03-t1"))
            .unwrap()
            .unwrap()
            .unwrap()
            .content,
        "Import the supported native 2.1 input for Cascade."
    );
    json!({
        "import_id": staged.import_id,
        "conversation_id": imported_conversation,
        "episode_id": staged.episodes[0].0,
        "attempt": claim.work.attempt_count,
        "incomplete_settlement_rejected": true,
        "changed_native_profile_rejected": true,
        "settlement": settlement.import_id,
        "already_completed": staged.already_completed,
        "exact_retry_completed": retry.already_completed,
    })
}

#[test]
fn g4_t03_native_import_normalizes_retries_and_rejects_conflicts_after_reopen() {
    let temp = ImportTemp::new();
    let owner = [211; 16];
    let path = temp.0.join("rel-cascade.rel");
    drop(RelWriter::create_path(&path, ReliquaryScope::Project, owner).unwrap());
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut batch = native_batch(owner);

    let staged = {
        let mut writer = RelWriter::open_path(&path, admission.clone(), [], []).unwrap();
        let staged = stage_native_import(&mut writer, batch.clone()).unwrap();
        assert!(!staged.already_completed);
        let turn = writer
            .read_session()
            .resolved_archive_turn("conv-c03", "C03-t1")
            .unwrap()
            .unwrap();
        assert_eq!(
            turn.content,
            "Import the supported native 2.1 input for Cascade."
        );
        assert_eq!(turn.principal_id.as_deref(), Some("principal-b"));
        assert_eq!(turn.timestamp_ns, 41);
        assert_eq!(turn.role, "user");
        assert!(writer.read_session().episode(staged.episodes[0]).is_some());

        // An incomplete epoch cannot be declared complete before the accepted
        // Insomnia completion and settlement have been recorded.
        assert!(
            writer
                .complete_native_import(&staged.import_id, 43)
                .is_err()
        );
        assert!(matches!(
            writer.read_session().import_status(&staged.import_id),
            Some(ImportEpochStatus::Open(_))
        ));
        staged
    };

    // Exact staged retry is a read-only confirmation after an independent
    // owner reopen. A changed source mapping is rejected without publication.
    let before_retry = fs::metadata(&path).unwrap().len();
    {
        let mut writer = RelWriter::open_path(&path, admission.clone(), [], []).unwrap();
        let retry = stage_native_import(&mut writer, batch.clone()).unwrap();
        assert_eq!(retry.import_id, staged.import_id);
        assert_eq!(retry.episodes, staged.episodes);
        assert!(!retry.already_completed);
        assert_eq!(fs::metadata(&path).unwrap().len(), before_retry);

        batch.turns[0].0.content = "Changed after the import receipt.".into();
        assert!(stage_native_import(&mut writer, batch).is_err());
        assert_eq!(fs::metadata(&path).unwrap().len(), before_retry);
        assert!(matches!(
            writer.read_session().import_status(&staged.import_id),
            Some(ImportEpochStatus::Open(_))
        ));
    }

    let accepted = {
        let mut writer = RelWriter::open_path(&path, admission.clone(), [], []).unwrap();
        let claim = writer
            .claim_native_import_episode(
                &staged.import_id,
                staged.episodes[0],
                "g4-native-import-worker",
                44,
                100,
            )
            .unwrap();
        let completion = GroupedInsomniaCompletion {
            episode_id: staged.episodes[0].0,
            attempt: claim.work.attempt_count,
            started_at_ns: claim.started_at_ns,
            completed_at_ns: 45,
            transaction_time_ns: 45,
            extractor_model: "g4-scripted".into(),
            extractor_version: "v1".into(),
            rejected_count: 0,
            memory_ids: vec![],
            external_memory_refs: vec![],
            global_version_start: writer.next_global_version().unwrap(),
            bodies: vec![],
            memory_records: vec![],
            routing_metadata: vec![],
            freshness_birth_turn: None,
            freshness_policy_version: None,
            freshness_birth_memory_ids: vec![],
        };
        assert_eq!(
            writer
                .commit_native_import_completion_with_routed_receipts(
                    &staged.import_id,
                    claim.work.lease_token.unwrap(),
                    completion,
                    45,
                    &[],
                )
                .unwrap(),
            CompletionCheck::Apply
        );
        let settlement = settle_native_import(&mut writer, &staged.import_id).unwrap();
        assert_eq!(settlement.import_id, staged.import_id);
        drop(writer);

        let mut writer = RelWriter::open_path(&path, admission.clone(), [], []).unwrap();
        let (accepted, did_write) = writer
            .complete_native_import(&staged.import_id, 46)
            .unwrap();
        assert!(did_write);
        assert_eq!(
            writer
                .complete_native_import(&staged.import_id, 47)
                .unwrap(),
            (accepted.clone(), false)
        );
        accepted
    };

    let mut reopened = RelWriter::open_path(&path, admission, [], []).unwrap();
    let turn = reopened
        .read_session()
        .resolved_archive_turn("conv-c03", "C03-t1")
        .unwrap()
        .unwrap();
    assert_eq!(
        turn.content,
        "Import the supported native 2.1 input for Cascade."
    );
    assert!(
        reopened
            .read_session()
            .episode(staged.episodes[0])
            .is_some()
    );
    assert!(matches!(
        reopened.read_session().import_status(&staged.import_id),
        Some(ImportEpochStatus::Completed { complete, .. }) if complete == accepted
    ));
    assert!(
        stage_native_import(&mut reopened, native_batch(owner))
            .unwrap()
            .already_completed
    );
}
