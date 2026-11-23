use super::*;
use crate::{
    VerifiedOwnerCopyPlan, reclaim_storage_copy, reconcile_divergent_rel_copy,
    rewrite_memory_history_copy,
};
use reliquary_memory_core::{MemoryDraft, MemorySourceRef};
use reliquary_perception_store::identity::{
    IdentityCitation, IdentityEntityRevision, IdentityMemoryRef, IdentityRelationshipRef,
    IdentitySpan,
};
use reliquary_perception_store::{Entity, EntityDraft, MemoryTextField};
use sha2::{Digest, Sha256};
use std::fs;

const OWNER: [u8; 16] = [23; 16];
const PEER: [u8; 16] = [24; 16];
const QUOTE: &str = "Mira signed the Lantern contract.";

pub(in crate::session) fn fixture_entity(writer: &mut RelWriter, name: &str) -> Entity {
    let entity = writer
        .session
        .common
        .perception
        .publish_entity(
            &mut writer.container,
            EntityDraft {
                canonical_name: name.into(),
                aliases: vec![],
                kind: "referent".into(),
                summary: format!("{name} is source-bound."),
                mutation_id: format!("fixture-entity-{name}"),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
        )
        .unwrap();
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    entity
}

pub(in crate::session) fn unresolved(owner: [u8; 16], entity: &Entity) -> IdentityReceipt {
    let evidence = IdentityEvidenceRef {
        endpoint_revision: entity.revision,
        relationships: vec![],
        limit: None,
    };
    IdentityReceipt {
        pair: IdentityPair {
            local_owner: owner,
            local_entity: entity.id,
            peer_owner: PEER,
            peer_entity: entity.id,
        },
        revision: 1,
        predecessor: None,
        fingerprint: [1; 32],
        policy: 1,
        route: "grounding-v1".into(),
        left: evidence.clone(),
        right: evidence,
        outcome: IdentityOutcome::Unresolved,
        reason: IdentityReason::InsufficientEvidence,
        citations: vec![],
    }
}

fn distinct_successor(previous: &IdentityReceipt, fingerprint: [u8; 32]) -> IdentityReceipt {
    let mut next = previous.clone();
    next.predecessor = Some(previous.revision);
    next.revision += 1;
    next.fingerprint = fingerprint;
    next
}

fn admission() -> GraphAdmission {
    GraphAdmission::for_replay(OWNER, [], [], [], [], [])
}

fn reopen(path: &Path) -> RelSession {
    RelSession::open_path(path, &admission(), [], []).unwrap()
}

fn assert_chain(session: &RelSession, expected: &[IdentityReceipt]) {
    let pair = expected[0].pair;
    assert_eq!(session.owner_id(), OWNER);
    assert_eq!(session.identity_pairs_after(None, 1), [pair]);
    assert!(session.identity_pairs_after(Some(&pair), 1).is_empty());
    assert_eq!(session.identity_receipt(&pair, None), expected.last());
    for receipt in expected {
        assert_eq!(
            session.identity_receipt(&pair, Some(receipt.revision)),
            Some(receipt)
        );
    }
}

#[test]
fn clocked_chain_reopens_in_copies_and_refuses_unsupported_history_rewrites() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.rel");
    let prefix = directory.path().join("prefix.rel");
    let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
    let entity = fixture_entity(&mut writer, "Mira");
    let first = unresolved(OWNER, &entity);
    drop(writer);

    let before = fs::read(&source).unwrap();
    let empty = reopen(&source);
    assert!(empty.identity_pairs_after(None, 16).is_empty());
    let empty_cut = empty.current_historical_cut();
    let empty_proof = empty.historical_proof(OWNER, &empty_cut).unwrap();
    drop(empty);
    assert_eq!(fs::read(&source).unwrap(), before);

    let mut writer = RelWriter::open_path(&source, admission(), [], []).unwrap();
    assert_eq!(
        writer.publish_identity_receipt(first.clone()).unwrap(),
        first
    );
    let after_first = writer.container.extent().unwrap();
    assert_eq!(
        writer.publish_identity_receipt(first.clone()).unwrap(),
        first
    );
    assert_eq!(writer.container.extent().unwrap(), after_first);
    drop(writer);
    let plan = VerifiedOwnerCopyPlan::prepare(&source).unwrap();
    plan.copy_to(&source, &prefix).unwrap();
    assert_chain(&reopen(&prefix), std::slice::from_ref(&first));

    let second = distinct_successor(&first, [2; 32]);
    let third = distinct_successor(&second, first.fingerprint);
    let mut writer = RelWriter::open_path(&source, admission(), [], []).unwrap();
    let mut wrong = second.clone();
    wrong.predecessor = None;
    assert!(writer.publish_identity_receipt(wrong).is_err());
    assert_eq!(writer.container.extent().unwrap(), after_first);
    writer.publish_identity_receipt(second.clone()).unwrap();
    writer.publish_identity_receipt(third.clone()).unwrap();
    drop(writer);

    let newest = reopen(&source);
    let current_cut = newest.current_historical_cut();
    let current_proof = newest.historical_proof(OWNER, &current_cut).unwrap();
    assert_eq!(
        newest.historical_proof(OWNER, &empty_cut).unwrap(),
        empty_proof
    );
    assert_ne!(current_proof.digest, empty_proof.digest);
    assert_eq!(
        current_proof.retained_records,
        empty_proof.retained_records + 3
    );
    let expected = [first.clone(), second, third];
    assert_chain(&newest, &expected);
    drop(newest);

    let (extension, right_wins) =
        VerifiedOwnerCopyPlan::prepare_prefix_reconciliation(&prefix, &source).unwrap();
    assert!(right_wins);
    let copied = directory.path().join("extended.rel");
    extension.copy_to(&source, &copied).unwrap();
    let copied_session = reopen(&copied);
    assert_chain(&copied_session, &expected);
    assert_eq!(
        copied_session
            .historical_proof(OWNER, &current_cut)
            .unwrap(),
        current_proof
    );

    // The left and right sources already share receipt 1 when they diverge.
    let mut writer = RelWriter::open_path(&prefix, admission(), [], []).unwrap();
    writer
        .publish_identity_receipt(distinct_successor(&first, [9; 32]))
        .unwrap();
    drop(writer);
    let refused = directory.path().join("refused.rel");
    let error = reconcile_divergent_rel_copy(&source, &prefix, &refused, &admission(), &[], &[])
        .unwrap_err();
    assert!(error.to_string().contains("identity receipts"));
    assert!(!refused.exists());
    let request = FreshHistoryRequest {
        source_owner_id: OWNER,
        source_digest: Sha256::digest(fs::read(&source).unwrap()).into(),
        plan_digest: [1; 32],
        identity: FreshHistoryIdentity::PreserveOwner,
        retained_memory_ids: vec![],
        retain_all_revisions: false,
        retain_revisions_since_memory_version: None,
        retain_derived_state: false,
        archive: FreshHistoryArchive::Omit,
        freshness: FreshHistoryFreshness::Reset,
        profile_ids: vec![],
    };
    let error = rewrite_memory_history_copy(&source, &refused, &request, &admission()).unwrap_err();
    assert!(error.contains("identity receipts"));
    assert!(!refused.exists());
}

