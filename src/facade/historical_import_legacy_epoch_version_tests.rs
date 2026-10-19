use crate::{Container, ContainerError, Cva};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "legacy-settlement-version-{}",
                uuid::Uuid::new_v4()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
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

#[derive(Clone, Copy)]
enum LegacySettlementVersion {
    V4,
    V5,
}

fn empty_import() -> crate::imports::NativeImport {
    crate::imports::NativeImport {
        schema_version: 1,
        source_namespace: "legacy-settlement-migration".into(),
        source_format: "fixture-v1".into(),
        conversations: Vec::new(),
    }
}

fn create_import_memory(cva: &mut Cva) {
    let mut parent = None;
    for index in 0..8 {
        let user = format!("u{index}");
        let assistant = format!("a{index}");
        cva.append_node(
            user.clone(),
            "legacy-v4-import".into(),
            parent.clone(),
            "user".into(),
            100 + index as i64 * 2,
            &format!("Imported source statement {index}."),
        )
        .unwrap();
        cva.append_node(
            assistant.clone(),
            "legacy-v4-import".into(),
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
            "legacy-v4-import",
            parent.as_deref().unwrap(),
            crate::EpisodeConfig::default(),
            crate::EpisodeOrigin::Import,
            Some((crate::EpisodeBoundary::Inactivity, 120)),
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
}

fn install_v4_import_with_birth(path: &Path) -> Vec<u8> {
    let current_path = path.with_file_name("v4-current.rel");
    let mut current = Cva::create_project(&current_path).unwrap();
    create_import_memory(&mut current);
    crate::insomnia::completion_migration_tests::copy_with_legacy_v6_completion(&mut current, path);
    drop(current);

    // V4 binds its input cut to the physical end of the legacy source. Build
    // the legacy completion and record carriers first, then derive the V4
    // settlement from that exact V1 prefix.
    let mut source_bytes = fs::read(path).unwrap();
    source_bytes[8..10].copy_from_slice(&2_u16.to_le_bytes());
    fs::write(path, source_bytes).unwrap();
    let mut cva = Cva::open(path).unwrap();
    let prepared = cva
        .freeze_historical_import()
        .unwrap()
        .prepare(&[])
        .unwrap();
    let mut old = prepared.settlement.clone();
    old.semantic = None;
    old.epoch = None;
    old.input_bytes = prepared.source_bytes;
    let bytes = old.encode();
    assert!(bytes.starts_with(b"CVAFRS04"));
    cva.container.set_owner_write_scope(true);
    cva.container.append(&bytes).unwrap();
    cva.sync().unwrap();
    drop(cva);

    let mut source_bytes = fs::read(path).unwrap();
    source_bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    source_bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, source_bytes).unwrap();
    bytes
}

fn epoch_prefix_digest(path: &Path, settlement_offset: u64) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary:import-immutable-prefix:v1");
    let mut scan = Container::open_read_only_for_scan(path).unwrap();
    scan.visit_payloads::<ContainerError>(|object, payload| {
        let offset = u64::from_le_bytes(object.legacy_bytes()[..8].try_into().unwrap());
        if offset != settlement_offset
            && !crate::conversation_compaction_codec::is_compaction_payload(payload)
        {
            hash.update(object.legacy_bytes());
            hash.update(payload);
        }
        Ok(())
    })
    .unwrap();
    hash.finalize().into()
}

