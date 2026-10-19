use super::*;
use crate::imports::{LegacyImportEpoch, LegacyImportEpochOrigin, LegacyImportEpochStatus};
use crate::{EpisodeBoundary, EpisodeConfig, EpisodeOrigin};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("legacy-epoch-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn legacy_frame_bytes(frame: &LegacyImportEpoch) -> Vec<u8> {
    let json = serde_json::to_vec(frame).unwrap();
    let mut bytes = b"CVAILEG1".to_vec();
    bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&json);
    bytes.extend_from_slice(&Sha256::digest(&bytes));
    bytes
}

fn rechecksum(mut payload: Vec<u8>) -> Vec<u8> {
    let end = payload.len() - 32;
    let digest = Sha256::digest(&payload[..end]);
    payload[end..].copy_from_slice(&digest);
    payload
}

fn valid_frame(owner: [u8; 16], prefix: &[u8], settlement: &[u8]) -> LegacyImportEpoch {
    let settlement_value =
        crate::freshness_storage::HistoricalImportFreshnessSettlement::decode_legacy_frame(
            settlement,
        )
        .unwrap();
    LegacyImportEpoch {
        origin: LegacyImportEpochOrigin::LegacyCvafrs03,
        status: LegacyImportEpochStatus::CompletedInitial,
        owner_uuid: owner,
        import_id: settlement_value.import_id,
        source_settlement_digest: format!("{:x}", Sha256::digest(settlement)),
        source_prefix_len: prefix.len() as u64,
        source_prefix_digest: format!("{:x}", Sha256::digest(prefix)),
    }
}

fn wrapper_bytes(frame: &LegacyImportEpoch, prefix_len: u64, prefix_digest: [u8; 32]) -> Vec<u8> {
    crate::imports::legacy_epoch_transport::encode(
        &legacy_frame_bytes(frame),
        &crate::imports::legacy_epoch_transport::PrefixBinding {
            length: prefix_len,
            digest: prefix_digest,
        },
    )
    .unwrap()
}

fn rewrite_wrapper(path: &Path, replacement: &[u8]) {
    let mut container = crate::Container::open(path).unwrap();
    let mut found = None;
    container
        .visit_payloads::<crate::ContainerError>(|object, payload| {
            if payload.starts_with(b"CVAILEG2") {
                assert_eq!(payload.len(), replacement.len());
                found = Some(object.offset() + 8);
            }
            Ok(())
        })
        .unwrap();
    drop(container);
    let offset = found.expect("owner contains relocated legacy epoch frame");
    let mut file = fs::OpenOptions::new().write(true).open(path).unwrap();
    file.seek(SeekFrom::Start(offset)).unwrap();
    file.write_all(replacement).unwrap();
    file.sync_all().unwrap();
}

fn create_completed_import_memory(cva: &mut Cva) -> crate::MemoryId {
    let mut parent = None;
    for index in 0..8 {
        let user = format!("u{index}");
        let assistant = format!("a{index}");
        cva.append_node(
            user.clone(),
            "legacy-epoch-conversation".into(),
            parent.clone(),
            "user".into(),
            100 + index as i64 * 2,
            &format!("Imported source statement {index}."),
        )
        .unwrap();
        cva.append_node(
            assistant.clone(),
            "legacy-epoch-conversation".into(),
            Some(user),
            "assistant".into(),
            101 + index as i64 * 2,
            "Acknowledged.",
        )
        .unwrap();
        parent = Some(assistant);
    }
    let episode = cva
        .materialize_path_episodes(
            "legacy-epoch-conversation",
            parent.as_deref().unwrap(),
            EpisodeConfig::default(),
            EpisodeOrigin::Import,
            Some((EpisodeBoundary::Inactivity, 120)),
        )
        .unwrap()
        .created
        .remove(0);
    cva.queue_insomnia_episode(episode.id, crate::InsomniaPriority::Import, 121)
        .unwrap();
    let memory = crate::insomnia::completion_tests::process(cva, 1, 130)
        .created
        .remove(0);

    let pass = crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), memory.id);
    cva.begin_initial_dream_pass(
        pass.clone(),
        memory.id,
        1,
        cva.rel_turn_count(),
        Default::default(),
        cva.graph_version(),
    )
    .unwrap();
    cva.reconcile_dream_lifecycle(memory.id).unwrap();
    cva.accept_initial_dream_settlement(&pass, cva.rel_turn_count())
        .unwrap();
    crate::dream_freshness::settle_initial_pass(cva, memory.id, true, 140).unwrap();
    memory.id
}

