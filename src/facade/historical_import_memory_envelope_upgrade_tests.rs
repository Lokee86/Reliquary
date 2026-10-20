//! Staging a completed legacy import epoch while converting accepted Memory
//! links to the current publication-envelope format.
//!
//! Exercises the concrete private stage before any promotion API is exposed.

use super::*;
use crate::container::publication_envelope;
use crate::historical_memory_fingerprint::MemoryHistory;
use crate::memory_compact_codec::publication::{self, MemoryPublication};
use sha2::{Digest, Sha256};
use std::fs;

fn payloads(path: &Path, v1: bool) -> Vec<Vec<u8>> {
    let mut container = if v1 {
        crate::Container::open_v1_read_only(path).unwrap()
    } else {
        crate::Container::open(path).unwrap()
    };
    let mut result = Vec::new();
    container
        .visit_payloads::<crate::ContainerError>(|_, payload| {
            result.push(payload.to_vec());
            Ok(())
        })
        .unwrap();
    result
}

/// Place a real standalone reservation immediately before the completed
/// import wrapper, then append an accepted legacy Memory marker after it.
/// The reservation is inside the wrapper's relocated physical prefix while
/// the semantic binding remains beyond the epoch boundary.
fn append_post_epoch_reserved_memory_publication(
    source_path: &Path,
    epoch: &crate::imports::LegacyImportEpoch,
    memory_id: crate::MemoryId,
) -> (u64, crate::imports::LegacyImportEpoch) {
    let mut source = crate::Container::open(source_path).unwrap();
    let identity = source.identity().unwrap();
    let owner_uuid = source.owner_uuid().unwrap();
    let mut frames = Vec::new();
    let mut wrapper_frame = None;
    source
        .visit_payloads::<crate::ContainerError>(|_, payload| {
            if payload.starts_with(b"CVAILEG") {
                let original = crate::imports::legacy_epoch_transport::original_payload(payload)
                    .unwrap()
                    .unwrap_or(payload);
                assert_eq!(
                    crate::imports::legacy_epoch::decode(original).unwrap(),
                    Some(epoch.clone())
                );
                wrapper_frame = Some(original.to_vec());
            } else {
                frames.push(payload.to_vec());
            }
            Ok(())
        })
        .unwrap();
    drop(source);
    let _wrapper_frame = wrapper_frame.expect("fixture has its completed import wrapper");

    let owner = Cva::open(source_path).unwrap();
    let previous = owner
        .memories
        .records()
        .iter()
        .rev()
        .find(|record| record.id == memory_id)
        .expect("imported Memory has an accepted revision")
        .clone();
    let global_head = owner.latest_global_version();
    let local_head = owner.memory_version();
    drop(owner);

    let rebuilt_path = source_path.with_extension("boundary-rebuilt.rel");
    let mut destination =
        crate::Container::create_with_identity_and_uuid(&rebuilt_path, identity, owner_uuid)
            .unwrap();
    for payload in frames {
        destination.append(&payload).unwrap();
    }
    // Raw transport does not allocate in the resident clock. Reopen the
    // copied prefix before allocating the new real standalone reservation.
    destination.sync().unwrap();
    drop(destination);
    let mut destination = crate::Container::open(&rebuilt_path).unwrap();
    let reserved_global = destination
        .allocate_version_with_transaction_time(Some(-777))
        .unwrap();
    assert_eq!(reserved_global, global_head + 1);
    let prefix_len = fs::metadata(&rebuilt_path).unwrap().len();
    let mut prefix = fs::read(&rebuilt_path).unwrap();
    // Bind the final V1 bytes, including the header installed below.
    prefix[8..10].copy_from_slice(&1_u16.to_le_bytes());
    prefix[10..12].copy_from_slice(&0_u16.to_le_bytes());
    // This is a genuine V1 proof: its own source prefix includes the
    // newly inserted reservation. Relocated CVAILEG2 wrappers are V2-only.
    let mut epoch = epoch.clone();
    epoch.source_prefix_len = prefix_len;
    epoch.source_prefix_digest = format!("{:x}", Sha256::digest(&prefix));
    destination.append(&legacy_frame_bytes(&epoch)).unwrap();

    let mut next = previous;
    next.revision += 1;
    next.memory_version = local_head + 1;
    next.global_version = reserved_global;
    next.scope = "post-import-boundary".into();
    next.mutation_id = "post-import-reserved-binding".into();
    next.updated_at_ns += 1;
    let carrier = destination
        .append(&crate::memory_codec::encode_record(&next).unwrap())
        .unwrap();
    destination
        .append(&crate::memory_codec::encode_version(
            crate::memory_codec::MemoryVersion {
                global_version: reserved_global,
                memory_version: next.memory_version,
                record: carrier,
            },
        ))
        .unwrap();
    destination.sync().unwrap();
    drop(destination);

    let mut bytes = fs::read(&rebuilt_path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(source_path, bytes).unwrap();
    fs::remove_file(rebuilt_path).unwrap();

    // Validate the handoff fixture with the real V1 owner replay before it is
    // used to assert converter behavior.
    let mut accepted = Cva::open_v1_read_only(source_path).unwrap();
    assert_eq!(accepted.legacy_import_epochs(), vec![epoch.clone()]);
    assert_eq!(accepted.latest_global_version(), reserved_global);
    assert_eq!(accepted.memory_version(), local_head + 1);
    assert_eq!(
        accepted
            .memory_revision(memory_id, next.revision)
            .unwrap()
            .scope,
        "post-import-boundary"
    );
    assert_eq!(accepted.transaction_time_ns(reserved_global), Some(-777));
    (reserved_global, epoch)
}

#[test]
fn completed_import_epoch_memory_upgrade_rewrites_accepted_links_and_preserves_evidence() {
    let scratch = Scratch::new();
    let source_path = scratch.path("completed-legacy-memory.rel");
    let destination = scratch.path("completed-envelope-memory.not-published");
    let (expected_epoch, _source_prefix, _legacy_frame, inert, memory_id) =
        owner_with_legacy_epoch(&source_path, true);
    let (orphan_body, orphan_record) = inert.unwrap();
    let (reserved_global, expected_epoch) =
        append_post_epoch_reserved_memory_publication(&source_path, &expected_epoch, memory_id);

    let source_bytes = fs::read(&source_path).unwrap();
    let source_modified = fs::metadata(&source_path).unwrap().modified().unwrap();
    let source_digest = Sha256::digest(&source_bytes);
    let mut source = Cva::open_v1_read_only(&source_path).unwrap();
    assert_eq!(source.legacy_import_epochs(), vec![expected_epoch.clone()]);
    assert!(
        source.import_epoch_statuses().is_empty(),
        "legacy evidence does not fabricate a modern native import epoch"
    );
    let source_memory = source.memory(memory_id).unwrap();
    let source_records = source.memories.records().to_vec();
    assert!(
        !source_records.is_empty(),
        "fixture has accepted Memory history"
    );
    assert!(source_records.iter().any(|record| record.id == memory_id));
    let source_global_head = source.latest_global_version();
    let source_local_head = source.memory_version();
    assert!(source_local_head >= source_records.len() as u64);

    let source_payloads = payloads(&source_path, true);
    let grouped_completion_frames = source_payloads
        .iter()
        .filter(|payload| payload.starts_with(b"CVAINSC"))
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        !grouped_completion_frames.is_empty(),
        "legacy import source contains grouped Insomnia completion evidence"
    );
    assert!(
        source_payloads
            .iter()
            .any(|payload| payload.starts_with(b"CVAMEMV1")),
        "source has direct legacy Memory marker links"
    );
    let reservation_slot = source_payloads
        .iter()
        .position(|payload| {
            payload.starts_with(b"CVAVERS2")
                && u64::from_le_bytes(payload[8..16].try_into().unwrap()) == reserved_global
        })
        .expect("source retains the reserved ticket");
    let epoch_slot = source_payloads
        .iter()
        .position(|payload| payload.starts_with(b"CVAILEG1"))
        .expect("source retains its completed epoch wrapper");
    let marker_slot = source_payloads
        .iter()
        .position(|payload| {
            crate::memory_codec::decode_version(payload)
                .unwrap()
                .is_some_and(|version| version.global_version == reserved_global)
        })
        .expect("source has the delayed accepted Memory marker");
    assert!(reservation_slot < epoch_slot && epoch_slot < marker_slot);
    let accepted_marker_count = source_payloads
        .iter()
        .filter(|payload| {
            crate::memory_codec::decode_version(payload)
                .unwrap()
                .is_some()
        })
        .count();
    assert!(
        accepted_marker_count > 0,
        "fixture has standalone legacy links"
    );
    let expected_global_tickets = source_payloads
        .iter()
        .filter_map(|payload| {
            crate::memory_codec::decode_version(payload)
                .unwrap()
                .map(|version| version.global_version)
        })
        .collect::<std::collections::BTreeSet<_>>();
    let source_grouped = grouped_completion_frames
        .iter()
        .map(|payload| {
            crate::insomnia::completion::decode_completion(payload)
                .unwrap()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(
        source_payloads
            .iter()
            .any(|payload| payload == &orphan_body)
    );
    assert!(
        source_payloads
            .iter()
            .any(|payload| payload == &orphan_record)
    );

    let source_history = MemoryHistory::read(&mut source.container, &source.memories).unwrap();
    let source_times = (0..=source_global_head)
        .map(|global| (global, source.transaction_time_ns(global)))
        .collect::<Vec<_>>();
    let source_epoch_statuses = source.import_epoch_statuses();
    let mut freshness_cuts = std::collections::BTreeSet::from([0, source.rel_turn_count()]);
    for settlement in source.freshness.import_settlements() {
        freshness_cuts.insert(settlement.base_cut);
        freshness_cuts.insert(settlement.settlement_cut);
    }
    let source_freshness = freshness_cuts
        .iter()
        .map(|cut| {
            (
                *cut,
                crate::historical_freshness_fingerprint::freshness_at_turn(&source, *cut)
                    .unwrap()
                    .fingerprint,
            )
        })
        .collect::<Vec<_>>();
    drop(source);

    let plan = crate::storage_upgrade::plan_storage_upgrade(
        crate::storage_upgrade::StorageUpgradeRequest {
            source: source_path.clone(),
            destination: destination.clone(),
        },
    )
    .unwrap();
    let staged = crate::storage_upgrade::stage_memory_envelope_storage_upgrade(&plan).unwrap();
    let staged_path = staged.test_path();
    let staged_payloads = payloads(staged_path, false);

    let mut converted = Vec::new();
    for payload in &staged_payloads {
        if let Some(envelope) = publication_envelope::decode(payload).unwrap() {
            let publication = publication::decode(envelope.body, envelope.global_version)
                .unwrap()
                .expect("Memory envelope publication");
            converted.push((envelope.global_version, publication));
        }
    }
    converted.sort_by_key(|(global, _)| *global);
    assert_eq!(
        converted.len(),
        accepted_marker_count,
        "one envelope per standalone legacy link"
    );
    assert_eq!(
        converted
            .iter()
            .map(|(global, _)| *global)
            .collect::<std::collections::BTreeSet<_>>(),
        expected_global_tickets,
        "envelope preserves the accepted legacy links' global tickets"
    );
    assert!(
        converted.iter().any(|(_, publication)| matches!(
            publication,
            MemoryPublication::Delta(delta) if delta.id == memory_id
        )),
        "fixture contains a direct revision after grouped import completion"
    );
    assert!(
        staged_payloads.iter().any(|payload| {
            publication_envelope::decode(payload)
                .unwrap()
                .is_some_and(|envelope| {
                    envelope.global_version == reserved_global
                        && envelope.clock
                            == crate::container::publication_envelope::PublicationClock::BindReserved
                })
        }),
        "post-epoch Memory marker binds the pre-epoch reservation"
    );
    assert!(
        converted.iter().all(|(_, publication)| !matches!(
            publication,
            MemoryPublication::Creation(creation) if creation.record.id == memory_id
        )),
        "grouped imported creation remains in grouped completion history"
    );
    assert!(
        staged_payloads
            .iter()
            .all(|payload| !payload.starts_with(b"CVAMEMV1"))
    );
    let staged_grouped = staged_payloads
        .iter()
        .filter(|payload| payload.starts_with(b"CVAINSC"))
        .map(|payload| {
            crate::insomnia::completion::decode_completion(payload)
                .unwrap()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(staged_grouped.len(), source_grouped.len());
    for (before, after) in source_grouped.iter().zip(&staged_grouped) {
        assert_eq!(before.episode_id, after.episode_id);
        assert_eq!(before.attempt, after.attempt);
        assert_eq!(before.started_at_ns, after.started_at_ns);
        assert_eq!(before.completed_at_ns, after.completed_at_ns);
        assert_eq!(before.transaction_time_ns, after.transaction_time_ns);
        assert_eq!(before.extractor_model, after.extractor_model);
        assert_eq!(before.extractor_version, after.extractor_version);
        assert_eq!(before.rejected_count, after.rejected_count);
        assert_eq!(before.memory_ids, after.memory_ids);
        assert_eq!(before.external_memory_refs, after.external_memory_refs);
        assert_eq!(before.global_version_start, after.global_version_start);
        assert_eq!(before.records, after.records);
        assert_eq!(before.routing_metadata, after.routing_metadata);
        assert_eq!(before.freshness_birth_turn, after.freshness_birth_turn);
        assert_eq!(
            before.freshness_policy_version,
            after.freshness_policy_version
        );
        assert_eq!(
            before.freshness_birth_memory_ids,
            after.freshness_birth_memory_ids
        );
        let body_rows = |completion: &crate::insomnia::completion::InsomniaCompletion| {
            completion
                .bodies
                .iter()
                .map(|body| (body.id, body.bytes.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(body_rows(before), body_rows(after));
    }
    assert!(
        staged_payloads
            .iter()
            .any(|payload| payload == &orphan_body)
    );
    assert!(
        staged_payloads
            .iter()
            .any(|payload| payload == &orphan_record)
    );

    // The relocated wrapper keeps the exact completed epoch and binds it to
    // the actual byte prefix preceding the wrapper in the staged container.
    let mut staged_container = crate::Container::open(staged_path).unwrap();
    let mut wrapped_epoch = None;
    staged_container
        .visit_payloads::<crate::ContainerError>(|_, payload| {
            if payload.starts_with(b"CVAILEG2") {
                wrapped_epoch = Some(payload.to_vec());
            }
            Ok(())
        })
        .unwrap();
    let (frame, binding) = crate::imports::legacy_epoch_transport::decode(
        &wrapped_epoch.expect("staged owner retains its relocated epoch wrapper"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(frame, expected_epoch);
    let staged_bytes = fs::read(staged_path).unwrap();
    let prefix_len = usize::try_from(binding.length).unwrap();
    assert!(prefix_len <= staged_bytes.len());
    assert_eq!(
        binding.digest.as_slice(),
        Sha256::digest(&staged_bytes[..prefix_len]).as_slice()
    );

    let mut reopened = Cva::open(staged_path).unwrap();
    assert_eq!(reopened.legacy_import_epochs(), vec![expected_epoch]);
    assert_eq!(reopened.import_epoch_statuses(), source_epoch_statuses);
    assert_eq!(reopened.memory(memory_id).unwrap(), source_memory);
    assert_eq!(reopened.memories.records(), source_records);
    assert_eq!(reopened.latest_global_version(), source_global_head);
    assert_eq!(reopened.memory_version(), source_local_head);
    let migrated_history =
        MemoryHistory::read(&mut reopened.container, &reopened.memories).unwrap();
    for cut in 0..=source_local_head {
        let before = source_history.at(cut).unwrap();
        let after = migrated_history.at(cut).unwrap();
        assert_eq!(
            before.fingerprint, after.fingerprint,
            "Memory history cut {cut}"
        );
        assert_eq!(
            before.current, after.current,
            "current Memory membership at cut {cut}"
        );
    }
    for record in &source_records {
        assert_eq!(
            source_history.revision(record.id, record.revision).unwrap(),
            migrated_history
                .revision(record.id, record.revision)
                .unwrap(),
            "body and routing semantics for {} revision {}",
            format!("{:?}", record.id.0),
            record.revision
        );
    }
    for (global, time) in source_times {
        assert_eq!(
            reopened.transaction_time_ns(global),
            time,
            "global time {global}"
        );
    }
    for (cut, fingerprint) in source_freshness {
        assert_eq!(
            crate::historical_freshness_fingerprint::freshness_at_turn(&reopened, cut)
                .unwrap()
                .fingerprint,
            fingerprint,
            "canonical normalized Freshness proof at activity cut {cut}"
        );
    }

    let after_bytes = fs::read(&source_path).unwrap();
    let after_modified = fs::metadata(&source_path).unwrap().modified().unwrap();
    assert_eq!(
        after_bytes, source_bytes,
        "staging leaves source bytes untouched"
    );
    assert_eq!(Sha256::digest(&after_bytes), source_digest);
    assert_eq!(
        after_modified, source_modified,
        "staging leaves source mtime untouched"
    );
    drop(staged);
    assert!(
        !destination.exists(),
        "staging does not publish destination"
    );
}