#[test]
fn full_replay_rejects_bad_predecessor_and_future_local_entity() {
    let directory = tempfile::tempdir().unwrap();
    for case in ["predecessor", "future Entity", "top-level"] {
        let source = directory.path().join(format!("{case}.rel"));
        let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
        let entity = fixture_entity(&mut writer, "Mira");
        let mut receipt = unresolved(OWNER, &entity);
        let body = match case {
            "predecessor" => {
                receipt.predecessor = Some(1);
                receipt.revision = 2;
                encode(&receipt).unwrap()
            }
            "future Entity" => {
                receipt.left.endpoint_revision = entity.revision + 1;
                encode(&receipt).unwrap()
            }
            _ => encode(&receipt).unwrap(),
        };
        if case == "top-level" {
            writer.container.append(&body).unwrap();
        } else {
            writer
                .container
                .append_publication_envelope(&body, None)
                .unwrap();
        }
        if case == "future Entity" {
            let updated = writer
                .session
                .common
                .perception
                .update_entity(
                    &mut writer.container,
                    entity.id,
                    entity.revision,
                    EntityDraft {
                        canonical_name: entity.canonical_name,
                        aliases: entity.aliases,
                        kind: entity.kind,
                        summary: entity.summary,
                        mutation_id: "after-forged-receipt".into(),
                        created_at_ns: entity.created_at_ns,
                        updated_at_ns: 2,
                    },
                )
                .unwrap();
            assert_eq!(updated.revision, receipt.left.endpoint_revision);
        }
        writer.container.sync().unwrap();
        drop(writer);
        let error = RelSession::open_path(&source, &admission(), [], [])
            .err()
            .unwrap();
        let reason = match case {
            "predecessor" => "revision chain",
            "future Entity" => "local Entity revision outside the publication prefix",
            _ => "missing allocating publication envelope",
        };
        assert!(error.to_string().contains(reason), "{error}");
    }
}

