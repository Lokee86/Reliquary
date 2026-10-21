use super::*;
use crate::imports::{ImportCompletionOutcome, NativeImport};
use std::fs;

#[test]
fn latest_upgrade_rebinds_completed_native_import_and_removes_normalization_duplicate() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("standalone-import-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("source.rel");
    let output = root.join("current.rel");
    let mut owner = Cva::create(&path).unwrap();
    owner
        .import_native(
            &NativeImport {
                schema_version: 1,
                source_namespace: "standalone-proof".into(),
                source_format: "test-v1".into(),
                conversations: Vec::new(),
            },
            100,
        )
        .unwrap();
    let physical = owner.import_epoch_scope().unwrap();
    owner
        .reconstruct_historical_freshness(Default::default())
        .unwrap();
    let mut old = owner.freshness.latest_import_settlement().unwrap().clone();
    let mut prefix = None;
    owner
        .container
        .visit_payloads::<crate::ContainerError>(|object, payload| {
            if payload.starts_with(b"CVAFRS06") {
                prefix = Some(object.offset());
            }
            Ok(())
        })
        .unwrap();
    let prefix = prefix.unwrap();
    drop(owner);
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(prefix)
        .unwrap();
    let semantic = old.semantic.take().unwrap();
    old.input_bytes = prefix;
    old.epoch = Some(crate::freshness_storage::ImportEventScope {
        owner: old.owner,
        import_id: old.import_id,
        start_evidence_digest: old.start_evidence_digest,
        born_memories: semantic.born_memories,
        affected_recipients: semantic.affected_recipients,
        source_nodes: semantic.source_nodes,
        start_prefix: physical.start_offset,
        freeze_prefix: prefix,
        base_cut: old.base_cut,
        freeze_cut: old.settlement_cut,
    });
    let mut hash = Sha256::new();
    hash.update(b"reliquary:import-immutable-prefix:v1");
    let mut scan = Container::open_read_only_for_scan(&path).unwrap();
    scan.visit_payloads::<crate::ContainerError>(|object, payload| {
        if !crate::conversation_compaction_codec::is_compaction_payload(payload) {
            hash.update(object.legacy_bytes());
            hash.update(payload);
        }
        Ok(())
    })
    .unwrap();
    old.epoch_prefix_digest = hash.finalize().into();
    drop(scan);
    let mut owner = Cva::open(&path).unwrap();
    owner.container.set_owner_write_scope(true);
    owner.container.append(&old.encode()).unwrap();
    owner.sync().unwrap();
    drop(owner);
    let mut owner = Cva::open(&path).unwrap();
    let id = owner.open_import_epoch().unwrap().import_id;
    let ImportCompletionOutcome::Completed {
        complete: mut expected,
        committed: true,
    } = owner.complete_native_import(&id, 200).unwrap()
    else {
        panic!("completion failed")
    };
    assert_eq!(owner.normalize_import_settlements().unwrap(), 1);
    let readiness = owner.native_import_readiness(&id).unwrap_err().to_string();
    let original = fs::read(&path).unwrap();
    let mut source = Container::open_read_only_for_scan(&path).unwrap();
    let plan = prepare(&mut source, &mut owner).unwrap();
    assert_eq!(plan.omitted.len(), 1);
    let mut target = Container::create_with_identity_and_uuid(
        &output,
        source.identity().unwrap(),
        source.owner_uuid().unwrap(),
    )
    .unwrap();
    source
        .visit_payloads::<StorageUpgradeError>(|object, payload| {
            if !plan.omitted.contains(&object) {
                target.append(
                    plan.replacements
                        .get(&object)
                        .map(Vec::as_slice)
                        .unwrap_or(payload),
                )?;
            }
            Ok(())
        })
        .unwrap();
    target.sync().unwrap();
    drop(target);
    let mut current = Cva::open(&output).unwrap();
    current.validate_historical_import_settlements().unwrap();
    current.validate_import_completion_settlements().unwrap();
    let settlement = current.freshness.latest_import_settlement().unwrap();
    assert_eq!(
        settlement.semantic.as_ref().unwrap().predecessor_digest,
        None
    );
    expected.historical_freshness_settlement_digest =
        format!("{:x}", Sha256::digest(settlement.encode()));
    assert!(
        matches!(current.complete_native_import(&id, 999).unwrap(), ImportCompletionOutcome::Completed { complete, committed: false } if complete == expected)
    );
    assert_eq!(
        current
            .native_import_readiness(&id)
            .unwrap_err()
            .to_string(),
        readiness
    );
    let mut count = 0;
    current
        .container
        .visit_payloads::<crate::ContainerError>(|_, payload| {
            assert!(!payload.starts_with(b"CVAFRS05"));
            count += usize::from(payload.starts_with(b"CVAFRS06"));
            Ok(())
        })
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(fs::read(&path).unwrap(), original);
    drop(current);
    drop(source);
    drop(owner);
    fs::remove_dir_all(root).unwrap();
}
