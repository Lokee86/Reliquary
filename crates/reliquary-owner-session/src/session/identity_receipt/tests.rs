use super::*;
use crate::{
    VerifiedOwnerCopyPlan, reclaim_storage_copy, reconcile_divergent_rel_copy,
    rewrite_memory_history_copy,
};
use reliquary_memory_core::{MemoryDraft, MemorySourceRef};
use reliquary_perception_store::identity::IdentityEntityRevision;
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
        mentions: vec![],
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

fn fixture_source_memory(writer: &mut RelWriter) -> reliquary_memory_core::MemoryRecord {
    fixture_source_memory_with_content(writer, QUOTE)
}

fn fixture_source_memory_with_content(
    writer: &mut RelWriter,
    content: &str,
) -> reliquary_memory_core::MemoryRecord {
    let owner = crate::session::perception_lifecycle::support::owner_hex(&OWNER);
    let memory = writer
        .session
        .common
        .memories
        .publish_versioned_creation(
            &mut writer.container,
            MemoryDraft {
                title: "Accepted source".into(),
                content: content.into(),
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
    memory
}

fn fixture_mention_reference(
    memory: &reliquary_memory_core::MemoryRecord,
    retained_ordinal: Option<u64>,
) -> IdentityMentionRef {
    IdentityMentionRef {
        key: MemoryEntityMentionKey {
            memory_id: memory.id.0,
            field: MemoryTextField::Content,
            start_byte: 0,
            end_byte: 4,
        },
        revision: 1,
        memory: IdentityMemoryRef {
            id: memory.id.0,
            revision: memory.revision,
            body_id: memory.body_id.0,
            span: Some(IdentitySpan {
                field: MemoryTextField::Content,
                start_byte: 0,
                end_byte: QUOTE.len() as u32,
            }),
        },
        retained_ordinal,
    }
}

fn fixture_retained_resolution(
    writer: &RelWriter,
    reference: &IdentityMentionRef,
    entity: &Entity,
) -> Vec<u8> {
    let mut row = b"RELERH01".to_vec();
    row.extend_from_slice(&OWNER);
    row.extend_from_slice(&writer.container.latest_version().to_le_bytes());
    row.extend_from_slice(&reference.retained_ordinal.unwrap().to_le_bytes());
    row.extend_from_slice(&reference.key.memory_id);
    row.push(2); // Content
    row.extend_from_slice(&reference.key.start_byte.to_le_bytes());
    row.extend_from_slice(&reference.key.end_byte.to_le_bytes());
    row.extend_from_slice(&reference.revision.to_le_bytes());
    row.extend_from_slice(&11i64.to_le_bytes());
    row.push(1); // Resolved
    row.extend_from_slice(&entity.id.0);
    row.push(3); // NamedReferent
    row
}

fn fixture_mention(
    writer: &mut RelWriter,
    memory: &reliquary_memory_core::MemoryRecord,
    entity: &Entity,
) -> MemoryEntityResolution {
    fixture_mention_at(writer, memory, entity, 0)
}

fn fixture_mention_at(
    writer: &mut RelWriter,
    memory: &reliquary_memory_core::MemoryRecord,
    entity: &Entity,
    offset: u32,
) -> MemoryEntityResolution {
    let mut key = fixture_mention_reference(memory, None).key;
    key.start_byte += offset;
    key.end_byte += offset;
    writer
        .commit_entity_mention(EntityMentionCommit {
            owner_id: OWNER,
            memory_revision: memory.revision,
            body_id: memory.body_id,
            key,
            surface: "Mira".into(),
            expected_revision: 0,
            updated_at_ns: 11,
            status: MemoryEntityResolutionStatus::Resolved {
                entity_id: entity.id,
                reason: EntityResolutionReason::NamedReferent,
            },
            target_revisions: vec![(entity.id, entity.revision)],
            new_entity: None,
        })
        .unwrap()
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
    let memory = fixture_source_memory(&mut writer);
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
        .map(|side| IdentityCitation::Relationship {
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

#[test]
fn prepared_proof_rechecks_cut_and_exact_references_before_publication() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.rel");
    let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
    let endpoint = fixture_entity(&mut writer, "Mira");
    let receipt = unresolved(OWNER, &endpoint);
    let (_, stale) = writer
        .read_session()
        .identity_evidence_with_proof(endpoint.id)
        .unwrap()
        .unwrap();
    fixture_entity(&mut writer, "Lantern");
    let extent = writer.container.extent().unwrap();
    assert!(
        writer
            .publish_prepared_identity_receipt(receipt.clone(), &stale)
            .is_err()
    );
    let (_, proof) = writer
        .read_session()
        .identity_evidence_with_proof(endpoint.id)
        .unwrap()
        .unwrap();

    let mut forged = receipt.clone();
    forged.left.endpoint_revision += 1;
    assert!(
        writer
            .publish_prepared_identity_receipt(forged, &proof)
            .is_err()
    );
    assert_eq!(writer.container.extent().unwrap(), extent);

    let budget = IdentityReceipt {
        left: IdentityEvidenceRef {
            limit: Some(IdentityLimit::Budget),
            ..receipt.left.clone()
        },
        right: IdentityEvidenceRef {
            limit: Some(IdentityLimit::Budget),
            ..receipt.right.clone()
        },
        reason: IdentityReason::EvidenceLimit,
        ..receipt
    };
    assert_eq!(
        writer
            .publish_prepared_identity_receipt(budget.clone(), &proof)
            .unwrap(),
        budget
    );
    drop(writer);
    assert_chain(&reopen(&source), &[budget]);
}

#[test]
fn legacy_relationship_citations_and_evidence_keep_their_named_encoding() {
    let legacy_citation = b"\x83\xa4side\xa4Left\xacrelationship\x02\xa6memory\x03";
    let citation = IdentityCitation::Relationship {
        side: IdentitySide::Left,
        relationship: 2,
        memory: 3,
    };
    assert_eq!(rmp_serde::to_vec_named(&citation).unwrap(), legacy_citation);
    assert_eq!(
        rmp_serde::from_slice::<IdentityCitation>(legacy_citation).unwrap(),
        citation
    );

    let legacy_evidence = b"\x83\xb1endpoint_revision\x01\xadrelationships\x90\xa5limit\xc0";
    let evidence = IdentityEvidenceRef {
        endpoint_revision: 1,
        relationships: vec![],
        limit: None,
        mentions: vec![],
    };
    assert_eq!(rmp_serde::to_vec_named(&evidence).unwrap(), legacy_evidence);
    assert_eq!(
        rmp_serde::from_slice::<IdentityEvidenceRef>(legacy_evidence).unwrap(),
        evidence
    );
    assert_eq!(
        serde_json::to_value(&evidence).unwrap(),
        serde_json::json!({"endpoint_revision": 1, "relationships": [], "limit": null})
    );
}

#[test]
fn discovery_pages_seek_and_resolved_context_follows_withdrawal() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("discovery.rel");
    let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
    let endpoint = fixture_entity(&mut writer, "Mira");
    let other = fixture_entity(&mut writer, "Different endpoint");
    let content =
        "Setup.\ncom.arden.editor confirms Mira signed contract M42-7816 under ticket abc-123.";
    let memory = fixture_source_memory_with_content(&mut writer, content);
    let session = writer.read_session();
    let before = session.identity_discovery_keys(None, 4).unwrap();
    assert_eq!(before.len(), 2);
    assert!(before[0].0 < before[1].0);
    assert!(before.iter().all(|(_, keys)| keys.len() == 1));
    assert_eq!(
        session.identity_discovery_keys(None, 1).unwrap(),
        before[..1]
    );
    assert_eq!(
        session
            .identity_discovery_keys(Some(before[0].0), 1)
            .unwrap(),
        before[1..]
    );

    let offset = content.find("Mira").unwrap().try_into().unwrap();
    let mention = fixture_mention_at(&mut writer, &memory, &endpoint, offset);
    let session = writer.read_session();
    let resolved = session.identity_discovery_keys(None, 4).unwrap();
    let count = |id| resolved.iter().find(|row| row.0 == id).unwrap().1.len();
    assert_eq!(count(endpoint.id), 4);
    assert_eq!(count(other.id), 1);

    writer
        .withdraw_entity_mention(WithdrawEntityMentionRequest {
            owner_id: OWNER,
            key: mention.key,
            expected_mention_revision: mention.revision,
            memory_id: memory.id,
            memory_revision: memory.revision,
            body_id: memory.body_id,
            source_surface: "Mira".into(),
            target_revisions: vec![(endpoint.id, endpoint.revision)],
            withdrawn_at_ns: 12,
        })
        .unwrap();
    assert_eq!(
        writer
            .read_session()
            .identity_discovery_keys(None, 4)
            .unwrap(),
        before
    );
    drop(writer);
    assert_eq!(
        reopen(&source).identity_discovery_keys(None, 4).unwrap(),
        before
    );
}

#[test]
fn direct_mention_receipt_binds_its_excerpt_and_historical_resolution() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("mention.rel");
    let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
    let endpoint = fixture_entity(&mut writer, "Mira");
    let memory = fixture_source_memory(&mut writer);
    let mention = fixture_mention(&mut writer, &memory, &endpoint);

    let (basis, proof) = writer
        .read_session()
        .identity_evidence_with_proof(endpoint.id)
        .unwrap()
        .unwrap();
    let mut receipt = unresolved(OWNER, &endpoint);
    receipt.left = basis.references();
    assert_eq!(receipt.left.mentions.len(), 1);
    receipt.right = receipt.left.clone();
    receipt.route = "a".repeat(64);
    receipt.outcome = IdentityOutcome::SameIdentity;
    receipt.reason = IdentityReason::Corroborated;
    receipt.citations = [IdentitySide::Left, IdentitySide::Right]
        .map(|side| IdentityCitation::Mention { side, mention: 0 })
        .to_vec();
    let original = receipt.left.mentions[0];
    assert_eq!(original.key, mention.key);
    assert_eq!(original.revision, mention.revision);

    let mut forged = receipt.clone();
    forged.left.mentions[0]
        .memory
        .span
        .as_mut()
        .unwrap()
        .end_byte -= 1;
    let extent = writer.container.extent().unwrap();
    let error = writer
        .publish_prepared_identity_receipt(forged.clone(), &proof)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("prepared local evidence changed")
    );

    let error = writer.publish_identity_receipt(forged).unwrap_err();
    assert!(error.to_string().contains("local mention excerpt"));
    assert_eq!(writer.container.extent().unwrap(), extent);
    writer
        .publish_prepared_identity_receipt(receipt.clone(), &proof)
        .unwrap();

    writer
        .withdraw_entity_mention(WithdrawEntityMentionRequest {
            owner_id: OWNER,
            key: mention.key,
            expected_mention_revision: mention.revision,
            memory_id: memory.id,
            memory_revision: memory.revision,
            body_id: memory.body_id,
            source_surface: "Mira".into(),
            target_revisions: vec![(endpoint.id, endpoint.revision)],
            withdrawn_at_ns: 12,
        })
        .unwrap();
    let error = writer
        .publish_identity_receipt(distinct_successor(&receipt, [2; 32]))
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("local mention revision or endpoint")
    );
    drop(writer);

    let reopened = reopen(&source);
    assert_chain(&reopened, std::slice::from_ref(&receipt));
    assert_eq!(
        reopened
            .retained_identity_mention_quote(endpoint.id, endpoint.revision, &original)
            .unwrap()
            .as_deref(),
        Some(QUOTE)
    );
    let withdrawn = IdentityMentionRef {
        revision: original.revision + 1,
        ..original
    };
    assert_eq!(
        reopened
            .retained_identity_mention_quote(endpoint.id, endpoint.revision, &withdrawn)
            .unwrap(),
        None
    );
}

