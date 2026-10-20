#![cfg(feature = "history-oracle-fixtures")]

//! Step188 measurements for the accepted authoritative V1 Entity corpus.
use super::*;
use crate::container::publication_envelope;
use crate::entity_codec::decode_version;
use crate::entity_compact_codec::publication::{self, EntityPublication};
use crate::{Container, ContainerError, ObjectRef};
use std::collections::BTreeSet;

#[derive(Default, serde::Serialize)]
struct LegacyEntityFrames {
    accepted_carrier_frames: u64,
    accepted_carrier_bytes: u64,
    terminal_frames: u64,
    terminal_bytes: u64,
    link_frames: u64,
    link_bytes: u64,
    inert_orphan_frames: u64,
    inert_orphan_bytes: u64,
}

#[derive(Default, serde::Serialize)]
struct EnvelopeEntityFrames {
    envelope_frames: u64,
    envelope_bytes: u64,
    creation_frames: u64,
    delta_frames: u64,
    direct_terminal_frames: u64,
    merge_terminal_frames: u64,
}

fn payloads(path: &Path, v1: bool) -> Vec<(ObjectRef, Vec<u8>)> {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut result = Vec::new();
    container
        .visit_payloads::<ContainerError>(|object, payload| {
            result.push((object, payload.to_vec()));
            Ok(())
        })
        .unwrap();
    result
}

fn legacy_frames(path: &Path) -> (LegacyEntityFrames, BTreeSet<u64>) {
    let values = payloads(path, true);
    let mut container = Container::open_v1_read_only(path).unwrap();
    let mut result = LegacyEntityFrames::default();
    let mut accepted = BTreeSet::new();
    let mut links = Vec::new();
    for (_, payload) in &values {
        if let Some(version) = decode_version(payload).unwrap() {
            accepted.insert(version.record);
            links.push(version.global_version);
            result.link_frames += 1;
            result.link_bytes += payload.len() as u64;
        }
    }
    for (object, payload) in &values {
        if !payload.starts_with(b"CVAENTR")
            && !payload.starts_with(b"CVAENTT1")
            && !payload.starts_with(b"CVAENTT2")
        {
            continue;
        }
        if accepted.contains(object) {
            if payload.starts_with(b"CVAENTT1") || payload.starts_with(b"CVAENTT2") {
                result.terminal_frames += 1;
                result.terminal_bytes += payload.len() as u64;
            } else {
                result.accepted_carrier_frames += 1;
                result.accepted_carrier_bytes += payload.len() as u64;
            }
        } else if payload.starts_with(b"CVAENTR") {
            result.inert_orphan_frames += 1;
            result.inert_orphan_bytes += payload.len() as u64;
        }
    }
    // Force each accepted carrier reference to be valid before recording a measurement.
    for reference in accepted {
        let _ = container.read(reference).unwrap();
    }
    (result, links.into_iter().collect())
}

fn envelope_frames(path: &Path) -> (EnvelopeEntityFrames, BTreeSet<u64>, u64, u64) {
    let values = payloads(path, false);
    let mut result = EnvelopeEntityFrames::default();
    let mut globals = BTreeSet::new();
    let mut orphan_frames = 0;
    let mut orphan_bytes = 0;
    for (_, payload) in &values {
        if payload.starts_with(b"CVAENTR") {
            orphan_frames += 1;
            orphan_bytes += payload.len() as u64;
        }
        let Some(envelope) = publication_envelope::decode(payload).unwrap() else {
            continue;
        };
        let Some(publication) =
            publication::decode(envelope.body, envelope.global_version).unwrap()
        else {
            continue;
        };
        globals.insert(envelope.global_version);
        result.envelope_frames += 1;
        result.envelope_bytes += payload.len() as u64;
        match publication {
            EntityPublication::Creation(_) => result.creation_frames += 1,
            EntityPublication::Delta(_) => result.delta_frames += 1,
            EntityPublication::DirectTombstone { .. } => result.direct_terminal_frames += 1,
            EntityPublication::MergeTombstone { .. } => result.merge_terminal_frames += 1,
        }
    }
    assert!(
        values.iter().all(|(_, payload)| {
            !payload.starts_with(b"CVAENTV1")
                && !payload.starts_with(b"CVAENTT1")
                && !payload.starts_with(b"CVAENTT2")
        }),
        "legacy Entity links and terminal carriers must be replaced"
    );
    (result, globals, orphan_frames, orphan_bytes)
}

#[test]
fn authoritative_v1_entity_histories_migrate_to_envelopes_without_semantic_or_source_changes() {
    let root = PathBuf::from(
        std::env::var_os("RELIQUARY_HISTORY_FIXTURES")
            .expect("RELIQUARY_HISTORY_FIXTURES must name accepted fixture root"),
    );
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
            let key = format!("{corpus}/{name}");
            let source = root.join(&key);
            let original = fs::read(&source).unwrap();
            let original_hash = sha256(&original);
            let original_modified = fs::metadata(&source).unwrap().modified().unwrap();
            let (before, expected_globals) = legacy_frames(&source);
            let destination = scratch.destination(&format!("{corpus}-{kind}-envelope"));
            let plan = plan_storage_upgrade(StorageUpgradeRequest {
                source: source.clone(),
                destination: destination.clone(),
            })
            .unwrap_or_else(|error| panic!("plan {key}: {error}"));
            let staged = super::super::stage_entity_envelope_storage_upgrade(&plan)
                .unwrap_or_else(|error| panic!("stage Entity envelopes {key}: {error}"));
            match file_kind {
                FileKind::Reliquary => compare_rel(&source, staged.test_path()),
                FileKind::Phylactery => compare_phy(&source, staged.test_path()),
            }
            let (after, actual_globals, inert_orphan_frames, inert_orphan_bytes) =
                envelope_frames(staged.test_path());
            assert_eq!(
                actual_globals, expected_globals,
                "accepted Entity global tickets for {key}"
            );
            assert_eq!(
                after.envelope_frames,
                before.accepted_carrier_frames + before.terminal_frames,
                "every accepted Entity link becomes one envelope for {key}"
            );
            assert_eq!(
                before.link_frames,
                before.accepted_carrier_frames + before.terminal_frames,
                "each accepted carrier has one legacy Entity link for {key}"
            );
            assert_eq!(
                inert_orphan_frames, before.inert_orphan_frames,
                "inert Entity carriers remain separately retained for {key}"
            );
            assert_eq!(
                inert_orphan_bytes, before.inert_orphan_bytes,
                "inert Entity carrier bytes remain unchanged for {key}"
            );
            assert_source_unchanged(&source, &original, &original_hash, original_modified);
            println!(
                "ENTITY_ENVELOPE_MIGRATION_MEASUREMENT {}",
                serde_json::json!({
                    "fixture": key,
                    "semantic_mismatches": 0,
                    "before": before,
                    "after": after,
                    "inert_orphan_frames_after": inert_orphan_frames,
                    "inert_orphan_bytes_after": inert_orphan_bytes,
                    "source_sha256": original_hash,
                    "source_unchanged": true,
                    "final_destination_published": false,
                    "scope": "Entity-owned frames only; Memory, Relationship, Ego, Graph and grouped whole-file bytes excluded"
                })
            );
            drop(staged);
            assert!(
                !destination.exists(),
                "staging must not publish destination for {key}"
            );
        }
    }
}
