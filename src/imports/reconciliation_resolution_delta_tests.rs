use super::*;
use crate::{EntityId, EntityResolutionReason, MemoryEntityResolutionStatus};

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
    let mut destination_projection = crate::entity_resolution_store::EntityResolutionStore::default();
    let destination_bytes = crate::entity_resolution_codec::delta::encode(
        &crate::entity_resolution_codec::DecodedEntityResolution {
            key: crate::entity_resolution_boundary::store_key(prior.key),
            revision: prior.revision,
            updated_at_ns: prior.updated_at_ns,
            status: Some(prior.status.clone()),
        },
        None,
    ).unwrap();
    destination_projection.ingest(&destination_bytes).unwrap();
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
    assert_eq!(target.entity_resolution(key), Some(expected.clone()));
    assert_eq!(std::fs::read(&source_path).unwrap(), before);
    let change = crate::entity_resolution_store::EntityResolutionStore::materialized_changes(
        &mut source.container,
    )
    .unwrap()
    .remove(&chunks[index])
    .unwrap();
    assert_eq!(
        crate::entity_resolution_codec::comparison_carrier(&change).unwrap(),
        crate::entity_resolution_codec::comparison_carrier(
            &crate::entity_resolution_codec::DecodedEntityResolution {
                key: crate::entity_resolution_boundary::store_key(expected.key),
                revision: expected.revision,
                updated_at_ns: expected.updated_at_ns,
                status: Some(expected.status),
            }
        ).unwrap()
    );
}
