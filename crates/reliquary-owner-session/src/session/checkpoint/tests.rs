use super::*;
use crate::session::{RelWriter, UserMemoryCorrection};
use reliquary_archive::{Archive, ArchivePublicationCoordinate, TurnInput};
use reliquary_freshness_store::FreshnessMemoryId;
use reliquary_graph::GraphAdmission;
use reliquary_memory_core::{MemoryDraft, PayloadState};
use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom};

const OWNER: [u8; 16] = [23; 16];

fn admit(
    owner: &Path,
    slot: &Path,
    writable: bool,
) -> Result<(Container, RelSession, Option<WriterCheckpoint>), OwnerSessionError> {
    let lease =
        Container::prepare_checkpoint_open(owner, writable, FileKind::Reliquary, Some(OWNER))?;
    let extent = lease.extent();
    let (container, session, mut checkpoint) =
        open_leased(lease, Some(slot.to_path_buf()), extent, writable)?;
    if let Some(checkpoint) = &mut checkpoint
        && checkpoint.attempted_extent.is_none()
    {
        checkpoint.refresh(&container, &session, true);
    }
    Ok((container, session, checkpoint))
}

fn draft() -> MemoryDraft {
    MemoryDraft {
        title: "Owner history".into(),
        content: "Retained body from before the checkpoint.".into(),
        category: "fact".into(),
        memory_type: "semantic".into(),
        authority_kind: "user".into(),
        temporal_status: "current".into(),
        scope: "local".into(),
        lifecycle_state: "extracted".into(),
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
        source_time_ns: None,
        temporal_inference: None,
        mutation_id: "checkpoint-source".into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

#[test]
fn restores_a_verified_cut_and_replays_a_bound_turn_and_grouped_correction() {
    let directory = tempfile::tempdir().unwrap();
    let owner = directory.path().join("owner.rel");
    let cache = directory.path().join("private");
    private_cache::test_directory(&cache).unwrap();
    let slot = cache.join("owner.checkpoint");

    let mut initial = RelWriter::create_current_path(&owner, OWNER).unwrap();
    let created = initial
        .session
        .common
        .memories
        .publish(&mut initial.container, None, 0, draft())
        .unwrap()
        .revision;
    let freshness_id = FreshnessMemoryId(created.record.id.0);
    initial
        .session
        .freshness
        .create_birth(&mut initial.container, freshness_id, 0, false)
        .unwrap();
    let reserved = initial.container.reserve_version(Some(-13)).unwrap();
    let untimed = initial.container.reserve_version(None).unwrap();
    let rows = (0..20)
        .map(|row| vec![row as f32; 1100])
        .collect::<Vec<_>>();
    let matrix = initial
        .session
        .common
        .packed_vectors
        .put_f32_rows(&mut initial.container, &rows)
        .unwrap();
    initial.container.sync().unwrap();
    drop(initial);

    let (lease, projected, checkpoint) = admit(&owner, &slot, true).unwrap();
    let covered = lease.extent().unwrap();
    assert_eq!(checkpoint.unwrap().attempted_extent, Some(covered));
    assert_eq!(
        projected.current_memory(created.record.id).unwrap(),
        created
    );
    let manifest = Entry::open(&slot).unwrap().manifest;
    assert_eq!(manifest.physical.covered_end, covered);
    drop((projected, lease));

    let mut writer =
        RelWriter::open_path(&owner, GraphAdmission::for_changes(OWNER, [], []), [], []).unwrap();
    writer
        .session
        .archive
        .accept_turn_at(
            &mut writer.container,
            TurnInput {
                id: "source".into(),
                conversation_id: "history".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("alice".into()),
                timestamp_ns: 2,
                content: "Exact Archive payload in the reserved publication.".into(),
                attachments: Vec::new(),
                project_attachments: Vec::new(),
            },
            Some(ArchivePublicationCoordinate {
                archive_version: 1,
                global_version: reserved,
                transaction_time_ns: Some(-13),
                bound_reservation: true,
            }),
        )
        .unwrap();
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    let next = writer
        .correct_user_memory(
            "alice",
            format!("management:{}", "aa".repeat(32)),
            [4; 32],
            UserMemoryCorrection {
                id: created.record.id,
                revision: created.record.revision,
                body: created.record.body_id,
                title: "Correction".into(),
                content: "One grouped revision and supersession.".into(),
                created_at_ns: 3,
            },
        )
        .unwrap()
        .replacement;
    drop(writer);

    let (lease, warm, checkpoint) = admit(&owner, &slot, true).unwrap();
    assert_eq!(checkpoint.unwrap().attempted_extent, Some(covered));
    assert_eq!(lease.transaction_time_ns(reserved), Some(-13));
    assert!(!lease.has_unbound_version_reservation(reserved));
    assert!(lease.has_unbound_version_reservation(untimed));
    assert_eq!(lease.transaction_time_ns(untimed), None);
    assert_eq!(warm.current_memory(next.record.id).unwrap(), next);
    assert_eq!(
        warm.freshness
            .memory_changes_after(0, 1, 1)
            .collect::<Vec<_>>(),
        [(1, freshness_id)]
    );
    let freshness_cut = warm.freshness.cut_through_turn(0);
    assert_eq!(
        warm.freshness
            .records_for_memories_at(&freshness_cut, [freshness_id])
            .unwrap(),
        warm.freshness.records_at(&freshness_cut).unwrap(),
    );
    // This row straddles two verified packed pages.
    assert_eq!(
        warm.common.packed_vectors.read_f32_row(matrix, 14).unwrap(),
        rows[14]
    );
    let old = warm
        .memory_history_at_cut(created.record.id, 1)
        .unwrap()
        .unwrap();
    assert_eq!(old.revisions, [created]);
    assert!(matches!(
        &old.revisions[0].payload,
        PayloadState::Available(_)
    ));
    let turn = warm
        .management_source_turn("history", "source", 4096, false)
        .unwrap()
        .unwrap();
    assert_eq!(
        turn.content.as_deref(),
        Some("Exact Archive payload in the reserved publication.")
    );
    let current_cut = warm.current_historical_cut();
    let proof = warm.historical_proof(OWNER, &current_cut).unwrap();
    drop((warm, lease));

    let full = RelSession::open_path_current(&owner).unwrap();
    assert_eq!(proof, full.historical_proof(OWNER, &current_cut).unwrap());

    // An incomplete staged replacement leaves the prior generation usable.
    let mut staging = tempfile::NamedTempFile::new_in(&cache).unwrap();
    staging.write_all(MAGIC).unwrap();
    staging.as_file().sync_all().unwrap();
    assert_eq!(
        Entry::open(&slot).unwrap().manifest.physical.covered_end,
        covered
    );
    drop(staging);

    // Damage in a state page falls through to complete native admission.
    let mut file = OpenOptions::new().write(true).open(&slot).unwrap();
    file.seek(SeekFrom::Start(STATE_START + 32)).unwrap();
    file.write_all(b"\xff").unwrap();
    drop(file);
    let (lease, restored, checkpoint) = admit(&owner, &slot, true).unwrap();
    assert_eq!(
        checkpoint.unwrap().attempted_extent,
        Some(lease.extent().unwrap())
    );
    assert_eq!(
        proof,
        restored.historical_proof(OWNER, &current_cut).unwrap()
    );
    drop((lease, restored));

    // An identical owner copied to a different file has no usable checkpoint.
    let copy = directory.path().join("copy.rel");
    fs::copy(&owner, &copy).unwrap();
    let (lease, restored, _) = admit(&copy, &slot, false).unwrap();
    assert_eq!(
        proof,
        restored.historical_proof(OWNER, &current_cut).unwrap()
    );
    drop((restored, lease));

    let mut file = OpenOptions::new().append(true).open(&owner).unwrap();
    file.write_all(b"x").unwrap();
    drop(file);
    assert!(admit(&owner, &slot, true).is_err());
}

#[test]
fn identity_receipts_restore_from_exact_checkpoint_and_validate_the_suffix() {
    use super::super::identity_receipt::tests::{fixture_entity, unresolved};

    let directory = tempfile::tempdir().unwrap();
    let owner = directory.path().join("identity.rel");
    let cache = directory.path().join("private");
    private_cache::test_directory(&cache).unwrap();
    let slot = cache.join("identity.checkpoint");
    let mut writer = RelWriter::create_current_path(&owner, OWNER).unwrap();
    let entity = fixture_entity(&mut writer, "Mira");
    let first = unresolved(OWNER, &entity);
    writer.publish_identity_receipt(first.clone()).unwrap();
    drop(writer);

    let (lease, cold, _) = admit(&owner, &slot, true).unwrap();
    let first_extent = lease.extent().unwrap();
    let first_cut = cold.current_historical_cut();
    let first_proof = cold.historical_proof(OWNER, &first_cut).unwrap();
    assert_eq!(Entry::open(&slot).unwrap().manifest.identity_cut, 1);
    drop((lease, cold));

    let (lease, warm, _) = admit(&owner, &slot, false).unwrap();
    assert_eq!(warm.identity_receipt(&first.pair, None), Some(&first));
    assert_eq!(
        warm.historical_proof(OWNER, &first_cut).unwrap(),
        first_proof
    );
    drop((lease, warm));

    let second = reliquary_perception_store::identity::IdentityReceipt {
        revision: 2,
        predecessor: Some(1),
        fingerprint: [2; 32],
        ..first.clone()
    };
    let mut writer =
        RelWriter::open_path(&owner, GraphAdmission::for_changes(OWNER, [], []), [], []).unwrap();
    writer.publish_identity_receipt(second.clone()).unwrap();
    drop(writer);

    let (lease, restored, refresh) = admit(&owner, &slot, true).unwrap();
    assert!(lease.extent().unwrap() > first_extent);
    assert_eq!(restored.identity_receipt(&first.pair, None), Some(&second));
    assert_eq!(
        refresh.unwrap().attempted_extent,
        Some(lease.extent().unwrap())
    );
    let manifest = Entry::open(&slot).unwrap().manifest;
    assert_eq!(manifest.physical.covered_end, lease.extent().unwrap());
    assert_eq!(manifest.identity_cut, 2);
    assert_eq!(
        restored.identity_receipt(&first.pair, Some(1)),
        Some(&first)
    );
    assert_eq!(
        restored.historical_proof(OWNER, &first_cut).unwrap(),
        first_proof
    );
}

#[path = "tests_identity_separation.rs"]
mod tests_identity_separation;