fn owner_with_legacy_epoch(
    path: &Path,
    include_inert_memory: bool,
) -> (
    LegacyImportEpoch,
    Vec<u8>,
    Vec<u8>,
    Option<(Vec<u8>, Vec<u8>)>,
    crate::MemoryId,
) {
    let mut cva = Cva::create(path).unwrap();
    let inert = if include_inert_memory {
        let orphan_title = "Unpublished legacy epoch body";
        let orphan_content = "This inert body's carrier is reclaimed.";
        let orphan_body = crate::memory_model::memory_body_bytes(orphan_title, orphan_content);
        let orphan_body_id = crate::memory_model::memory_body_id(orphan_title, orphan_content);
        let body_payload = crate::memory_codec::encode_body(orphan_body_id, &orphan_body).unwrap();
        cva.container.append(&body_payload).unwrap();

        let inert_record = crate::memory_model::MemoryRecord {
            id: crate::MemoryId([0x91; 32]),
            revision: 1,
            body_id: orphan_body_id,
            category: "fact".into(),
            memory_type: "project".into(),
            authority_kind: "direct".into(),
            temporal_status: "current".into(),
            scope: "project".into(),
            lifecycle_state: "knowledge".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_time_ns: None,
            source_ref: None,
            temporal_inference: None,
            mutation_id: "unpublished-legacy-epoch-carrier".into(),
            created_at_ns: 10,
            updated_at_ns: 10,
            global_version: 0,
            memory_version: 1,
        };
        let record_payload = crate::memory_codec::encode_record(&inert_record).unwrap();
        cva.container.append(&record_payload).unwrap();

        let attached = vec![0x37; 64 * 1024];
        let file = cva
            .store_file("legacy-epoch-backed.bin".into(), None, &attached)
            .unwrap();
        cva.bind_legacy_project_file(
            file.id,
            crate::ProjectFileRef {
                revision: crate::ProjectRevisionRef {
                    repository: crate::ProjectRepositoryRef {
                        kind: crate::ProjectRepositoryKind::Lore,
                        repository_id: "legacy-epoch-repository".into(),
                        project_path: ".".into(),
                    },
                    revision: "legacy-epoch-revision".into(),
                },
                path: "uploads/legacy-epoch-backed.bin".into(),
                content_hash: Some(sha2::Sha256::digest(&attached).into()),
            },
        )
        .unwrap();

        Some((body_payload, record_payload))
    } else {
        None
    };
    let imported_memory_id = create_completed_import_memory(&mut cva);

    // Build real legacy completion bytes before constructing the prefix proof;
    // changing only a V2 header would incorrectly label compact records as V1.
    let legacy_copy = path.with_extension("legacy-building.rel");
    crate::insomnia::completion_migration_tests::copy_with_legacy_v6_completion(
        &mut cva,
        &legacy_copy,
    );
    drop(cva);
    let mut legacy_bytes = fs::read(&legacy_copy).unwrap();
    legacy_bytes[8..10].copy_from_slice(&2_u16.to_le_bytes());
    fs::write(path, legacy_bytes).unwrap();
    fs::remove_file(&legacy_copy).unwrap();
    let mut cva = Cva::open(path).unwrap();
    let prepared = cva
        .freeze_historical_import()
        .unwrap()
        .prepare(&[])
        .unwrap();
    let mut settlement = prepared.settlement.clone();
    settlement.semantic = None;
    settlement.input_bytes = prepared.source_bytes;
    let settlement_bytes = crate::freshness_storage::encode_legacy_test_fixture(&settlement);
    cva.container.append(&settlement_bytes).unwrap();
    cva.sync().unwrap();
    let source_prefix_len = fs::metadata(path).unwrap().len();
    let placeholder = LegacyImportEpoch {
        origin: LegacyImportEpochOrigin::LegacyCvafrs03,
        status: LegacyImportEpochStatus::CompletedInitial,
        owner_uuid: settlement.owner,
        import_id: settlement.import_id,
        source_settlement_digest: "00".repeat(32),
        source_prefix_len,
        source_prefix_digest: "00".repeat(32),
    };
    let placeholder_bytes = legacy_frame_bytes(&placeholder);
    let legacy_object = cva.container.append(&placeholder_bytes).unwrap();
    cva.sync().unwrap();
    drop(cva);

    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, &bytes).unwrap();
    let prefix_len = legacy_object.offset() as usize;
    let prefix = fs::read(path).unwrap()[..prefix_len].to_vec();
    let frame = valid_frame(settlement.owner, &prefix, &settlement_bytes);
    let legacy = legacy_frame_bytes(&frame);
    assert_eq!(legacy.len(), placeholder_bytes.len());
    let mut file = fs::OpenOptions::new().write(true).open(path).unwrap();
    file.seek(SeekFrom::Start(legacy_object.offset() + 8))
        .unwrap();
    file.write_all(&legacy).unwrap();
    file.sync_all().unwrap();
    (frame, prefix, legacy, inert, imported_memory_id)
}