fn install_legacy_settlement(path: &Path, version: LegacySettlementVersion) -> Vec<u8> {
    if matches!(version, LegacySettlementVersion::V4) {
        return install_v4_import_with_birth(path);
    }
    let mut cva = Cva::create_project(path).unwrap();
    cva.import_native(&empty_import(), 100).unwrap();
    let physical = cva.import_epoch_scope().unwrap();
    cva.reconstruct_historical_freshness(Default::default())
        .unwrap();
    let mut old = cva.freshness.latest_import_settlement().unwrap().clone();
    let settlement_offset = cva
        .container
        .object_payloads()
        .unwrap()
        .into_iter()
        .filter_map(|(object, payload)| {
            payload
                .starts_with(b"CVAFRS06")
                .then(|| u64::from_le_bytes(object.legacy_bytes()[..8].try_into().unwrap()))
        })
        .next()
        .expect("current semantic settlement is present");

    let semantic = old.semantic.take().unwrap();
    old.input_bytes = settlement_offset;
    old.epoch = match version {
        LegacySettlementVersion::V4 => None,
        LegacySettlementVersion::V5 => Some(crate::freshness_storage::ImportEventScope {
            owner: old.owner,
            import_id: old.import_id,
            start_evidence_digest: old.start_evidence_digest,
            born_memories: semantic.born_memories,
            affected_recipients: semantic.affected_recipients,
            source_nodes: semantic.source_nodes,
            start_prefix: physical.start_offset,
            freeze_prefix: settlement_offset,
            base_cut: old.base_cut,
            freeze_cut: old.settlement_cut,
        }),
    };
    if matches!(version, LegacySettlementVersion::V5) {
        old.epoch_prefix_digest = epoch_prefix_digest(path, settlement_offset);
    }
    let bytes = old.encode();
    match version {
        LegacySettlementVersion::V4 => assert!(bytes.starts_with(b"CVAFRS04")),
        LegacySettlementVersion::V5 => assert!(bytes.starts_with(b"CVAFRS05")),
    }
    drop(cva);

    fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_len(settlement_offset)
        .unwrap();
    let mut container = Container::open(path).unwrap();
    container.append(&bytes).unwrap();
    container.sync().unwrap();
    drop(container);

    let mut file = fs::OpenOptions::new().write(true).open(path).unwrap();
    use std::io::{Seek, SeekFrom, Write};
    file.seek(SeekFrom::Start(8)).unwrap();
    file.write_all(&1_u16.to_le_bytes()).unwrap();
    file.write_all(&0_u16.to_le_bytes()).unwrap();
    file.sync_all().unwrap();
    bytes
}

fn assert_upgrade_preserves_legacy_settlement(version: LegacySettlementVersion) {
    let scratch = Scratch::new();
    let source_path = scratch.path("source.rel");
    let destination = scratch.path("upgraded.rel");
    let original_legacy = install_legacy_settlement(&source_path, version);
    let source_bytes = fs::read(&source_path).unwrap();
    let mut source = Cva::open_v1_read_only(&source_path).unwrap();
    let owner = source.owner_uuid();
    let global = source.latest_global_version();
    let archive = source.archive_version();
    let memory = source.memory_version();
    let source_settlement = source.freshness.latest_import_settlement().unwrap().clone();
    assert_eq!(source_settlement.semantic, None);
    let source_fingerprint = crate::historical_freshness_fingerprint::freshness_at_turn(
        &source,
        source.rel_turn_count(),
    )
    .unwrap()
    .fingerprint;
    let source_times = (0..=global)
        .map(|version| source.transaction_time_ns(version))
        .collect::<Vec<_>>();
    drop(source);

    let plan = crate::storage_upgrade::plan_storage_upgrade(
        crate::storage_upgrade::StorageUpgradeRequest {
            source: source_path.clone(),
            destination: destination.clone(),
        },
    )
    .unwrap();
    let staged = crate::storage_upgrade::stage_memory_storage_upgrade(&plan).unwrap();
    let output_path = staged.test_path();
    let mut output = Cva::open(output_path).unwrap();
    assert_eq!(output.owner_uuid(), owner);
    assert_eq!(output.latest_global_version(), global);
    assert_eq!(output.archive_version(), archive);
    assert_eq!(output.memory_version(), memory);
    let current = output.freshness.latest_import_settlement().unwrap();
    assert_eq!(current.import_id, source_settlement.import_id);
    assert!(current.semantic.is_some());
    assert_eq!(current.records, source_settlement.records);
    assert_eq!(current.cohort, source_settlement.cohort);
    assert_eq!(output.normalize_import_settlements().unwrap(), 0);
    assert_eq!(
        crate::historical_freshness_fingerprint::freshness_at_turn(
            &output,
            output.rel_turn_count()
        )
        .unwrap()
        .fingerprint,
        source_fingerprint
    );
    assert_eq!(
        (0..=global)
            .map(|version| output.transaction_time_ns(version))
            .collect::<Vec<_>>(),
        source_times
    );
    drop(output);

    let mut found_exact_legacy = false;
    let mut container = Container::open(output_path).unwrap();
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            found_exact_legacy |= payload == original_legacy;
            Ok(())
        })
        .unwrap();
    assert!(
        found_exact_legacy,
        "migration retains original settlement bytes"
    );
    assert_eq!(fs::read(&source_path).unwrap(), source_bytes);
    let output_bytes = fs::read(output_path).unwrap();
    assert_eq!(
        u16::from_le_bytes(output_bytes[8..10].try_into().unwrap()),
        2
    );
}

#[test]
fn v4_native_import_settlement_migrates_to_canonical_semantics_without_source_changes() {
    assert_upgrade_preserves_legacy_settlement(LegacySettlementVersion::V4);
}

#[test]
fn v5_native_import_settlement_migrates_to_canonical_semantics_without_source_changes() {
    assert_upgrade_preserves_legacy_settlement(LegacySettlementVersion::V5);
}
