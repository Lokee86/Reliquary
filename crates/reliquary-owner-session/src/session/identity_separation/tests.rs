use super::super::identity_receipt::tests::{fixture_entity, unresolved};
use super::*;
use crate::model::identity::IdentityReceipt;
use reliquary_perception_store::EntityDraft;
use std::fs;

const OWNER: [u8; 16] = [23; 16];
const PEER: [u8; 16] = [24; 16];

fn admission(owner: [u8; 16]) -> GraphAdmission {
    GraphAdmission::for_replay(owner, [], [], [], [], [])
}

fn create_peer(path: &Path) -> (Entity, IdentityEndpointProof) {
    let mut writer = RelWriter::create_current_path(path, PEER).unwrap();
    let entity = fixture_entity(&mut writer, "peer Entity");
    let proof = writer
        .read_session()
        .identity_endpoint_proof(entity.id, None)
        .unwrap();
    (entity, proof)
}

fn prepare(
    writer: &mut RelWriter,
    right: EntityId,
    right_proof: IdentityEndpointProof,
) -> (Entity, IdentityReceipt, IdentitySeparationEvent) {
    let left = fixture_entity(writer, "recording Entity");
    let mut receipt = unresolved(OWNER, &left);
    receipt.pair.peer_entity = right;
    receipt.right.endpoint_revision = right_proof.revision;
    writer.publish_identity_receipt(receipt.clone()).unwrap();
    let event = IdentitySeparationEvent {
        pair: receipt.pair,
        revision: 1,
        predecessor: None,
        action_id: [1; 16],
        operator_id: "operator-a".into(),
        action: IdentitySeparationAction::Confirm {
            receipt_revision: receipt.revision,
            receipt_fingerprint: receipt.fingerprint,
            left: writer
                .read_session()
                .identity_endpoint_proof(left.id, None)
                .unwrap(),
            right: right_proof,
        },
    };
    (left, receipt, event)
}

fn undo(confirmation: &IdentitySeparationEvent) -> IdentitySeparationEvent {
    IdentitySeparationEvent {
        pair: confirmation.pair,
        revision: confirmation.revision + 1,
        predecessor: Some(confirmation.revision),
        action_id: [4; 16],
        operator_id: "operator-b".into(),
        action: IdentitySeparationAction::Undo {
            target_revision: confirmation.revision,
        },
    }
}

fn publish_revised_entity(writer: &mut RelWriter, entity: &Entity) -> Entity {
    let updated = writer
        .session
        .common
        .perception
        .update_entity(
            &mut writer.container,
            entity.id,
            entity.revision,
            EntityDraft {
                canonical_name: entity.canonical_name.clone(),
                aliases: vec!["New alias".into()],
                kind: entity.kind.clone(),
                summary: "Later Entity description".into(),
                mutation_id: "revision-2".into(),
                created_at_ns: entity.created_at_ns,
                updated_at_ns: 2,
            },
        )
        .unwrap();
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    updated
}

