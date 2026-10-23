//! Current runtime admits one physical format, before semantic replay or repair.
use reliquary_memory::{
    Container, ContainerError, Cva, CvaError, FormatVersion, Phylactery, PhylacteryError,
};
use std::{fs, path::PathBuf};
fn directory() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("current-format-boundary-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}
#[test]
fn obsolete_and_unknown_owner_headers_fail_before_replay_or_repair() {
    let root = directory();
    for phy in [false, true] {
        for (major, minor) in [(1u16, 0u16), (3, 0), (2, 1)] {
            let path = root.join(format!("{phy}-{major}-{minor}.cva"));
            if phy {
                drop(Phylactery::create(&path).unwrap());
            } else {
                drop(Cva::create(&path).unwrap());
            }
            let mut bytes = fs::read(&path).unwrap();
            bytes[8..10].copy_from_slice(&major.to_le_bytes());
            bytes[10..12].copy_from_slice(&minor.to_le_bytes());
            bytes.extend_from_slice(&[5, 0, 0]); // Even a damaged tail cannot trigger recovery.
            fs::write(&path, &bytes).unwrap();
            let expected = FormatVersion { major, minor };
            assert!(
                matches!(Container::open(&path),Err(ContainerError::UnsupportedVersion(v)) if v==expected)
            );
            assert!(
                matches!(Cva::open(&path),Err(CvaError::Container(ContainerError::UnsupportedVersion(v))) if v==expected)
            );
            assert!(
                matches!(Phylactery::open(&path),Err(PhylacteryError::Container(ContainerError::UnsupportedVersion(v))) if v==expected)
            );
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
    }
}
#[test]
fn current_rel_and_phy_reopen_with_exact_identity_and_writable_state() {
    let root = directory();
    let rel = root.join("current.rel");
    let phy = root.join("current.phy");
    let rel_id = Cva::create(&rel).unwrap().owner_uuid();
    let phy_id = Phylactery::create(&phy).unwrap().owner_uuid();
    assert_eq!(Cva::open(&rel).unwrap().owner_uuid(), rel_id);
    assert_eq!(Phylactery::open(&phy).unwrap().owner_uuid(), phy_id);
    for path in [&rel, &phy] {
        let mut container = Container::open(path).unwrap();
        assert_eq!(container.version(), FormatVersion { major: 2, minor: 0 });
        container.append(b"current opaque test backing").unwrap();
        container.sync().unwrap();
    }
}

#[test]
#[ignore = "explicit local promoted six-file corpus admission"]
fn promoted_canonical_corpus_reopens_after_runtime_retirement() {
    use sha2::{Digest, Sha256};
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let report: serde_json::Value = serde_json::from_slice(
        &fs::read(repo.join("benchmarks/canonical-history-promotion-2026-10-21.json")).unwrap(),
    )
    .unwrap();
    let conversion: serde_json::Value = serde_json::from_slice(
        &fs::read(
            repo.join("benchmarks/dependency-convergence-phase3-corpus-conversion-2026-10-23.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(conversion["format"], "CVAFRS07");
    let root = repo.join("../reliquary-fixtures/local/authoritative");
    for row in report["fixtures"].as_array().unwrap() {
        let key = row["fixture"].as_str().unwrap();
        let path = root.join(key);
        let before = fs::read(&path).unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let converted = conversion["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fixture| fixture["fixture"] == key)
            .expect("missing conversion fixture");
        assert_eq!(
            converted["original_sha256"], row["canonical_sha256"],
            "conversion before-image differs from immutable promotion for {key}"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&before)),
            converted["converted_sha256"].as_str().unwrap(),
            "promoted converted hash {key}"
        );
        let heads = if key.ends_with(".rel") {
            let owner = Cva::open(&path).unwrap();
            vec![
                owner.latest_global_version(),
                owner.archive_version(),
                owner.memory_version(),
                owner.entity_version(),
                owner.relationship_version(),
                owner.graph_version(),
                owner.memory_graph_version(),
                owner.vector_version(),
                owner.rel_turn_count(),
            ]
        } else {
            let owner = Phylactery::open(&path).unwrap();
            vec![
                owner.latest_global_version(),
                owner.memory_version(),
                owner.entity_version(),
                owner.relationship_version(),
                owner.graph_version(),
                owner.memory_graph_version(),
            ]
        };
        assert_eq!(
            serde_json::to_value(heads).unwrap(),
            row["heads"],
            "owner heads {key}"
        );
        assert_eq!(
            fs::read(&path).unwrap(),
            before,
            "open changed canonical bytes {key}"
        );
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
        println!("retired runtime admitted promoted {key}");
    }
}

#[test]
fn semantic_owners_require_current_durable_identity() {
    let path = directory().join("bare.cva");
    drop(Container::create(&path).unwrap());
    let before = fs::read(&path).unwrap();
    assert!(matches!(
        Cva::open(&path),
        Err(CvaError::Container(ContainerError::InvalidIdentity))
    ));
    assert!(matches!(
        Phylactery::open(&path),
        Err(PhylacteryError::Container(ContainerError::InvalidIdentity))
    ));
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn retired_freshness_and_import_evidence_reject_without_changing_source() {
    let root = directory();
    for magic in [
        b"CVAFRS06",
        b"CVAFRS03",
        b"CVAFRS04",
        b"CVAFRS05",
        b"CVAILEG1",
        b"CVAILEG2",
    ] {
        let path = root.join(format!("{}.rel", std::str::from_utf8(magic).unwrap()));
        drop(Cva::create(&path).unwrap());
        let mut container = Container::open(&path).unwrap();
        container.append(magic).unwrap();
        container.sync().unwrap();
        drop(container);
        let before = fs::read(&path).unwrap();
        assert!(matches!(
            Cva::open(&path),
            Err(CvaError::Freshness(_)) | Err(CvaError::Import(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}
