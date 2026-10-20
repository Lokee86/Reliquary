//! Step178 authoritative Memory envelope migration proof.

use super::*;
use crate::container::publication_envelope::{self, PublicationClock};
use crate::memory_compact_codec::publication::{self, MemoryPublication};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default, Serialize)]
struct MemoryEnvelopeBytes {
    legacy_postimage_count: u64,
    legacy_postimage_bytes: u64,
    legacy_postimage_physical_frame_bytes: u64,
    legacy_link_count: u64,
    legacy_link_bytes: u64,
    legacy_link_physical_frame_bytes: u64,
    legacy_terminal_count: u64,
    legacy_terminal_bytes: u64,
    legacy_terminal_physical_frame_bytes: u64,
    body_object_count: u64,
    body_object_bytes: u64,
    routing_metadata_count: u64,
    routing_metadata_bytes: u64,
    memory_envelope_count: u64,
    memory_envelope_body_bytes: u64,
    memory_envelope_framing_bytes: u64,
    memory_envelope_frame_bytes: u64,
    creation_count: u64,
    delta_count: u64,
    terminal_envelope_count: u64,
    grouped_completion_count: u64,
    grouped_completion_bytes: u64,
    external_memory_publication_count: u64,
    external_memory_publication_bytes: u64,
}

fn memory_envelope_bytes(path: &Path, v1: bool) -> MemoryEnvelopeBytes {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut result = MemoryEnvelopeBytes::default();
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            let len = payload.len() as u64;
            if payload.starts_with(b"CVAMEMR") {
                result.legacy_postimage_count += 1;
                result.legacy_postimage_bytes += len;
                result.legacy_postimage_physical_frame_bytes += len + 8;
            } else if payload.starts_with(b"CVAMEMV1") {
                result.legacy_link_count += 1;
                result.legacy_link_bytes += len;
                result.legacy_link_physical_frame_bytes += len + 8;
            } else if payload.starts_with(b"CVAMEMT1") {
                result.legacy_terminal_count += 1;
                result.legacy_terminal_bytes += len;
                result.legacy_terminal_physical_frame_bytes += len + 8;
            } else if payload.starts_with(b"CVAMBDY1") {
                result.body_object_count += 1;
                result.body_object_bytes += len;
            } else if payload.starts_with(b"CVAMRTE") {
                result.routing_metadata_count += 1;
                result.routing_metadata_bytes += len;
            } else if payload.starts_with(b"CVAINSC") {
                // These can contain atomic multi-owner publications. Count
                // their complete frame separately; do not allocate bytes to
                // Memory or claim Memory-only savings from them.
                result.grouped_completion_count += 1;
                result.grouped_completion_bytes += len;
            } else if payload.starts_with(b"CVAXMEM") {
                result.external_memory_publication_count += 1;
                result.external_memory_publication_bytes += len;
            } else if let Some(envelope) = publication_envelope::decode(payload).unwrap() {
                if !envelope.body.starts_with(b"CVAMC002")
                    && !envelope.body.starts_with(b"CVAMD002")
                    && !envelope.body.starts_with(b"CVAMT002")
                {
                    return Ok(());
                }
                assert_eq!(envelope.clock, PublicationClock::BindReserved);
                match publication::decode(envelope.body, envelope.global_version)
                    .unwrap()
                    .expect("recognized Memory envelope body")
                {
                    MemoryPublication::Creation(_) => result.creation_count += 1,
                    MemoryPublication::Delta(_) => result.delta_count += 1,
                    MemoryPublication::Tombstone { .. } => result.terminal_envelope_count += 1,
                }
                result.memory_envelope_count += 1;
                result.memory_envelope_body_bytes += envelope.body.len() as u64;
                result.memory_envelope_frame_bytes += len + 8;
                result.memory_envelope_framing_bytes += len - envelope.body.len() as u64 + 8;
            }
            Ok(())
        })
        .unwrap();
    result
}

fn advance_memory_head(memory_head: &mut u64, version: u64, context: &str) {
    assert_eq!(
        version,
        *memory_head + 1,
        "dense Memory version at {context}"
    );
    *memory_head = version;
}