#[test]
fn confirmation_chain_reopens_without_peer_and_exact_retry_returns_original() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("recording.rel");
    let peer = directory.path().join("peer.rel");
    let (right, proof) = create_peer(&peer);
    let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
    let (left, receipt, first) = prepare(&mut writer, right.id, proof);
    assert!(
        writer
            .read_session()
            .identity_separation(&first.pair, None)
            .is_none()
    );
    fixture_entity(&mut writer, "unrelated Entity");
    assert_eq!(
        writer.publish_identity_separation(first.clone()).unwrap(),
        first
    );

    let second = IdentitySeparationEvent {
        revision: 2,
        predecessor: Some(1),
        action_id: [2; 16],
        ..first.clone()
    };
    writer.publish_identity_separation(second.clone()).unwrap();
    let old_head = undo(&first);
    assert!(writer.publish_identity_separation(old_head).is_err());

    let undo_superseded = IdentitySeparationEvent {
        action: IdentitySeparationAction::Undo { target_revision: 1 },
        ..undo(&second)
    };
    assert!(writer.publish_identity_separation(undo_superseded).is_err());
    let left = publish_revised_entity(&mut writer, &left);
    writer
        .session
        .common
        .perception
        .tombstone_entity(&mut writer.container, left.id, left.revision)
        .unwrap();
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    let undo_second = IdentitySeparationEvent {
        action_id: first.action_id,
        ..undo(&second)
    };
    assert_eq!(
        writer
            .publish_identity_separation(undo_second.clone())
            .unwrap(),
        undo_second
    );
    let bytes = writer.container.extent().unwrap();
    assert_eq!(
        writer.publish_identity_separation(first.clone()).unwrap(),
        first
    );
    let changed_claim = IdentitySeparationEvent {
        operator_id: first.operator_id.clone(),
        action_id: first.action_id,
        ..undo_second.clone()
    };
    assert!(writer.publish_identity_separation(changed_claim).is_err());
    let undo_again = IdentitySeparationEvent {
        revision: 4,
        predecessor: Some(3),
        action_id: [5; 16],
        ..undo_second.clone()
    };
    assert!(writer.publish_identity_separation(undo_again).is_err());
    assert_eq!(writer.container.extent().unwrap(), bytes);
    drop(writer);
    fs::remove_file(peer).unwrap();

    let reopened = RelSession::open_path(&source, &admission(OWNER), [], []).unwrap();
    assert_eq!(reopened.identity_receipt(&first.pair, None), Some(&receipt));
    assert_eq!(
        reopened.identity_separation(&first.pair, None),
        Some(&undo_second)
    );
    assert_eq!(
        reopened.identity_separation(&first.pair, Some(2)),
        Some(&second)
    );
    assert_eq!(
        reopened.identity_separation_by_action(first.action_id, &first.operator_id),
        Some(&first)
    );
    assert_eq!(
        reopened.identity_separation_by_action(first.action_id, &undo_second.operator_id),
        Some(&undo_second)
    );
    assert!(!reopened.identity_endpoint_active(left.id));
    assert!(reopened.identity_endpoint_proof(left.id, None).is_none());
    let IdentitySeparationAction::Confirm { left: original, .. } = first.action else {
        unreachable!()
    };
    assert_eq!(
        reopened.identity_endpoint_proof(left.id, Some(original.revision)),
        Some(original)
    );
}

#[test]
fn endpoint_proofs_require_the_reviewed_history_after_remount() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("peer.rel");
    let divergent = directory.path().join("replacement.rel");
    let (entity, original) = create_peer(&source);
    let mut writer = RelWriter::open_path(&source, admission(PEER), [], []).unwrap();
    let updated = publish_revised_entity(&mut writer, &entity);
    let updated_proof = writer
        .read_session()
        .identity_endpoint_proof(entity.id, None)
        .unwrap();
    drop(writer);

    let reopened = RelSession::open_path(&source, &admission(PEER), [], []).unwrap();
    assert!(reopened.identity_endpoint_active(entity.id));
    assert_eq!(
        reopened.identity_endpoint_proof(entity.id, Some(1)),
        Some(original)
    );
    assert_eq!(
        reopened.identity_endpoint_proof(entity.id, None),
        Some(updated_proof)
    );
    assert_ne!(updated_proof, original);

    let mut writer = RelWriter::create_current_path(&divergent, PEER).unwrap();
    let altered = writer
        .session
        .common
        .perception
        .publish_entity(
            &mut writer.container,
            EntityDraft {
                canonical_name: entity.canonical_name,
                aliases: entity.aliases,
                kind: entity.kind,
                summary: "Different original Entity description".into(),
                mutation_id: entity.mutation_id,
                created_at_ns: entity.created_at_ns,
                updated_at_ns: entity.updated_at_ns,
            },
        )
        .unwrap();
    assert_eq!(altered.id, entity.id);
    let altered_updated = publish_revised_entity(&mut writer, &altered);
    assert_eq!(altered_updated, updated);
    assert_ne!(
        writer
            .read_session()
            .identity_endpoint_proof(entity.id, Some(1)),
        Some(original)
    );
    assert_ne!(
        writer
            .read_session()
            .identity_endpoint_proof(entity.id, None),
        Some(updated_proof)
    );
}