fn contains_payload(path: &Path, expected: &[u8]) -> bool {
    let mut container = crate::Container::open(path).unwrap();
    let mut found = false;
    container
        .visit_payloads::<crate::ContainerError>(|_, payload| {
            found |= payload == expected;
            Ok(())
        })
        .unwrap();
    found
}

fn assert_rebound_epoch(path: &Path, expected: &LegacyImportEpoch) {
    let mut container = crate::Container::open(path).unwrap();
    let mut wrapped = None;
    container
        .visit_payloads::<crate::ContainerError>(|_, payload| {
            if payload.starts_with(b"CVAILEG2") {
                wrapped = Some(payload.to_vec());
            }
            Ok(())
        })
        .unwrap();
    let (frame, binding) = crate::imports::legacy_epoch_transport::decode(
        &wrapped.expect("maintenance retains the relocated proof"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(frame, *expected);
    let bytes = fs::read(path).unwrap();
    assert_eq!(
        binding.digest,
        Sha256::digest(&bytes[..binding.length as usize]).as_slice()
    );
}

#[test]
fn cvai_legacy2_preserves_exact_legacy_evidence_and_rejects_bad_framing() {
    let owner = [0x44; 16];
    let old = LegacyImportEpoch {
        origin: LegacyImportEpochOrigin::LegacyCvafrs03,
        status: LegacyImportEpochStatus::CompletedInitial,
        owner_uuid: owner,
        import_id: [0x22; 32],
        source_settlement_digest: "11".repeat(32),
        source_prefix_len: 987,
        source_prefix_digest: "33".repeat(32),
    };
    let original = legacy_frame_bytes(&old);
    let binding = crate::imports::legacy_epoch_transport::PrefixBinding {
        length: 12_345,
        digest: [0x7a; 32],
    };
    let encoded = crate::imports::legacy_epoch_transport::encode(&original, &binding).unwrap();
    let (decoded, decoded_binding) = crate::imports::legacy_epoch_transport::decode(&encoded)
        .unwrap()
        .unwrap();
    assert_eq!(decoded, old);
    assert_eq!(decoded_binding.length, binding.length);
    assert_eq!(decoded_binding.digest, binding.digest);
    assert_eq!(&encoded[..8], b"CVAILEG2");
    let original_len = u32::from_le_bytes(encoded[8..12].try_into().unwrap()) as usize;
    assert_eq!(&encoded[12..12 + original_len], original);

    let mut bad_checksum = encoded.clone();
    let last = bad_checksum.len() - 1;
    bad_checksum[last] ^= 1;
    assert!(crate::imports::legacy_epoch_transport::decode(&bad_checksum).is_err());
    assert!(crate::imports::legacy_epoch_transport::decode(&encoded[..encoded.len() - 1]).is_err());

    let mut bad_nested = b"CVAILEG2".to_vec();
    bad_nested.extend_from_slice(&4_u32.to_le_bytes());
    bad_nested.extend_from_slice(b"nope");
    bad_nested.extend_from_slice(&binding.length.to_le_bytes());
    bad_nested.extend_from_slice(&binding.digest);
    let bad_nested = rechecksum(bad_nested);
    assert!(crate::imports::legacy_epoch_transport::decode(&bad_nested).is_err());

    let mut nested_bad_checksum = original.clone();
    let last = nested_bad_checksum.len() - 1;
    nested_bad_checksum[last] ^= 1;
    let mut bad_nested = b"CVAILEG2".to_vec();
    bad_nested.extend_from_slice(&(nested_bad_checksum.len() as u32).to_le_bytes());
    bad_nested.extend_from_slice(&nested_bad_checksum);
    bad_nested.extend_from_slice(&binding.length.to_le_bytes());
    bad_nested.extend_from_slice(&binding.digest);
    assert!(crate::imports::legacy_epoch_transport::decode(&rechecksum(bad_nested)).is_err());

    let mut unknown_store = crate::imports::ImportEpochStore::default();
    assert!(unknown_store.ingest(b"CVAILEG3").is_err());
}

#[test]
fn v1_owner_upgrade_rebinds_legacy_epoch_to_relocated_prefix_and_reopens() {
    let scratch = Scratch::new();
    let source_path = scratch.path("legacy-source.rel");
    let request = crate::storage_upgrade::StorageUpgradeRequest {
        source: source_path.clone(),
        destination: scratch.path("legacy-output.rel"),
    };
    let (expected, source_prefix, original_legacy, _, _) =
        owner_with_legacy_epoch(&source_path, false);
    let before = fs::read(&source_path).unwrap();
    let mut source = Cva::open_v1_read_only(&source_path).unwrap();
    assert_eq!(source.legacy_import_epochs(), vec![expected.clone()]);
    let expected_global_head = source.latest_global_version();
    drop(source);

    let plan = crate::storage_upgrade::plan_storage_upgrade(request).unwrap();
    let staged = crate::storage_upgrade::stage_memory_storage_upgrade(&plan).unwrap();
    let mut reopened = Cva::open(staged.test_path()).unwrap();
    assert_eq!(reopened.legacy_import_epochs(), vec![expected.clone()]);
    assert_eq!(reopened.latest_global_version(), expected_global_head);
    drop(reopened);
    assert_eq!(fs::read(&source_path).unwrap(), before);

    let mut output = crate::Container::open(staged.test_path()).unwrap();
    let mut wrapper = None;
    output
        .visit_payloads::<crate::ContainerError>(|_, payload| {
            if payload.starts_with(b"CVAILEG2") {
                wrapper = Some(payload.to_vec());
            }
            Ok(())
        })
        .unwrap();
    let wrapper = wrapper.expect("migration emits a compact legacy proof wrapper");
    let (frame, binding) = crate::imports::legacy_epoch_transport::decode(&wrapper)
        .unwrap()
        .unwrap();
    assert_eq!(frame, expected);
    assert_ne!(binding.length, source_prefix.len() as u64);
    let relocated = fs::read(staged.test_path()).unwrap();
    assert_eq!(
        binding.digest,
        Sha256::digest(&relocated[..binding.length as usize]).as_slice()
    );
    assert_eq!(
        crate::imports::legacy_epoch::decode(&original_legacy)
            .unwrap()
            .unwrap(),
        expected
    );

    let mut tampered_cases = Vec::new();
    tampered_cases.push(wrapper_bytes(&expected, binding.length, [0x01; 32]));
    tampered_cases.push(wrapper_bytes(&expected, binding.length - 1, binding.digest));
    let mut wrong_owner = expected.clone();
    let different = wrong_owner
        .owner_uuid
        .iter()
        .position(|byte| *byte != wrong_owner.owner_uuid[0])
        .expect("fixture owner has distinct UUID bytes");
    wrong_owner.owner_uuid.swap(0, different);
    tampered_cases.push(wrapper_bytes(&wrong_owner, binding.length, binding.digest));
    let mut wrong_settlement = expected.clone();
    wrong_settlement.source_settlement_digest = "ab".repeat(32);
    tampered_cases.push(wrapper_bytes(
        &wrong_settlement,
        binding.length,
        binding.digest,
    ));
    for (index, tampered) in tampered_cases.into_iter().enumerate() {
        let copy = scratch.path(&format!("tampered-{index}.rel"));
        fs::copy(staged.test_path(), &copy).unwrap();
        rewrite_wrapper(&copy, &tampered);
        assert!(Cva::open(&copy).is_err(), "tampered case {index} opened");
    }

    let v1_copy = scratch.path("v1-wrapper.rel");
    fs::copy(staged.test_path(), &v1_copy).unwrap();
    let mut bytes = fs::read(&v1_copy).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(&v1_copy, &bytes).unwrap();
    let before = fs::read(&v1_copy).unwrap();
    assert!(Cva::open_v1_read_only(&v1_copy).is_err());
    assert_eq!(fs::read(&v1_copy).unwrap(), before);
}

#[test]
fn reclaimed_migrated_owner_rebinds_legacy_epoch_after_dropping_inert_memory_frames() {
    let scratch = Scratch::new();
    let source_path = scratch.path("legacy-source-with-inert-memory.rel");
    let request = crate::storage_upgrade::StorageUpgradeRequest {
        source: source_path.clone(),
        destination: scratch.path("legacy-output-with-inert-memory.rel"),
    };
    let (expected, _source_prefix, _original_legacy, inert, memory_id) =
        owner_with_legacy_epoch(&source_path, true);
    let (inert_body, inert_record) = inert.unwrap();
    let source_bytes = fs::read(&source_path).unwrap();
    let mut source = Cva::open_v1_read_only(&source_path).unwrap();
    let source_memory = source.memory(memory_id).unwrap();
    let source_record = source.memories.records().last().unwrap().clone();
    let source_global = source.latest_global_version();
    let source_local = source.memory_version();
    assert!(contains_payload(&source_path, &inert_body));
    assert!(contains_payload(&source_path, &inert_record));
    drop(source);

    let plan = crate::storage_upgrade::plan_storage_upgrade(request).unwrap();
    let staged = crate::storage_upgrade::stage_memory_storage_upgrade(&plan).unwrap();
    let staged_path = staged.test_path();
    assert!(contains_payload(staged_path, &inert_body));
    assert!(contains_payload(staged_path, &inert_record));

    let mut migrated = Cva::open(staged_path).unwrap();
    assert_eq!(migrated.legacy_import_epochs(), vec![expected.clone()]);
    assert_eq!(migrated.memory(memory_id).unwrap(), source_memory);
    assert_eq!(migrated.memories.records().last().unwrap(), &source_record);
    assert_eq!(migrated.latest_global_version(), source_global);
    assert_eq!(migrated.memory_version(), source_local);
    assert_eq!(migrated.normalize_import_settlements().unwrap(), 0);
    let report = migrated
        .reclaim_storage(scratch.path("legacy-reclaimed.rel"))
        .unwrap();
    assert!(report.reclaimed_chunks >= 2);
    drop(migrated);
    assert_eq!(fs::read(&source_path).unwrap(), source_bytes);

    let reclaimed_path = scratch.path("legacy-reclaimed.rel");
    assert!(!contains_payload(&reclaimed_path, &inert_body));
    assert!(!contains_payload(&reclaimed_path, &inert_record));
    let mut reclaimed = Cva::open(&reclaimed_path).unwrap();
    assert_eq!(reclaimed.legacy_import_epochs(), vec![expected.clone()]);
    assert_eq!(reclaimed.memory(memory_id).unwrap(), source_memory);
    assert_eq!(reclaimed.memories.records().last().unwrap(), &source_record);
    assert_eq!(reclaimed.latest_global_version(), source_global);
    assert_eq!(reclaimed.memory_version(), source_local);
    assert_rebound_epoch(&reclaimed_path, &expected);

    let mut repack_source = Cva::open(staged_path).unwrap();
    assert_eq!(repack_source.normalize_import_settlements().unwrap(), 0);
    let repacked_path = scratch.path("legacy-repacked.rel");
    let repack = repack_source
        .repack_project_backed_attachments(&repacked_path)
        .unwrap();
    assert_eq!(repack.dropped_content_objects, 1);
    drop(repack_source);
    let mut repacked = Cva::open(&repacked_path).unwrap();
    assert_eq!(repacked.legacy_import_epochs(), vec![expected.clone()]);
    assert_eq!(repacked.memory(memory_id).unwrap(), source_memory);
    assert_eq!(repacked.latest_global_version(), source_global);
    assert_eq!(repacked.memory_version(), source_local);
    drop(repacked);
    assert_rebound_epoch(&repacked_path, &expected);

    let mut prune_source = Cva::open(staged_path).unwrap();
    assert_eq!(prune_source.normalize_import_settlements().unwrap(), 0);
    prune_source
        .append_node(
            "legacy-epoch-prune-user".into(),
            "legacy-epoch-prune-conversation".into(),
            None,
            "user".into(),
            200,
            &"Terminal conversation payload for explicit prune. ".repeat(4096),
        )
        .unwrap();
    prune_source
        .tombstone_conversation("legacy-epoch-prune-conversation")
        .unwrap();
    let prune_global = prune_source.latest_global_version();
    let prune_local = prune_source.memory_version();
    let pruned_path = scratch.path("legacy-pruned.rel");
    let prune = prune_source
        .prune_tombstoned_history(
            &pruned_path,
            crate::HistoryPruneRequest {
                targets: vec![crate::owner_lifecycle::LifecycleTarget::Conversation(
                    "legacy-epoch-prune-conversation".into(),
                )],
            },
        )
        .unwrap();
    assert_eq!(prune.pruned_conversations, 1);
    drop(prune_source);
    let mut pruned = Cva::open(&pruned_path).unwrap();
    assert_eq!(pruned.legacy_import_epochs(), vec![expected.clone()]);
    assert_eq!(pruned.memory(memory_id).unwrap(), source_memory);
    assert_eq!(pruned.latest_global_version(), prune_global);
    assert_eq!(pruned.memory_version(), prune_local);
    drop(pruned);
    assert_rebound_epoch(&pruned_path, &expected);
}