#[test]
fn tail_recovery_retains_only_the_complete_chain() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("interrupted.rel");
    let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
    let entity = fixture_entity(&mut writer, "Mira");
    let first = unresolved(OWNER, &entity);
    writer.publish_identity_receipt(first.clone()).unwrap();
    let second = distinct_successor(&first, [2; 32]);
    writer.publish_identity_receipt(second.clone()).unwrap();
    drop(writer);

    let complete = fs::read(&source).unwrap();
    let shorter = complete.len() as u64 - 8;
    let file = fs::OpenOptions::new().write(true).open(&source).unwrap();
    file.set_len(shorter).unwrap();
    file.sync_all().unwrap();
    drop(file);
    assert!(RelSession::open_path(&source, &admission(), [], []).is_err());

    let mut repair = Container::open_for_repair(&source, FileKind::Reliquary).unwrap();
    let plan = repair.plan_tail_repair().unwrap().unwrap();
    repair.repair_incomplete_tail(plan).unwrap();
    drop(repair);
    assert_chain(&reopen(&source), &[first.clone()]);
    let mut writer = RelWriter::open_path(&source, admission(), [], []).unwrap();
    writer.publish_identity_receipt(second.clone()).unwrap();
    drop(writer);
    assert_chain(&reopen(&source), &[first, second]);
    assert_eq!(fs::read(source).unwrap(), complete);
}

