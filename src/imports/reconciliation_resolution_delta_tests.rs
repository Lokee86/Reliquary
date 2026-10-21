use super::*;
use crate::{EntityId, EntityResolutionReason, MemoryEntityResolutionStatus};

#[test]
fn entity_resolution_retained_interval_keeps_inert_original_and_frozen_visibility() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!(
            "retained-resolution-import-{}",
            uuid::Uuid::new_v4()
        ));
    std::fs::create_dir_all(&root).unwrap();
    let source_path = root.join("source.rel");
    let target_path = root.join("target.rel");
    let raw = b"CVAERS02literal old evidence with unexplained trailing scalar";
    let mut source = Cva::create(&source_path).unwrap();
    source.container.append(raw).unwrap();
    source.sync().unwrap();
    drop(source);
    let mut bytes = std::fs::read(&source_path).unwrap();
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    std::fs::write(&source_path, &bytes).unwrap();
    let mut source = Cva::open_v1_read_only(&source_path).unwrap();
    let mut target = Cva::create(&target_path).unwrap();
    assert!(inventory(&mut source).unwrap().is_empty());
    let chunks = source.container.chunks().unwrap();
    let mut coordinates =
        crate::imports::reconciliation_interval::Coordinates::shared(&mut source, chunks.len())
            .unwrap();
    crate::imports::reconciliation_interval::replay_frame(
        &mut source,
        &mut target,
        chunks.len() - 1,
        raw,
        &mut coordinates,
    )
    .unwrap();
    target.sync().unwrap();
    assert!(target.entity_resolutions.records().is_empty());
    assert!(inventory(&mut target).unwrap().is_empty());
    drop(target);
    let mut target = Cva::open(&target_path).unwrap();
    let chunks = target.container.chunks().unwrap();
    let payload = target.container.read(*chunks.last().unwrap()).unwrap();
    assert_eq!(
        crate::entity_resolution_codec::retained::decode(&payload).unwrap(),
        Some(raw.as_slice())
    );
    let reclaimed = root.join("reclaimed.rel");
    target.reclaim_storage(&reclaimed).unwrap();
    let mut cleaned = Cva::open(&reclaimed).unwrap();
    let chunks = cleaned.container.chunks().unwrap();
    let payload = cleaned.container.read(*chunks.last().unwrap()).unwrap();
    assert_eq!(
        crate::entity_resolution_codec::retained::decode(&payload).unwrap(),
        Some(raw.as_slice())
    );
    assert!(target.entity_resolutions.records().is_empty());
    assert_eq!(std::fs::read(&source_path).unwrap(), bytes);
}

#[test]
fn entity_resolution_delta_interval_uses_source_postimage_and_destination_evidence_base() {
    let (mut source, _, keys, source_path) =
        crate::entity_resolution_test_support::rel_with_three_mentions();
    let key = keys[0];
    source
        .put_entity_unresolved(
            key,
            0,
            vec![EntityId([1; 32])],
            EntityResolutionReason::Ambiguous,
            [1; 32],
            [2; 32],
            10,
        )
        .unwrap();
    source.sync().unwrap();
    let target_path = source_path.with_extension("target.rel");
    std::fs::copy(&source_path, &target_path).unwrap();
    source
        .put_entity_unresolved(
            key,
            1,
            vec![EntityId([1; 32])],
            EntityResolutionReason::Ambiguous,
            [3; 32],
            [2; 32],
            20,
        )
        .unwrap();
    source.sync().unwrap();
    let expected = source.entity_resolution(key).unwrap().clone();
    let mut target = Cva::open(&target_path).unwrap();
    // A structurally accepted destination prefix may have different inherited
    // candidates and context at the same local revision. Never copy source delta
    // omissions into that unrelated evidence base.
    let mut prior = expected.clone();
    prior.revision = 1;
    if let MemoryEntityResolutionStatus::Pending(v) = &mut prior.status {
        v.candidate_entity_ids = vec![EntityId([2; 32])];
        v.context_fingerprint = [9; 32];
        v.last_attempt_at_ns = 10;
        v.attempt_count = 1;
    }
    let mut destination_projection =
        crate::entity_resolution_store::EntityResolutionStore::default();
    destination_projection
        .ingest(&crate::entity_resolution_codec::encode_resolution(&prior).unwrap())
        .unwrap();
    target.entity_resolutions = destination_projection;
    let chunks = source.container.chunks().unwrap();
    let index = chunks.len() - 1;
    let payload = source.container.read(chunks[index]).unwrap();
    assert!(crate::entity_resolution_codec::delta::is_current(&payload));
    let before = std::fs::read(&source_path).unwrap();
    let mut coordinates =
        crate::imports::reconciliation_interval::Coordinates::shared(&mut source, index).unwrap();
    crate::imports::reconciliation_interval::replay_frame(
        &mut source,
        &mut target,
        index,
        &payload,
        &mut coordinates,
    )
    .unwrap();
    assert_eq!(target.entity_resolution(key), Some(&expected));
    assert_eq!(std::fs::read(&source_path).unwrap(), before);
    let change = crate::entity_resolution_store::EntityResolutionStore::materialized_changes(
        &mut source.container,
    )
    .unwrap()
    .remove(&chunks[index])
    .unwrap();
    assert_eq!(
        crate::entity_resolution_codec::comparison_carrier(&change).unwrap(),
        crate::entity_resolution_codec::encode_resolution(&expected).unwrap()
    );
}