#[test]
fn cold_replay_rejects_forged_separation_carriers() {
    let directory = tempfile::tempdir().unwrap();
    let (right, proof) = create_peer(&directory.path().join("peer.rel"));
    for (case, reason) in [
        ("digest", "local Entity proof differs"),
        ("comparison", "comparison receipt differs"),
        ("operator", "action identity"),
        ("peer", "invalid peer Entity proof"),
        ("chain", "revision chain"),
        ("duplicate", "action ID already used"),
        ("raw", "missing allocating publication envelope"),
    ] {
        let source = directory.path().join(format!("{case}.rel"));
        let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
        let (_, receipt, mut event) = prepare(&mut writer, right.id, proof);
        match case {
            "digest" => {
                if let IdentitySeparationAction::Confirm { left, .. } = &mut event.action {
                    left.digest = [0; 32];
                }
            }
            "comparison" => {
                writer
                    .publish_identity_receipt(IdentityReceipt {
                        revision: 2,
                        predecessor: Some(1),
                        fingerprint: [2; 32],
                        ..receipt
                    })
                    .unwrap();
            }
            "operator" => event.operator_id.push('\n'),
            "peer" => {
                if let IdentitySeparationAction::Confirm { right, .. } = &mut event.action {
                    right.history_generation = [0; 32];
                }
            }
            "chain" => event.predecessor = Some(1),
            "duplicate" => {
                writer.publish_identity_separation(event.clone()).unwrap();
                event.revision = 2;
                event.predecessor = Some(1);
            }
            _ => {}
        }
        let body = encode(&event).unwrap();
        if case == "raw" {
            writer.container.append(&body).unwrap();
        } else {
            writer
                .container
                .append_publication_envelope(&body, None)
                .unwrap();
        }
        writer.container.sync().unwrap();
        drop(writer);

        let error = RelSession::open_path(&source, &admission(OWNER), [], [])
            .err()
            .unwrap();
        assert!(error.to_string().contains(reason), "{case}: {error}");
    }
}

#[test]
fn phylactery_rejects_separation_publications() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("owner.phy");
    let mut writer = PhyWriter::create_path(&source, OWNER).unwrap();
    writer
        .container
        .append_publication_envelope(MARKER, None)
        .unwrap();
    writer.container.sync().unwrap();
    drop(writer);
    let error = PhySession::open_path(&source, &admission(OWNER), [])
        .err()
        .unwrap();
    assert!(matches!(
        error,
        OwnerSessionError::RelOnlyRecordInPhylactery("cross-REL Entity separation")
    ));
}

#[test]
fn separation_proof_survives_verified_entity_description_pruning() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("recording.rel");
    let pruned = directory.path().join("pruned.rel");
    let (right, proof) = create_peer(&directory.path().join("peer.rel"));
    let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
    let (left, _, event) = prepare(&mut writer, right.id, proof);
    writer.publish_identity_separation(event.clone()).unwrap();
    writer
        .session
        .common
        .perception
        .tombstone_entity(&mut writer.container, left.id, left.revision)
        .unwrap();
    drop(writer);

    let admission = admission(OWNER);
    let plan = crate::VerifiedOwnerCopyPlan::prepare(&source).unwrap();
    prune_terminal_entity_descriptions_copy(&plan, &source, &pruned, &[left.id], &admission)
        .unwrap();
    let reopened = RelSession::open_path(&pruned, &admission, [], []).unwrap();
    assert!(
        reopened
            .entity_revision(left.id, left.revision)
            .unwrap()
            .summary
            .is_empty()
    );
    let IdentitySeparationAction::Confirm { left: proof, .. } = event.action else {
        unreachable!()
    };
    assert_eq!(
        reopened.identity_endpoint_proof(left.id, Some(left.revision)),
        Some(proof)
    );
    assert_eq!(
        reopened.identity_separation(&event.pair, None),
        Some(&event)
    );
}

#[test]
fn incomplete_separation_tail_recovers_the_complete_confirmation() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("recording.rel");
    let (right, proof) = create_peer(&directory.path().join("peer.rel"));
    let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
    let (_, _, first) = prepare(&mut writer, right.id, proof);
    writer.publish_identity_separation(first.clone()).unwrap();
    let withdrawal = undo(&first);
    writer
        .publish_identity_separation(withdrawal.clone())
        .unwrap();
    drop(writer);
    let complete = fs::read(&source).unwrap();
    let file = fs::OpenOptions::new().write(true).open(&source).unwrap();
    file.set_len(complete.len() as u64 - 8).unwrap();
    file.sync_all().unwrap();
    drop(file);

    assert!(RelSession::open_path(&source, &admission(OWNER), [], []).is_err());
    let mut repair = Container::open_for_repair(&source, FileKind::Reliquary).unwrap();
    let plan = repair.plan_tail_repair().unwrap().unwrap();
    repair.repair_incomplete_tail(plan).unwrap();
    drop(repair);

    let mut writer = RelWriter::open_path(&source, admission(OWNER), [], []).unwrap();
    assert_eq!(
        writer.read_session().identity_separation(&first.pair, None),
        Some(&first)
    );
    writer.publish_identity_separation(withdrawal).unwrap();
    drop(writer);
    assert_eq!(fs::read(&source).unwrap(), complete);
}