#[test]
fn grounded_receipt_survives_description_pruning_and_reclamation() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.rel");
    let pruned = directory.path().join("pruned.rel");
    let reclaimed = directory.path().join("reclaimed.rel");
    let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
    let endpoint = fixture_entity(&mut writer, "Mira");
    let landmark = fixture_entity(&mut writer, "Lantern");
    let owner = crate::session::perception_lifecycle::support::owner_hex(&OWNER);
    let memory = writer
        .session
        .common
        .memories
        .publish_versioned_creation(
            &mut writer.container,
            MemoryDraft {
                title: "Accepted source".into(),
                content: QUOTE.into(),
                category: "fact".into(),
                memory_type: "project".into(),
                authority_kind: "direct".into(),
                temporal_status: "current".into(),
                scope: "project".into(),
                lifecycle_state: "current".into(),
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
                    owner_id: owner,
                    principal_id: Some("principal-a".into()),
                    source_episode_id: [7; 32],
                    source_node_id: "accepted-turn".into(),
                    content_source_conversation_id: Some("conversation".into()),
                    content_source_node_id: Some("accepted-turn".into()),
                    grounding_source_conversation_id: None,
                    grounding_source_node_id: None,
                }),
                source_time_ns: Some(10),
                temporal_inference: None,
                mutation_id: "source-memory".into(),
                created_at_ns: 10,
                updated_at_ns: 10,
            },
            10,
        )
        .unwrap()
        .revision
        .record;
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    let mut participants = [&endpoint, &landmark].map(|entity| RelationshipParticipantEvidence {
        entity_id: entity.id,
        entity_revision: entity.revision,
        role: None,
    });
    participants.sort_by_key(|participant| participant.entity_id);
    let relationship = writer
        .create_relationship(CreateRelationshipRequest {
            owner_id: OWNER,
            kind: "signed".into(),
            participants: participants.to_vec(),
            source_memory_id: memory.id,
            source_memory_revision: memory.revision,
            source_body_id: memory.body_id,
            source_field: MemoryTextField::Content,
            start_byte: 0,
            end_byte: QUOTE.len(),
            source_quote: QUOTE.into(),
            mutation_id: "receipt-relationship".into(),
            updated_at_ns: 11,
        })
        .unwrap();
    let mut receipt = unresolved(OWNER, &endpoint);
    receipt.left.relationships.push(IdentityRelationshipRef {
        id: relationship.id,
        revision: relationship.revision,
        participants: participants
            .iter()
            .map(|participant| IdentityEntityRevision {
                id: participant.entity_id,
                revision: participant.entity_revision,
            })
            .collect(),
        memories: vec![IdentityMemoryRef {
            id: memory.id.0,
            revision: memory.revision,
            body_id: memory.body_id.0,
            span: Some(IdentitySpan {
                field: MemoryTextField::Content,
                start_byte: 0,
                end_byte: QUOTE.len() as u32,
            }),
        }],
    });
    receipt.right = receipt.left.clone();
    receipt.route = "a".repeat(64);
    receipt.outcome = IdentityOutcome::SameIdentity;
    receipt.reason = IdentityReason::Corroborated;
    receipt.citations = [IdentitySide::Left, IdentitySide::Right]
        .map(|side| IdentityCitation {
            side,
            relationship: 0,
            memory: 0,
        })
        .to_vec();
    let mut invalid = receipt.clone();
    invalid.left.relationships[0].memories[0]
        .span
        .as_mut()
        .unwrap()
        .start_byte += 1;
    let extent = writer.container.extent().unwrap();
    let error = writer.publish_identity_receipt(invalid).unwrap_err();
    assert!(error.to_string().contains("local witness"));
    assert_eq!(writer.container.extent().unwrap(), extent);
    writer.publish_identity_receipt(receipt.clone()).unwrap();
    writer
        .retire_relationship(RelationshipRetirementRequest {
            owner_id: OWNER,
            relationship_id: relationship.id,
            relationship_revision: relationship.revision,
            source_memory_id: memory.id,
            source_memory_revision: memory.revision,
            source_body_id: memory.body_id,
            source_field: MemoryTextField::Content,
            start_byte: 0,
            end_byte: QUOTE.len(),
            source_quote: QUOTE.into(),
        })
        .unwrap();
    assert!(writer.publish_identity_receipt(receipt.clone()).is_err());

    let next = IdentityReceipt {
        revision: 2,
        predecessor: Some(1),
        fingerprint: [2; 32],
        ..unresolved(OWNER, &endpoint)
    };
    writer.publish_identity_receipt(next.clone()).unwrap();
    drop(writer);

    // Ensure reclamation actually rewrites backing while retaining the receipt frames.
    let mut container = Container::open_writer(&source).unwrap();
    let orphan = vec![0x5a; 512];
    let mut frame = b"RELFILE1".to_vec();
    frame.extend_from_slice(&Sha256::digest(&orphan));
    frame.extend_from_slice(&(orphan.len() as u32).to_le_bytes());
    frame.extend_from_slice(&orphan);
    container.append(&frame).unwrap();
    container.sync().unwrap();
    drop(container);

    let expected = [receipt, next];
    let plan = VerifiedOwnerCopyPlan::prepare(&source).unwrap();
    prune_terminal_relationship_descriptions_copy(
        &plan,
        &source,
        &pruned,
        &[relationship.id],
        &admission(),
    )
    .unwrap();
    let pruned_session = reopen(&pruned);
    assert_chain(&pruned_session, &expected);
    assert!(
        pruned_session
            .relationship_revision(relationship.id, 1)
            .unwrap()
            .summary
            .is_empty()
    );

    let report = reclaim_storage_copy(&plan, &source, &reclaimed, &admission()).unwrap();
    assert_eq!(report.removed_frames, 1);
    let before = reopen(&source);
    let cut = before.current_historical_cut();
    let after = reopen(&reclaimed);
    assert_chain(&after, &expected);
    assert_eq!(
        before.historical_proof(OWNER, &cut).unwrap(),
        after.historical_proof(OWNER, &cut).unwrap()
    );
}
