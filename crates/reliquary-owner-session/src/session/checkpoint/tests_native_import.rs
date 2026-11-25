use super::*;
use crate::records::{
    ImportEpochStatus, ImportEpochStore, ImportFreshnessSettlement, hex_digest,
    import_settlement::source_prefix_digest,
};
use crate::session::RelWriter;
use reliquary_archive::{Branch, TurnInput};
use reliquary_graph::GraphAdmission;
use reliquary_insomnia_store::{EpisodeId, InsomniaPriority};

#[test]
fn suspended_native_import_replays_source_suffix_and_recovers_queued_episode() {
    let directory = tempfile::tempdir().unwrap();
    let owner = directory.path().join("import.rel");
    let private = directory.path().join("private");
    private_cache::test_directory(&private).unwrap();
    let slot = private.join("owner.checkpoint");
    let owner_id = [86; 16];
    let mut writer = RelWriter::create_current_path(&owner, owner_id).unwrap();
    writer
        .start_native_import(&"ab".repeat(32), "fixture/account", "schema-1", 1)
        .unwrap();
    let import_id = writer.session.open_native_import_id().unwrap().to_owned();
    drop(writer);

    let open_checkpoint = || {
        let lease =
            Container::prepare_checkpoint_open(&owner, true, FileKind::Reliquary, Some(owner_id))
                .unwrap();
        let extent = lease.extent();
        open_leased(lease, Some(slot.clone()), extent, true).unwrap()
    };
    let (lease, session, checkpoint) = open_checkpoint();
    let covered_end = lease.extent().unwrap();
    assert_eq!(session.open_native_import_id(), Some(import_id.as_str()));
    checkpoint.unwrap().refresh(&lease, &session, true);
    drop((session, lease));

    // Write through an uncached session so the fixture retains the import-start cut.
    let mut writer = RelWriter::open_path(
        &owner,
        GraphAdmission::for_changes(owner_id, [], []),
        [],
        [],
    )
    .unwrap();
    let reservation = writer.container.reserve_version(Some(19)).unwrap();
    writer.container.sync().unwrap();
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    let input = TurnInput {
        id: "node".into(),
        conversation_id: "conversation".into(),
        parent_id: None,
        role: "user".into(),
        principal_id: None,
        timestamp_ns: 2,
        content: "Imported source text.".into(),
        attachments: Vec::new(),
        project_attachments: Vec::new(),
    };
    let branch = Branch {
        id: "main".into(),
        conversation_id: input.conversation_id.clone(),
        leaf_node_id: input.id.clone(),
        canonical: true,
    };
    let accepted = writer
        .accept_native_import_turn(&import_id, input.clone(), branch.clone())
        .unwrap();
    drop(writer);

    let (lease, session, checkpoint) = open_checkpoint();
    assert_eq!(checkpoint.unwrap().attempted_extent, Some(covered_end));
    assert_eq!(lease.transaction_time_ns(reservation), Some(19));
    assert!(lease.has_unbound_version_reservation(reservation));
    assert_eq!(session.open_native_import_id(), Some(import_id.as_str()));
    assert_eq!(
        session.archive_node("conversation", "node"),
        Some(&accepted)
    );
    assert_eq!(
        session.archive_branch("conversation", "main"),
        Some(&branch)
    );
    let turn = session
        .management_source_turn("conversation", "node", 4096, false)
        .unwrap()
        .unwrap();
    assert_eq!(turn.content.as_deref(), Some(input.content.as_str()));
    let cut = session.current_historical_cut();
    let proof = session.historical_proof(owner_id, &cut).unwrap();
    let suffix_end = lease.extent().unwrap();
    drop((session, lease));

    let detached = RelSession::open_path_current(&owner).unwrap();
    assert_eq!(detached.open_native_import_id(), Some(import_id.as_str()));
    assert_eq!(proof, detached.historical_proof(owner_id, &cut).unwrap());
    drop(detached);

    let (container, session, _) = open_checkpoint();
    let mut writer = RelWriter {
        container,
        session,
        checkpoint: None,
        verified_dream_import: None,
    };
    writer
        .accept_native_import_turn(&import_id, input, branch)
        .unwrap();
    assert_eq!(writer.container.extent().unwrap(), suffix_end);

    // A suffix with a new Episode also changes the Insomnia scheduler.
    let episode = writer
        .finalize_native_import_episode(&import_id, "conversation", "node", "node", 3)
        .unwrap();
    drop(writer);
    let (lease, session, checkpoint) = open_checkpoint();
    assert_eq!(checkpoint.unwrap().attempted_extent, None);
    assert!(lease.extent().unwrap() > suffix_end);
    assert_eq!(
        session
            .insomnia_work(EpisodeId(episode.0))
            .unwrap()
            .priority,
        InsomniaPriority::Import
    );
}

#[test]
fn import_settlement_still_verifies_the_physical_prefix_and_fences_later_mutation() {
    let directory = tempfile::tempdir().unwrap();
    for correct_digest in [true, false] {
        let owner = directory.path().join(format!("{correct_digest}.rel"));
        let owner_id = [87; 16];
        let mut writer = RelWriter::create_current_path(&owner, owner_id).unwrap();
        let ImportEpochStatus::Open(start) = writer
            .start_native_import(&"ab".repeat(32), "fixture/account", "schema-1", 1)
            .unwrap()
        else {
            panic!("new import must open");
        };
        let settlement = ImportFreshnessSettlement {
            owner_uuid: owner_id,
            import_id: start.import_id.clone(),
            start_digest: hex_digest(&serde_json::to_vec(&start).unwrap()),
            receipt_digest: hex_digest(b"receipt"),
            prefix_digest: if correct_digest {
                source_prefix_digest(&writer.container).unwrap()
            } else {
                hex_digest(b"incorrect prefix")
            },
            rel_turn: 0,
            global_version: start.pre.global_version,
            memory_version: start.pre.memory_version,
            graph_version: start.pre.graph_version,
            freshness_state_digest: hex_digest(b"freshness"),
            freshness_memory_count: 0,
            replay_source_authority: None,
        };
        if correct_digest {
            writer
                .session
                .imports
                .append_settlement(&mut writer.container, settlement.clone())
                .unwrap();
            let imports = ImportEpochStore::open(&writer.container).unwrap();
            assert_eq!(imports.settlement(&start.import_id), Some(&settlement));
            writer.container.reserve_version(None).unwrap();
            writer.container.sync().unwrap();
            assert_eq!(
                ImportEpochStore::open(&writer.container).err().as_deref(),
                Some("import mutation followed settlement before completion")
            );
        } else {
            assert_eq!(
                writer
                    .session
                    .imports
                    .append_settlement(&mut writer.container, settlement.clone())
                    .unwrap_err(),
                "import settlement source prefix changed"
            );
            writer
                .container
                .append(&settlement.encode().unwrap())
                .unwrap();
            writer.container.sync().unwrap();
            assert_eq!(
                ImportEpochStore::open(&writer.container).err().as_deref(),
                Some("import settlement physical source prefix mismatch")
            );
        }
    }
}
