use super::*;
use crate::{
    DreamClassifier, DreamVerificationPolicy, DreamVerifier, DreamWorkflowError, EntityOwnerSession,
};
use reliquary_container::{Container, ReliquaryScope};
use reliquary_freshness_store::{FreshnessMemoryId, FreshnessStore};
use reliquary_memory_core::{
    MemoryDraft, MemoryRecord, MemorySourceRef, MemoryStore, encode_grouped_creation,
    memory_body_id, memory_id,
};
use reliquary_owner_session::{
    CompletionCheck, GraphAdmission,
    records::{GroupedInsomniaCompletion, GroupedMemoryBody},
};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
#[path = "test_postprocessing.rs"]
mod test_postprocessing;
use test_postprocessing::{ImportDreamEndpoint, ImportEmbedding, add_required_perception};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&dir).unwrap();
        Self(dir.join(format!(
            "import-{}-{}.rel",
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
fn batch(owner: [u8; 16]) -> NativeImportBatch {
    let source = ImportReceipt {
        schema_version: 1,
        owner_uuid: owner,
        import_key: [0x3a; 32],
        source_namespace: "fixture/account".into(),
        source_format: "normalized-v1".into(),
        source_digest: [0x3a; 32],
        receipt_digest: [0; 32],
        conversations: vec![reliquary_owner_session::model::ImportedConversation {
            source_id: "source-conversation".into(),
            conversation_id: "conversation".into(),
            title: None,
            messages: vec![reliquary_owner_session::model::ImportedMessage {
                source_author: None,
                source_id: "source-turn".into(),
                revision: "1".into(),
                source_parent_id: None,
                replaces: None,
                deleted: false,
                node_id: Some("imported-turn".into()),
                fingerprint: [0x34; 32],
                artifacts: vec![],
            }],
            branches: vec![reliquary_owner_session::model::ImportedBranch {
                source_id: "source-main".into(),
                branch_id: "main".into(),
                leaf_node_id: Some("imported-turn".into()),
                canonical: true,
            }],
        }],
    };
    NativeImportBatch {
        source,
        turns: vec![(
            TurnInput {
                id: "imported-turn".into(),
                conversation_id: "conversation".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("alice".into()),
                timestamp_ns: 1,
                content: "imported evidence".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
            Branch {
                id: "main".into(),
                conversation_id: "conversation".into(),
                leaf_node_id: "imported-turn".into(),
                canonical: true,
            },
        )],
        episodes: vec![NativeImportEpisode {
            conversation_id: "conversation".into(),
            start_node_id: "imported-turn".into(),
            end_node_id: "imported-turn".into(),
            finalized_at_ns: 11,
        }],
        started_at_ns: 10,
    }
}

fn memory_draft(mutation_id: &str, lifecycle: &str) -> MemoryDraft {
    MemoryDraft {
        title: mutation_id.into(),
        content: format!("content for {mutation_id}"),
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
        lifecycle_state: lifecycle.into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: None,
        source_time_ns: Some(1),
        temporal_inference: None,
        mutation_id: mutation_id.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

#[test]
fn import_restarts_then_finishes_scoped_work_and_settlement_once() {
    let path = Temp::new();
    let phy_path = Temp(path.0.with_extension("phy"));
    let owner = [0x62; 16];
    let phy_owner = [0x65; 16];
    let input = batch(owner);
    let (id, episode);
    {
        let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();
        let (mut live_turn, mut live_branch) = input.turns[0].clone();
        live_turn.id = "live-turn".into();
        live_turn.conversation_id = "live-conversation".into();
        live_turn.timestamp_ns = 100;
        live_branch.conversation_id = live_turn.conversation_id.clone();
        live_branch.leaf_node_id = live_turn.id.clone();
        writer
            .accept_turn_and_advance_branch(live_turn, live_branch)
            .unwrap();
        let staged = stage_native_import(&mut writer, input.clone()).unwrap();
        let timeline = import_timeline(&writer).unwrap();
        assert_eq!(timeline.activity_order[0].node_id, "live-turn");
        assert_eq!(timeline.activity_order[1].node_id, "imported-turn");
        id = staged.import_id;
        episode = staged.episodes[0];
        assert!(writer.complete_native_import(&id, 12).is_err());
        assert!(matches!(
            writer.read_session().import_status(&id),
            Some(ImportEpochStatus::Open(_))
        ));
    }
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::open_path(&path.0, grant.clone(), [], []).unwrap();
    let before = fs::metadata(&path.0).unwrap().len();
    assert_eq!(
        stage_native_import(&mut writer, input.clone())
            .unwrap()
            .episodes,
        vec![episode]
    );
    assert_eq!(fs::metadata(&path.0).unwrap().len(), before);
    let claim = writer
        .claim_native_import_episode(&id, episode, "worker", 20, 100)
        .unwrap();
    assert_eq!(claim.turns[0].content, "imported evidence");
    let mut phy = reliquary_owner_session::PhyWriter::create_path(&phy_path.0, phy_owner).unwrap();
    let mut draft = memory_draft("import-routed-only", "extracted");
    draft.scope = "user".into();
    draft.source_ref = Some(MemorySourceRef {
        owner_id: owner.iter().map(|byte| format!("{byte:02x}")).collect(),
        principal_id: Some("alice".into()),
        source_episode_id: episode.0,
        source_node_id: "imported-turn".into(),
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
    });
    let proof = writer
        .read_session()
        .prove_routed_user_memory(&phy, &draft, "imported evidence", None)
        .unwrap();
    let (published, routed) = phy.publish_routed_user_memory(proof).unwrap();
    let routed_id = published.record.id;
    let completion = GroupedInsomniaCompletion {
        episode_id: episode.0,
        attempt: claim.work.attempt_count,
        started_at_ns: claim.started_at_ns,
        completed_at_ns: 21,
        transaction_time_ns: 21,
        extractor_model: "fixture".into(),
        extractor_version: "v1".into(),
        rejected_count: 0,
        memory_ids: vec![],
        external_memory_refs: vec![routed.external_ref()],
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
                &id,
                claim.work.lease_token.unwrap(),
                completion,
                21,
                &[routed],
            )
            .unwrap(),
        CompletionCheck::Apply
    );
    assert!(writer.native_import_memory_ids(&id).unwrap().is_empty());
    assert_eq!(
        writer
            .read_session()
            .native_import_routed_memory_ids(&id, phy_owner)
            .unwrap(),
        vec![routed_id]
    );
    assert!(
        writer
            .read_session()
            .native_import_routed_memory_ids(&id, [0xff; 16])
            .unwrap()
            .is_empty()
    );
    let frozen = settle_native_import(&mut writer, &id).unwrap();
    assert_eq!(frozen.import_id, id);
    assert!(
        writer
            .read_session()
            .native_import_settlement(&id)
            .is_some()
    );
    assert!(settle_native_import(&mut writer, &id).is_ok());
    drop(writer);
    // Simulate a restart after the settlement sync but before completion.
    let mut writer = RelWriter::open_path(&path.0, grant.clone(), [], []).unwrap();
    assert_eq!(
        writer
            .read_session()
            .native_import_routed_memory_ids(&id, phy_owner)
            .unwrap(),
        vec![routed_id]
    );
    let sealed = stage_native_import(&mut writer, input.clone()).unwrap();
    assert!(!sealed.already_completed);
    assert_eq!(sealed.episodes, vec![episode]);
    assert_eq!(
        writer.read_session().native_import_settlement(&id),
        Some(&frozen)
    );
    let (accepted, did_write) = writer.complete_native_import(&id, 22).unwrap();
    assert!(did_write);
    assert!(
        writer
            .read_session()
            .native_import_settlement(&id)
            .is_some()
    );
    assert_eq!(
        writer.complete_native_import(&id, 22).unwrap(),
        (accepted.clone(), false)
    );
    drop(writer);
    let mut reopened = RelWriter::open_path(&path.0, grant, [], []).unwrap();
    assert_eq!(
        reopened
            .read_session()
            .native_import_routed_memory_ids(&id, phy_owner)
            .unwrap(),
        vec![routed_id]
    );
    assert!(matches!(reopened.read_session().import_status(&id),
        Some(ImportEpochStatus::Completed { complete, .. }) if complete == accepted));
    assert!(
        stage_native_import(&mut reopened, input)
            .unwrap()
            .already_completed
    );
    assert_eq!(
        reopened.complete_native_import(&id, 30).unwrap(),
        (accepted, false)
    );
}

#[test]
fn import_rejects_episode_evidence_outside_its_source_receipt() {
    let path = Temp::new();
    let owner = [0x63; 16];
    let mut input = batch(owner);
    let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();

    let (mut prior_turn, mut prior_branch) = input.turns[0].clone();
    prior_turn.id = "prior-turn".into();
    prior_branch.leaf_node_id = prior_turn.id.clone();
    writer
        .accept_turn_and_advance_branch(prior_turn, prior_branch)
        .unwrap();

    input.turns[0].0.parent_id = Some("prior-turn".into());
    input.episodes[0].start_node_id = "prior-turn".into();
    let staged = stage_native_import(&mut writer, input).unwrap();
    let before = fs::metadata(&path.0).unwrap().len();
    let error = writer
        .claim_native_import_episode(&staged.import_id, staged.episodes[0], "worker", 20, 100)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("path escapes its receipt source set")
    );
    let error = settle_native_import(&mut writer, &staged.import_id).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("path escapes its receipt source set")
    );
    assert_eq!(fs::metadata(&path.0).unwrap().len(), before);
}

#[test]
fn historical_dream_credit_is_published_once_and_survives_reopen() {
    let path = Temp::new();
    let owner = [0x64; 16];
    let input = batch(owner);
    let old_id;
    let excluded_id;
    let source_memory_id;
    let old_cut;
    let excluded_after;
    let settlement_turn;
    let import_id;
    {
        drop(RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap());
        let mut container = Container::open_writer(&path.0).unwrap();
        let mut memories = MemoryStore::open(&container).unwrap();
        old_id = memories
            .publish(
                &mut container,
                None,
                0,
                memory_draft("historical-old", "knowledge"),
            )
            .unwrap()
            .revision
            .record
            .id;
        excluded_id = memories
            .publish(
                &mut container,
                None,
                0,
                memory_draft("historical-excluded", "archived"),
            )
            .unwrap()
            .revision
            .record
            .id;
        let mut freshness = FreshnessStore::open(&container).unwrap();
        freshness
            .create_birth(&mut container, FreshnessMemoryId(old_id.0), 1, true)
            .unwrap();
        freshness
            .create_birth(&mut container, FreshnessMemoryId(excluded_id.0), 1, true)
            .unwrap();
        container.sync().unwrap();
        drop(container);
        let mut writer = RelWriter::open_path(
            &path.0,
            GraphAdmission::for_replay(owner, [old_id.0, excluded_id.0], [], [], [], []),
            [],
            [],
        )
        .unwrap();
        let mut parent = None;
        for index in 0..600 {
            let id = format!("aging-{index}");
            writer
                .accept_turn_and_advance_branch(
                    TurnInput {
                        id: id.clone(),
                        conversation_id: "aging".into(),
                        parent_id: parent.clone(),
                        role: "user".into(),
                        principal_id: Some("alice".into()),
                        timestamp_ns: index,
                        content: "age existing Memory".into(),
                        attachments: vec![],
                        project_attachments: vec![],
                    },
                    Branch {
                        id: "main".into(),
                        conversation_id: "aging".into(),
                        leaf_node_id: id.clone(),
                        canonical: true,
                    },
                )
                .unwrap();
            parent = Some(id);
        }
        old_cut = writer
            .read_session()
            .freshness_record_at(FreshnessMemoryId(old_id.0), 600)
            .unwrap()
            .unwrap();

        let staged = stage_native_import(&mut writer, input).unwrap();
        let episode = staged.episodes[0];
        let claim = writer
            .claim_native_import_episode(&staged.import_id, episode, "worker", 700, 100)
            .unwrap();
        let mutation = "historical-import-source";
        source_memory_id = memory_id(mutation);
        let body = GroupedMemoryBody::from_content("Imported source", "Imported source content");
        let record = MemoryRecord {
            id: source_memory_id,
            revision: 1,
            body_id: memory_body_id("Imported source", "Imported source content"),
            category: "fact".into(),
            memory_type: "project".into(),
            authority_kind: "direct".into(),
            temporal_status: "current".into(),
            scope: "project".into(),
            lifecycle_state: "extracted".into(),
            archived: false,
            parent_id: None,
            superseded_by: None,
            source_node_id: Some("imported-turn".into()),
            content_source_conversation_id: Some("conversation".into()),
            content_source_node_id: Some("imported-turn".into()),
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: Some(episode.0),
            source_ref: Some(MemorySourceRef {
                owner_id: "project".into(),
                principal_id: None,
                source_episode_id: episode.0,
                source_node_id: "imported-turn".into(),
                content_source_conversation_id: Some("conversation".into()),
                content_source_node_id: Some("imported-turn".into()),
                grounding_source_conversation_id: None,
                grounding_source_node_id: None,
            }),
            source_time_ns: Some(1),
            temporal_inference: None,
            mutation_id: mutation.into(),
            created_at_ns: 701,
            updated_at_ns: 701,
            memory_version: writer.read_session().memory_version() + 1,
        };
        let completion = GroupedInsomniaCompletion {
            episode_id: episode.0,
            attempt: claim.work.attempt_count,
            started_at_ns: claim.started_at_ns,
            completed_at_ns: 701,
            transaction_time_ns: 701,
            extractor_model: "fixture".into(),
            extractor_version: "v1".into(),
            rejected_count: 0,
            memory_ids: vec![source_memory_id.0],
            external_memory_refs: vec![],
            global_version_start: writer.next_global_version().unwrap(),
            bodies: vec![body],
            memory_records: vec![encode_grouped_creation(&record).unwrap()],
            routing_metadata: vec![],
            freshness_birth_turn: Some(writer.read_session().rel_turn_count()),
            freshness_policy_version: Some(1),
            freshness_birth_memory_ids: vec![source_memory_id.0],
        };
        writer
            .commit_native_import_completion_with_routed_receipts(
                &staged.import_id,
                claim.work.lease_token.unwrap(),
                completion,
                701,
                &[],
            )
            .unwrap();
        let id = &staged.import_id;
        import_id = id.clone();
        assert_eq!(
            writer.native_import_memory_ids(id).unwrap(),
            vec![source_memory_id]
        );
        assert!(
            writer
                .validate_native_import_memory("wrong-epoch", source_memory_id)
                .is_err()
        );
        assert!(writer.validate_native_import_memory(id, old_id).is_err());
        assert!(
            finish_native_import_postprocessing(
                &mut writer,
                id,
                &NativeImportPostprocessingPlan::default(),
                800,
            )
            .is_err()
        );
        assert!(
            !writer
                .read_session()
                .has_native_import_historical_freshness(id)
        );

        let candidates = [old_id];
        let policy = DreamVerificationPolicy {
            topical: true,
            ..Default::default()
        };
        let classifier = DreamClassifier::new(ImportDreamEndpoint("yes"));
        let held = prepare_native_import_dream(
            &mut writer,
            id,
            source_memory_id,
            NativeImportDreamCandidates::Explicit(&candidates),
        )
        .unwrap()
        .unwrap()
        .evaluate(
            &classifier,
            Some(&DreamVerifier::new(ImportDreamEndpoint("uncertain"))),
            policy,
        )
        .unwrap();
        let before = fs::metadata(&path.0).unwrap().len();
        assert_eq!(
            commit_native_import_dream(&mut writer, &held, 702),
            Err(DreamWorkflowError::IncompleteFrontier),
        );
        assert_eq!(fs::metadata(&path.0).unwrap().len(), before);
        assert!(
            !writer
                .read_session()
                .dream_pair_evaluated(source_memory_id, old_id)
        );

        // Simulate interruption after the accepted batch but before settlement.
        let (pass, publication) = RelWriter::native_import_dream_identity(id, source_memory_id);
        let draft = crate::draft_dream_batch_organization(
            &EntityOwnerSession::Rel(writer.read_session()),
            owner,
            source_memory_id,
            &candidates,
            &classifier,
            Some(&DreamVerifier::new(ImportDreamEndpoint("yes"))),
            policy,
        )
        .unwrap();
        let cut =
            crate::dream_batch_workflow::prepare_dream_batch_cut(owner, publication, &draft, 703)
                .unwrap();
        assert!(writer.commit_dream_batch(cut.clone()).is_err());
        assert!(
            writer
                .commit_native_import_dream_batch("wrong-epoch", &pass, cut.clone())
                .is_err()
        );
        assert!(
            writer
                .commit_native_import_dream_batch(id, &pass, cut)
                .unwrap()
        );
        assert!(!writer.initial_dream_settled(&pass));
        drop(writer);

        let mut writer = RelWriter::open_path_current(&path.0).unwrap();
        assert!(
            prepare_native_import_dream(
                &mut writer,
                id,
                source_memory_id,
                NativeImportDreamCandidates::Explicit(&[]),
            )
            .unwrap()
            .is_none()
        );
        assert!(writer.initial_dream_settled(&pass));
        assert!(!settle_native_import_initial_dream(&mut writer, id, source_memory_id).unwrap());

        let profile = crate::prepare_compatibility_profile(&ImportEmbedding).unwrap();
        let profile = writer.publish_native_import_profile(id, profile).unwrap();
        let mut plan = NativeImportPostprocessingPlan {
            memory_vector_profiles: vec![profile],
            ..Default::default()
        };
        assert_eq!(
            writer
                .native_import_missing_memory_vectors(id, profile)
                .unwrap(),
            vec![source_memory_id]
        );
        assert!(
            writer
                .native_import_memory_vector_input(id, profile, &[old_id])
                .is_err()
        );
        assert!(finish_native_import_postprocessing(&mut writer, id, &plan, 800).is_err());
        assert!(
            !writer
                .read_session()
                .has_native_import_historical_freshness(id)
        );
        let vector_input = writer
            .native_import_memory_vector_input(id, profile, &[source_memory_id])
            .unwrap();
        let vectors = crate::evaluate_memory_vectors(vector_input, &ImportEmbedding).unwrap();
        assert!(crate::commit_native_import_memory_vectors(&mut writer, id, &vectors).unwrap());
        assert!(!crate::commit_native_import_memory_vectors(&mut writer, id, &vectors).unwrap());

        add_required_perception(&mut writer, id, source_memory_id, &mut plan);
        let excluded_before = writer
            .read_session()
            .freshness_record_at(
                FreshnessMemoryId(excluded_id.0),
                writer.read_session().rel_turn_count(),
            )
            .unwrap()
            .unwrap();
        let frozen = settle_native_import(&mut writer, id).unwrap();
        drop(writer);
        let mut writer = RelWriter::open_path_current(&path.0).unwrap();
        assert_eq!(
            writer.native_import_memory_ids(id).unwrap(),
            vec![source_memory_id]
        );
        assert!(writer.validate_native_import_postprocessing(id).is_err());
        let outcome = finish_native_import_postprocessing(&mut writer, id, &plan, 800).unwrap();
        assert_eq!(outcome.settlement, frozen);
        assert!(outcome.did_complete);
        assert!(
            writer
                .read_session()
                .has_native_import_historical_freshness(id)
        );
        assert!(
            writer
                .publish_native_import_profile(
                    id,
                    crate::prepare_compatibility_profile(&ImportEmbedding).unwrap()
                )
                .is_err()
        );
        assert!(
            !finish_native_import_postprocessing(&mut writer, id, &plan, 801)
                .unwrap()
                .did_complete
        );
        let settlement = outcome.settlement;
        settlement_turn = settlement.rel_turn;
        let after = writer
            .read_session()
            .freshness_record_through_turn(FreshnessMemoryId(old_id.0), settlement.rel_turn)
            .unwrap()
            .unwrap();
        excluded_after = writer
            .read_session()
            .freshness_record_through_turn(FreshnessMemoryId(excluded_id.0), settlement.rel_turn)
            .unwrap()
            .unwrap();
        assert!(after.score > old_cut.score);
        assert_eq!(excluded_after, excluded_before);
    }
    let reopened = RelWriter::open_path_current(&path.0).unwrap();
    assert_eq!(
        reopened
            .read_session()
            .native_import_memory_ids(&import_id)
            .unwrap(),
        vec![source_memory_id]
    );
    assert!(matches!(
        reopened.read_session().import_status(&import_id),
        Some(ImportEpochStatus::Completed { .. })
    ));
    assert!(
        reopened
            .read_session()
            .has_native_import_historical_freshness(&import_id)
    );
    let after = reopened
        .read_session()
        .freshness_record_through_turn(FreshnessMemoryId(old_id.0), 601)
        .unwrap()
        .unwrap();
    assert!(after.score > old_cut.score);
    assert_eq!(
        reopened
            .read_session()
            .freshness_record_through_turn(FreshnessMemoryId(excluded_id.0), settlement_turn)
            .unwrap()
            .unwrap(),
        excluded_after
    );
}
