use super::*;
use crate::VerifiedOwnerCopyPlan;
use crate::observation::*;
use crate::session::{RelWriter, observations};
use reliquary_archive::TurnInput;
use reliquary_graph::GraphAdmission;
use reliquary_memory_core::{MemoryDraft, MemorySourceRef};
use reliquary_perception_store::MemoryTextField;

const OWNER: [u8; 16] = [87; 16];

fn reopen_at_checkpoint(
    owner: &Path,
    slot: &Path,
) -> (Container, RelSession, Option<WriterCheckpoint>) {
    let lease =
        Container::prepare_checkpoint_open(owner, true, FileKind::Reliquary, Some(OWNER)).unwrap();
    let extent = lease.extent();
    open_leased(lease, Some(slot.to_path_buf()), extent, true).unwrap()
}

fn open_writer(owner: &Path) -> RelWriter {
    RelWriter::open_path(owner, GraphAdmission::for_changes(OWNER, [], []), [], []).unwrap()
}

fn source(writer: &mut RelWriter, key: &str, content: &str) -> ObservationCitation {
    writer
        .session
        .archive
        .accept_turn(
            &mut writer.container,
            TurnInput {
                id: key.into(),
                conversation_id: "accepted".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("owner".into()),
                timestamp_ns: 1,
                content: content.into(),
                attachments: Vec::new(),
                project_attachments: Vec::new(),
            },
        )
        .unwrap();
    let body = writer
        .session
        .common
        .memories
        .publish_versioned_creation(
            &mut writer.container,
            MemoryDraft {
                title: "Accepted evidence".into(),
                content: content.into(),
                category: "fact".into(),
                memory_type: "semantic".into(),
                authority_kind: "direct".into(),
                temporal_status: "current".into(),
                scope: "local".into(),
                lifecycle_state: "knowledge".into(),
                archived: false,
                parent_id: None,
                superseded_by: None,
                source_node_id: None,
                content_source_conversation_id: None,
                content_source_node_id: None,
                grounding_source_conversation_id: None,
                grounding_source_node_id: None,
                source_episode_id: None,
                source_ref: Some(MemorySourceRef {
                    owner_id: "57".repeat(16),
                    principal_id: Some("owner".into()),
                    source_episode_id: [2; 32],
                    source_node_id: key.into(),
                    content_source_conversation_id: Some("accepted".into()),
                    content_source_node_id: Some(key.into()),
                    grounding_source_conversation_id: None,
                    grounding_source_node_id: None,
                }),
                source_time_ns: Some(1),
                temporal_inference: None,
                mutation_id: key.into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
            1,
        )
        .unwrap()
        .revision;
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    ObservationCitation {
        owner_id: OWNER,
        history_generation: writer.session.history_generation,
        source: ObservationSource::Memory(body.record.id.0),
        revision: body.record.revision,
        body_id: body.record.body_id.0,
        field: MemoryTextField::Content,
        start_byte: 0,
        end_byte: content.len() as u32,
    }
}

fn draft(claim: &str, citations: Vec<ObservationCitation>) -> ObservationDraft {
    let receptors = [
        ObservationReceptorKind::Support,
        ObservationReceptorKind::Challenge,
        ObservationReceptorKind::Qualification,
        ObservationReceptorKind::Change,
    ]
    .into_iter()
    .map(|kind| ObservationReceptor {
        kind,
        description: format!("Evidence to evaluate {kind:?} for the claim"),
        embedding_profile: [5; 32],
        vector: vec![0.0, 1.0],
    })
    .collect();
    ObservationDraft {
        proposition: claim.into(),
        profile: ObservationProfile::Ordinary,
        status: ObservationStatus::Observed,
        time_scope: ObservationTimeScope::Current,
        validity: None,
        citations,
        reviewed: Vec::new(),
        receptors,
    }
}

fn batch(
    writer: &RelWriter,
    fingerprint: u8,
    operations: Vec<ObservationOperation>,
) -> ObservationBatch {
    ObservationBatch {
        owner_id: OWNER,
        history_generation: writer.session.history_generation,
        input_fingerprint: [fingerprint; 32],
        policy: ObservationPolicy {
            version: 1,
            general_route: [1; 32],
            decision_route: [2; 32],
        },
        operations,
        progress: None,
    }
}

fn create(writer: &mut RelWriter, fingerprint: u8, draft: ObservationDraft) -> ObservationId {
    let batch = batch(
        writer,
        fingerprint,
        vec![ObservationOperation::Create { draft }],
    );
    writer.publish_observation_batch(batch).unwrap().revisions[0].0
}

#[test]
fn observation_dependency_review_and_suffix_replay_preserve_exact_history() {
    let directory = tempfile::tempdir().unwrap();
    let owner = directory.path().join("claim.rel");
    let cache = directory.path().join("private");
    private_cache::test_directory(&cache).unwrap();
    let slot = cache.join("claim.checkpoint");
    let mut writer = RelWriter::create_current_path(&owner, OWNER).unwrap();

    let evidence = source(&mut writer, "source", "Hélen accepts office purchases.");
    let a = create(
        &mut writer,
        1,
        draft("Office purchases are accepted.", vec![evidence]),
    );
    let cite_a = writer
        .session
        .observation(a)
        .unwrap()
        .revision
        .full_citation();
    let b = create(
        &mut writer,
        2,
        draft("Office has a purchasing workflow.", vec![evidence, cite_a]),
    );
    let cite_b = writer
        .session
        .observation(b)
        .unwrap()
        .revision
        .full_citation();
    let c = create(
        &mut writer,
        3,
        draft("Office purchase steps can be documented.", vec![cite_b]),
    );
    assert_eq!(
        writer
            .session
            .observation(c)
            .unwrap()
            .revision
            .lineage
            .len(),
        1
    );
    assert_eq!(writer.session.observation_freshness(a), Some(100));
    let earlier = writer.session.current_historical_cut();
    let earlier_proof = writer.session.historical_proof(OWNER, &earlier).unwrap();
    drop(writer);

    let (lease, exact, checkpoint) = reopen_at_checkpoint(&owner, &slot);
    let covered = lease.extent().unwrap();
    assert_eq!(checkpoint.unwrap().attempted_extent, Some(covered));
    assert_eq!(
        exact.historical_proof(OWNER, &earlier).unwrap(),
        earlier_proof
    );
    drop((exact, lease));
    let (lease, exact, checkpoint) = reopen_at_checkpoint(&owner, &slot);
    assert_eq!(checkpoint.unwrap().attempted_extent, Some(covered));
    assert_eq!(
        exact.historical_proof(OWNER, &earlier).unwrap(),
        earlier_proof
    );
    drop((exact, lease));

    let mut writer = open_writer(&owner);
    let new_evidence = source(
        &mut writer,
        "new-evidence",
        "Purchases below one hundred are accepted.",
    );
    let pending = batch(
        &writer,
        4,
        vec![ObservationOperation::Pending {
            id: a,
            expected_revision: 1,
            reason: ObservationPendingReason::NewEvidence,
            evidence: vec![new_evidence],
        }],
    );
    writer.publish_observation_batch(pending).unwrap();
    assert_eq!(
        writer.session.observation(c).unwrap().standing,
        ObservationStanding::Pending(ObservationPendingReason::UpstreamReview)
    );
    assert!(writer.session.observation_citation_text(&cite_b).is_err());
    assert_eq!(
        writer
            .session
            .historical_observation_citation_text(&cite_b)
            .unwrap(),
        "Office has a purchasing workflow."
    );

    let reaffirm = batch(
        &writer,
        5,
        vec![ObservationOperation::Reaffirm {
            id: a,
            expected_revision: 1,
            reviewed: vec![new_evidence],
        }],
    );
    writer.publish_observation_batch(reaffirm).unwrap();
    assert_eq!(
        writer.session.observation(c).unwrap().standing,
        ObservationStanding::Current
    );

    let reject = batch(
        &writer,
        6,
        vec![ObservationOperation::Pending {
            id: a,
            expected_revision: 1,
            reason: ObservationPendingReason::NewEvidence,
            evidence: vec![ObservationCitation {
                start_byte: 2,
                ..evidence
            }],
        }],
    );
    assert!(writer.publish_observation_batch(reject).is_err());

    let pending = batch(
        &writer,
        7,
        vec![ObservationOperation::Pending {
            id: a,
            expected_revision: 1,
            reason: ObservationPendingReason::NewEvidence,
            evidence: vec![new_evidence],
        }],
    );
    writer.publish_observation_batch(pending).unwrap();
    let mut qualified = draft(
        "Office purchases below one hundred are accepted.",
        vec![new_evidence],
    );
    qualified.reviewed = vec![new_evidence];
    let successor = batch(
        &writer,
        8,
        vec![ObservationOperation::Successor {
            id: a,
            expected_revision: 1,
            draft: qualified,
            change: ObservationChange::Qualification,
            credited_activity: None,
        }],
    );
    let receipt = writer.publish_observation_batch(successor.clone()).unwrap();
    assert_eq!(receipt.revisions, vec![(a, 2)]);
    assert_eq!(
        writer.session.observation(b).unwrap().standing,
        ObservationStanding::Pending(ObservationPendingReason::ChangedSupport)
    );
    assert_eq!(
        writer.session.observation(c).unwrap().standing,
        ObservationStanding::Pending(ObservationPendingReason::UpstreamReview)
    );
    let latest = writer.session.current_historical_cut();
    let latest_proof = writer.session.historical_proof(OWNER, &latest).unwrap();
    drop(writer);

    let (lease, warm, checkpoint) = reopen_at_checkpoint(&owner, &slot);
    assert_eq!(checkpoint.unwrap().attempted_extent, Some(covered));
    assert_eq!(warm.historical_proof(OWNER, &latest).unwrap(), latest_proof);
    assert_eq!(
        warm.observation_revision(a, 1).unwrap().standing,
        ObservationStanding::Superseded
    );
    assert_eq!(
        warm.observation_revision(a, 2)
            .unwrap()
            .revision
            .draft
            .citations,
        vec![new_evidence]
    );
    drop((lease, warm));

    let mut writer = open_writer(&owner);
    assert_eq!(
        writer.publish_observation_batch(successor).unwrap(),
        receipt
    );
    drop(writer);

    let copy = directory.path().join("claim-copy.rel");
    VerifiedOwnerCopyPlan::prepare(&owner)
        .unwrap()
        .copy_to(&owner, &copy)
        .unwrap();
    let reopened = RelSession::open_path_current(&copy).unwrap();
    assert_eq!(
        reopened.historical_proof(OWNER, &earlier).unwrap(),
        earlier_proof
    );
    assert_eq!(
        reopened.historical_proof(OWNER, &latest).unwrap(),
        latest_proof
    );
    assert_eq!(reopened.observation_freshness(a), Some(100));
}

#[test]
fn unclocked_observation_cannot_be_admitted_as_an_ordinary_rel_frame() {
    let directory = tempfile::tempdir().unwrap();
    let owner = directory.path().join("bad.rel");
    let mut writer = RelWriter::create_current_path(&owner, OWNER).unwrap();
    writer.container.append(observations::MARKER).unwrap();
    writer.container.sync().unwrap();
    drop(writer);
    assert!(RelSession::open_path_current(&owner).is_err());
}

#[test]
fn observation_freshness_uses_the_largest_match_and_credits_only_a_verified_upgrade() {
    let directory = tempfile::tempdir().unwrap();
    let owner = directory.path().join("awards.rel");
    let mut writer = RelWriter::create_current_path(&owner, OWNER).unwrap();
    let support = source(&mut writer, "support", "The team approves orders.");
    let id = create(
        &mut writer,
        1,
        draft("The team approves orders.", vec![support]),
    );
    let qualifying = source(
        &mut writer,
        "qualifying",
        "The team approves orders below one hundred.",
    );
    let ObservationSource::Memory(source) = qualifying.source else {
        unreachable!()
    };
    let activity = ObservationActivity { turn: 2, source };
    for number in 0..250 {
        let key = format!("clock-{number}");
        writer
            .session
            .archive
            .accept_turn(
                &mut writer.container,
                TurnInput {
                    id: key,
                    conversation_id: "accepted".into(),
                    parent_id: None,
                    role: "assistant".into(),
                    principal_id: Some("owner".into()),
                    timestamp_ns: 1,
                    content: "Accounting clock activity".into(),
                    attachments: Vec::new(),
                    project_attachments: Vec::new(),
                },
            )
            .unwrap();
    }
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    assert_eq!(writer.session.observation_freshness(id), Some(75));
    let award = |amount| ObservationOperation::SemanticAward {
        id,
        expected_revision: 1,
        activity,
        evidence: qualifying,
        amount,
    };
    let batch1 = batch(&writer, 2, vec![award(2), award(6), award(6)]);
    let result = writer.publish_observation_batch(batch1).unwrap();
    assert_eq!(
        result.awards,
        vec![ObservationAwardReceipt {
            id,
            activity,
            previous: 0,
            total: 6
        }]
    );
    assert_eq!(writer.session.observation_freshness(id), Some(81));

    let comparison = ObservationComparison {
        id,
        target_revision: 1,
        citation: qualifying,
        outcome: ObservationComparisonOutcome::MaterialQualification,
    };
    let compare = ObservationOperation::Comparison {
        comparison: comparison.clone(),
    };
    let invalid = batch(&writer, 3, vec![compare.clone()]);
    assert!(writer.publish_observation_batch(invalid).is_err());
    assert!(writer.session.observation_comparison(&comparison).is_none());

    let pending = ObservationOperation::Pending {
        id,
        expected_revision: 1,
        reason: ObservationPendingReason::NewEvidence,
        evidence: vec![qualifying],
    };
    let trigger = batch(&writer, 4, vec![compare, pending]);
    writer.publish_observation_batch(trigger).unwrap();
    assert_eq!(writer.session.observation_freshness(id), Some(81));
    assert_eq!(writer.session.observation_pending_after(None, 5).len(), 1);
    let successor = batch(
        &writer,
        5,
        vec![ObservationOperation::Successor {
            id,
            expected_revision: 1,
            draft: draft(
                "The team approves orders below one hundred.",
                vec![qualifying],
            ),
            change: ObservationChange::Qualification,
            credited_activity: Some(activity),
        }],
    );
    let accepted = writer.publish_observation_batch(successor.clone()).unwrap();
    assert_eq!(
        accepted.awards,
        vec![ObservationAwardReceipt {
            id,
            activity,
            previous: 6,
            total: 25
        }]
    );
    assert_eq!(writer.session.observation_freshness(id), Some(100));
    drop(writer);

    let mut writer = open_writer(&owner);
    assert_eq!(writer.session.observation_award(id, activity), Some(25));
    assert_eq!(writer.session.observation_freshness(id), Some(100));
    assert_eq!(
        writer.publish_observation_batch(successor).unwrap(),
        accepted
    );
    assert_eq!(
        writer.session.observation_comparison(&comparison),
        Some(ObservationComparisonOutcome::MaterialQualification)
    );

    let correction = self::source(
        &mut writer,
        "correction",
        "The contractor approves orders. The team never approves them.",
    );
    let ObservationSource::Memory(source) = correction.source else {
        unreachable!()
    };
    let corrected_activity = ObservationActivity {
        turn: writer.session.rel_turn_count(),
        source,
    };
    let trigger = batch(
        &writer,
        6,
        vec![
            ObservationOperation::SemanticAward {
                id,
                expected_revision: 2,
                activity: corrected_activity,
                evidence: correction,
                amount: 6,
            },
            ObservationOperation::Pending {
                id,
                expected_revision: 2,
                reason: ObservationPendingReason::NewEvidence,
                evidence: vec![correction],
            },
        ],
    );
    writer.publish_observation_batch(trigger).unwrap();
    let corrected = batch(
        &writer,
        7,
        vec![ObservationOperation::Successor {
            id,
            expected_revision: 2,
            draft: draft("The contractor approves orders.", vec![correction]),
            change: ObservationChange::Correction,
            credited_activity: Some(corrected_activity),
        }],
    );
    let accepted = writer.publish_observation_batch(corrected.clone()).unwrap();
    assert_eq!(
        accepted.awards,
        vec![ObservationAwardReceipt {
            id,
            activity: corrected_activity,
            previous: 6,
            total: 50,
        }]
    );
    assert_eq!(writer.session.observation_freshness(id), Some(100));
    assert_eq!(
        writer.publish_observation_batch(corrected).unwrap(),
        accepted
    );
}

#[test]
fn clocked_observation_rejects_archive_evidence_from_outside_its_prefix() {
    use sha2::{Digest, Sha256};

    let directory = tempfile::tempdir().unwrap();
    let owner = directory.path().join("ahead.rel");
    let mut writer = RelWriter::create_current_path(&owner, OWNER).unwrap();
    let mut batch = batch(&writer, 1, vec![]);
    let bytes = vec![5];
    batch.progress = Some(ObservationAuditProgress {
        previous: None,
        fingerprint: Sha256::digest(&bytes).into(),
        bytes,
    });
    let record = ObservationRecord {
        batch,
        archive_cut: 1,
        rel_turn: 0,
        memory_cut: 0,
        perception_cut: 0,
        lineages: vec![],
    };
    let mut body = observations::MARKER.to_vec();
    body.extend(rmp_serde::to_vec_named(&record).unwrap());
    writer
        .container
        .append_publication_envelope(&body, None)
        .unwrap();
    writer.container.sync().unwrap();
    drop(writer);
    assert!(RelSession::open_path_current(&owner).is_err());
}