/// Map each accepted physical high-water to the Memory local head visible at
/// that frontier. This models late-bound reserved tickets and grouped frames:
/// a link at global 1 after the scan has observed ticket 3 first becomes visible
/// at frontier 3; grouped records become visible together at the group's end.
fn memory_head_by_global_frontier(path: &Path, v1: bool, expected_head: u64) -> BTreeMap<u64, u64> {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut next_global = 1_u64;
    let mut transaction_times = BTreeMap::new();
    let mut reservations = BTreeSet::new();
    let mut memory_head = 0_u64;
    let mut ledger = BTreeMap::from([(0, 0)]);
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            crate::container_version::observe_version_payload_state(
                &mut next_global,
                &mut transaction_times,
                &mut reservations,
                payload,
            )?;

            if let Some(link) = crate::memory_codec::decode_version(payload).unwrap() {
                advance_memory_head(&mut memory_head, link.memory_version, "legacy Memory link");
            } else if let Some(envelope) = publication_envelope::decode(payload).unwrap() {
                if let Some(publication) =
                    publication::decode(envelope.body, envelope.global_version).unwrap()
                {
                    let local = match publication {
                        MemoryPublication::Creation(creation) => creation.record.memory_version,
                        MemoryPublication::Delta(delta) => delta.memory_version,
                        MemoryPublication::Tombstone { memory_version, .. } => memory_version,
                    };
                    advance_memory_head(&mut memory_head, local, "Memory envelope");
                }
            } else if let Some(completion) =
                crate::insomnia::completion::decode_completion(payload).unwrap()
            {
                for record in completion.records {
                    advance_memory_head(
                        &mut memory_head,
                        record.memory_version,
                        "grouped completion Memory record",
                    );
                }
            } else if let Some(external) =
                crate::external_source::publication::decode(payload).unwrap()
            {
                for record in external.records {
                    advance_memory_head(
                        &mut memory_head,
                        record.memory_version,
                        "external publication Memory record",
                    );
                }
            }

            ledger.insert(next_global.saturating_sub(1), memory_head);
            Ok(())
        })
        .unwrap();
    assert_eq!(memory_head, expected_head, "fully admitted Memory head");
    ledger
}

fn assert_memory_global_frontiers(source_path: &Path, staged_path: &Path, kind: FileKind) {
    let (source_head, staged_head, source_global, staged_global) = match kind {
        FileKind::Reliquary => {
            let source = Cva::open_v1_read_only(source_path).unwrap();
            let staged = Cva::open(staged_path).unwrap();
            (
                source.memory_version(),
                staged.memory_version(),
                source.latest_global_version(),
                staged.latest_global_version(),
            )
        }
        FileKind::Phylactery => {
            let source = Phylactery::open_v1_read_only(source_path).unwrap();
            let staged = Phylactery::open(staged_path).unwrap();
            (
                source.memory_version(),
                staged.memory_version(),
                source.latest_global_version(),
                staged.latest_global_version(),
            )
        }
    };
    assert_eq!(source_head, staged_head);
    assert_eq!(source_global, staged_global);
    let source_ledger = memory_head_by_global_frontier(source_path, true, source_head);
    let staged_ledger = memory_head_by_global_frontier(staged_path, false, staged_head);
    for global in 0..=source_global {
        let source_memory = source_ledger
            .range(..=global)
            .next_back()
            .map(|(_, memory)| *memory)
            .unwrap_or(0);
        let staged_memory = staged_ledger
            .range(..=global)
            .next_back()
            .map(|(_, memory)| *memory)
            .unwrap_or(0);
        assert_eq!(
            source_memory, staged_memory,
            "Memory head at global cut {global}"
        );
    }
}

