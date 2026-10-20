#![cfg(feature = "history-oracle-fixtures")]

use super::super::shell::stage_relationship_envelope_storage_upgrade;
use super::super::{StorageUpgradeRequest, plan_storage_upgrade};
use super::{ACCEPTED_HASHES, CORPORA, sha256};
use crate::container::publication_envelope;
use crate::relationship_codec;
use crate::relationship_compact_codec::publication as relationship_publication;
use crate::{Container, ContainerError, Cva, FileKind, Phylactery};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn count_relationship_links(path: &Path, v1: bool) -> u64 {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut count = 0;
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            if payload.starts_with(b"CVARELV1") {
                assert!(
                    relationship_codec::decode_version(payload)
                        .expect("valid CVARELV1 framing")
                        .is_some(),
                    "CVARELV1 marker decodes as a Relationship link"
                );
                count += 1;
            }
            Ok(())
        })
        .unwrap();
    count
}

fn count_relationship_envelopes(path: &Path) -> (u64, u64) {
    let mut container = Container::open(path).unwrap();
    let mut envelopes = 0;
    let mut links = 0;
    for object in container.chunks().unwrap() {
        let payload = container.read(object).unwrap();
        if relationship_codec::decode_version(&payload)
            .unwrap()
            .is_some()
        {
            links += 1;
        }
        if let Some(envelope) = publication_envelope::decode(&payload).unwrap() {
            if relationship_publication::decode(envelope.body, envelope.global_version)
                .unwrap()
                .is_some()
            {
                envelopes += 1;
            }
        }
    }
    (envelopes, links)
}

#[test]
fn authoritative_sources_have_zero_relationship_envelope_upgrade_applicability() {
    let fixture_root = PathBuf::from(
        std::env::var_os("RELIQUARY_HISTORY_FIXTURES")
            .expect("RELIQUARY_HISTORY_FIXTURES must name accepted fixture root"),
    );
    let manifest_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let accepted: Value =
        serde_json::from_slice(&fs::read(manifest_root.join(ACCEPTED_HASHES)).unwrap()).unwrap();
    let scratch = super::ScratchDirectory::new();
    let mut records = Vec::new();

    for corpus in CORPORA {
        for (kind, name, file_kind) in [
            ("rel", "project.prj.rel", FileKind::Reliquary),
            ("phy", "user.phy", FileKind::Phylactery),
        ] {
            let key = format!("{corpus}/{name}");
            let source = fixture_root.join(&key);
            let original = fs::read(&source).unwrap();
            let digest = sha256(&original);
            assert_eq!(
                digest,
                accepted["authoritative_hashes"][&key].as_str().unwrap(),
                "pinned authoritative hash for {key}"
            );
            let modified = fs::metadata(&source).unwrap().modified().unwrap();
            let accepted_links = count_relationship_links(&source, true);
            assert_eq!(accepted_links, 0, "zero accepted CVARELV1 links in {key}");

            match file_kind {
                FileKind::Reliquary => {
                    let owner = Cva::open_v1_read_only(&source).unwrap();
                    assert_eq!(owner.relationship_version(), 0, "REL head for {key}");
                }
                FileKind::Phylactery => {
                    let owner = Phylactery::open_v1_read_only(&source).unwrap();
                    assert_eq!(owner.relationship_version(), 0, "PHY head for {key}");
                }
            }

            let destination = scratch.destination(&format!("{corpus}-{kind}"));
            let plan = plan_storage_upgrade(StorageUpgradeRequest {
                source: source.clone(),
                destination: destination.clone(),
            })
            .unwrap_or_else(|error| panic!("plan {key}: {error}"));
            let staged = stage_relationship_envelope_storage_upgrade(&plan)
                .unwrap_or_else(|error| panic!("stage {key}: {error}"));
            let (generated_envelopes, remaining_links) =
                count_relationship_envelopes(staged.test_path());
            assert_eq!(
                generated_envelopes, 0,
                "no Relationship envelopes for {key}"
            );
            assert_eq!(remaining_links, 0, "no legacy Relationship links for {key}");
            match file_kind {
                FileKind::Reliquary => {
                    let owner = Cva::open(staged.test_path()).unwrap();
                    assert_eq!(owner.relationship_version(), 0, "staged REL head for {key}");
                }
                FileKind::Phylactery => {
                    let owner = Phylactery::open(staged.test_path()).unwrap();
                    assert_eq!(owner.relationship_version(), 0, "staged PHY head for {key}");
                }
            }

            let after = fs::read(&source).unwrap();
            let metadata_after = fs::metadata(&source).unwrap();
            assert_eq!(after, original, "source bytes unchanged for {key}");
            assert_eq!(sha256(&after), digest, "source hash unchanged for {key}");
            assert_eq!(
                metadata_after.modified().unwrap(),
                modified,
                "source mtime unchanged for {key}"
            );
            assert!(!destination.exists(), "staging did not publish {key}");
            drop(staged);
            assert!(!destination.exists(), "no final published stage for {key}");
            assert!(fs::read_dir(&scratch.0).unwrap().all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".storage-upgrade-")
            }));
            records.push(serde_json::json!({
                "fixture": key,
                "owner_kind": kind,
                "pinned_source_sha256": digest,
                "accepted_cvarelv1_links": accepted_links,
                "generated_relationship_envelopes": generated_envelopes,
                "remaining_relationship_links": remaining_links,
                "relationship_head_before_and_after": 0,
                "source_bytes_hash_mtime_unchanged": true,
                "final_published_stage": false
            }));
        }
    }

    assert_eq!(records.len(), 6);
    println!(
        "RELATIONSHIP_ENVELOPE_APPLICABILITY {}",
        serde_json::json!({"records": records, "accepted_links_total": 0, "generated_envelopes_total": 0})
    );
}