#[test]
fn full_replay_rejects_a_mention_first_resolved_after_publication() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("future-mention.rel");
    let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
    let endpoint = fixture_entity(&mut writer, "Mira");
    let memory = fixture_source_memory(&mut writer);
    let mut receipt = unresolved(OWNER, &endpoint);
    receipt
        .left
        .mentions
        .push(fixture_mention_reference(&memory, None));
    writer
        .container
        .append_publication_envelope(&encode(&receipt).unwrap(), None)
        .unwrap();
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    fixture_mention(&mut writer, &memory, &endpoint);
    drop(writer);

    let error = RelSession::open_path(&source, &admission(), [], [])
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("local mention revision or endpoint")
    );
}

#[test]
fn retained_resolution_receipt_binds_its_ordinal_and_survives_an_ordinary_override() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("retained-mention.rel");
    let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
    let endpoint = fixture_entity(&mut writer, "Mira");
    let memory = fixture_source_memory(&mut writer);
    let reference = fixture_mention_reference(&memory, Some(7));
    let row = fixture_retained_resolution(&writer, &reference, &endpoint);
    writer.container.append(&row).unwrap();
    writer.container.sync().unwrap();
    drop(writer);

    let mut writer = RelWriter::open_path(&source, admission(), [], []).unwrap();
    let (basis, proof) = writer
        .read_session()
        .identity_evidence_with_proof(endpoint.id)
        .unwrap()
        .unwrap();
    let mut receipt = unresolved(OWNER, &endpoint);
    receipt.left = basis.references();
    assert_eq!(receipt.left.mentions, [reference]);
    receipt.citations = vec![IdentityCitation::Mention {
        side: IdentitySide::Left,
        mention: 0,
    }];

    let mut forged = receipt.clone();
    forged.left.mentions[0].retained_ordinal = Some(6);
    let error = writer
        .publish_prepared_identity_receipt(forged.clone(), &proof)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("prepared local evidence changed")
    );
    let error = writer.publish_identity_receipt(forged).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("local mention revision or endpoint")
    );

    writer
        .publish_prepared_identity_receipt(receipt.clone(), &proof)
        .unwrap();
    let live = fixture_mention(&mut writer, &memory, &endpoint);
    assert_eq!(live.key, reference.key);
    assert_eq!(live.revision, reference.revision);
    let error = writer
        .publish_identity_receipt(distinct_successor(&receipt, [2; 32]))
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("local mention revision or endpoint")
    );

    let basis = writer
        .read_session()
        .identity_evidence(endpoint.id)
        .unwrap()
        .unwrap();
    assert_eq!(
        basis.references().mentions,
        [fixture_mention_reference(&memory, None)]
    );
    drop(writer);

    let reopened = reopen(&source);
    assert_chain(&reopened, std::slice::from_ref(&receipt));
    assert_eq!(
        reopened
            .retained_identity_mention_quote(endpoint.id, endpoint.revision, &reference)
            .unwrap()
            .as_deref(),
        Some(QUOTE)
    );
}

#[test]
fn full_replay_rejects_a_retained_resolution_written_after_the_receipt() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("future-retained-mention.rel");
    let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
    let endpoint = fixture_entity(&mut writer, "Mira");
    let memory = fixture_source_memory(&mut writer);
    let reference = fixture_mention_reference(&memory, Some(7));
    let mut receipt = unresolved(OWNER, &endpoint);
    receipt.left.mentions.push(reference);
    // The later record claims the same source cut the receipt actually observed.
    let row = fixture_retained_resolution(&writer, &reference, &endpoint);
    writer
        .container
        .append_publication_envelope(&encode(&receipt).unwrap(), None)
        .unwrap();
    writer.container.append(&row).unwrap();
    writer.container.sync().unwrap();
    drop(writer);

    let error = RelSession::open_path(&source, &admission(), [], [])
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("local mention revision or endpoint")
    );
}