fn assert_converted_memory_frame_counts(
    fixture: &str,
    before: &MemoryEnvelopeBytes,
    after: &MemoryEnvelopeBytes,
) {
    let expected_envelopes = before.legacy_link_count;
    assert_eq!(
        after.memory_envelope_count, expected_envelopes,
        "one BindReserved Memory envelope for every accepted legacy marker in {fixture}"
    );
    assert_eq!(
        after.legacy_link_count, 0,
        "legacy version links remain in {fixture}"
    );
    assert!(
        after.terminal_envelope_count <= before.legacy_terminal_count,
        "more linked terminal envelopes than source terminal carriers in {fixture}"
    );
    assert_eq!(
        after.legacy_postimage_count,
        before.legacy_postimage_count.saturating_sub(
            before
                .legacy_link_count
                .saturating_sub(after.terminal_envelope_count)
        ),
        "accepted postimages are replaced; inert unlinked postimages may remain in {fixture}"
    );
    assert_eq!(
        after.legacy_terminal_count,
        before
            .legacy_terminal_count
            .saturating_sub(after.terminal_envelope_count),
        "linked terminal carriers are replaced; inert terminal carriers may remain in {fixture}"
    );
    assert_eq!(
        after.grouped_completion_count, before.grouped_completion_count,
        "atomic grouped completion frames are retained in {fixture}"
    );
    assert_eq!(
        after.body_object_count, before.body_object_count,
        "immutable Memory body object count in {fixture}"
    );
    assert_eq!(
        after.body_object_bytes, before.body_object_bytes,
        "immutable Memory body bytes in {fixture}"
    );
    assert_eq!(
        after.routing_metadata_count, before.routing_metadata_count,
        "Memory routing metadata count in {fixture}"
    );
    assert_eq!(
        after.routing_metadata_bytes, before.routing_metadata_bytes,
        "Memory routing metadata bytes in {fixture}"
    );
    assert_eq!(
        after.external_memory_publication_count, before.external_memory_publication_count,
        "external grouped Memory publications remain semantically represented in {fixture}"
    );
}

#[test]
fn authoritative_v1_memory_envelope_conversion_preserves_all_pinned_histories() {
    let root = PathBuf::from(
        std::env::var_os("RELIQUARY_HISTORY_FIXTURES")
            .expect("RELIQUARY_HISTORY_FIXTURES must name the accepted fixture root"),
    );
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let accepted: Value = serde_json::from_slice(
        &fs::read(manifest.join(AUTHORITATIVE_BASELINE)).expect("accepted source hashes"),
    )
    .unwrap();
    let scratch = ScratchDirectory::new();

    for corpus in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ] {
        for (kind, name, file_kind) in [
            ("rel", "project.prj.rel", FileKind::Reliquary),
            ("phy", "user.phy", FileKind::Phylactery),
        ] {
            let fixture = format!("{corpus}/{name}");
            let source_path = root.join(&fixture);
            let source_bytes = fs::read(&source_path).unwrap();
            let source_digest = sha256(&source_bytes);
            assert_eq!(
                source_digest,
                accepted["authoritative_hashes"][&fixture]
                    .as_str()
                    .unwrap_or_else(|| panic!("missing accepted source hash for {fixture}")),
                "pinned authoritative V1 fixture {fixture}"
            );
            let modified = fs::metadata(&source_path).unwrap().modified().unwrap();
            let before = memory_envelope_bytes(&source_path, true);
            assert_eq!(before.memory_envelope_count, 0, "V1 input {fixture}");
            let destination = scratch.destination(&format!("{corpus}-{kind}"));
            let plan = plan_storage_upgrade(StorageUpgradeRequest {
                source: source_path.clone(),
                destination: destination.clone(),
            })
            .unwrap_or_else(|error| panic!("plan {fixture}: {error}"));
            let staged = crate::storage_upgrade::stage_memory_envelope_storage_upgrade(&plan)
                .unwrap_or_else(|error| {
                    panic!("stage Memory envelope migration {fixture}: {error}")
                });
            let after = memory_envelope_bytes(staged.test_path(), false);
            assert_converted_memory_frame_counts(&fixture, &before, &after);

            match file_kind {
                FileKind::Reliquary => {
                    compare_rel(&source_path, staged.test_path());
                }
                FileKind::Phylactery => {
                    compare_phy(&source_path, staged.test_path());
                }
            }
            assert_memory_global_frontiers(&source_path, staged.test_path(), file_kind);
            assert_source_unchanged(&source_path, &source_bytes, &source_digest, modified);
            println!(
                "MEMORY_ENVELOPE_MIGRATION_MEASUREMENT {}",
                serde_json::json!({
                    "fixture": fixture,
                    "before": before,
                    "after": after,
                    "scope": "Memory-owned standalone carriers, body objects and framing; grouped completion and external publication frames are reported as whole atomic frames and excluded from Memory-only byte deltas; whole-container size savings are not claimed"
                })
            );
            drop(staged);
            assert!(
                !destination.exists(),
                "staging does not publish the requested destination for {fixture}"
            );
        }
    }
}
