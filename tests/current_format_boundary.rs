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
fn owner_open_reports_incomplete_tail_without_repairing_rel_or_phy() {
    use std::io::Write as _;

    let root = directory();
    let rel = root.join("incomplete.rel");
    let phy = root.join("incomplete.phy");
    drop(Cva::create(&rel).unwrap());
    drop(Phylactery::create(&phy).unwrap());

    for path in [&rel, &phy] {
        fs::OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(&[8, 0, 0])
            .unwrap();
        let before = fs::read(path).unwrap();
        if path == &rel {
            assert!(matches!(
                Cva::open(path),
                Err(CvaError::Container(ContainerError::RepairRequired { .. }))
            ));
        } else {
            assert!(matches!(
                Phylactery::open(path),
                Err(PhylacteryError::Container(
                    ContainerError::RepairRequired { .. }
                ))
            ));
        }
        assert_eq!(
            fs::read(path).unwrap(),
            before,
            "owner open repaired {path:?}"
        );
    }
}

#[test]
fn second_owner_writer_is_rejected_until_rel_and_phy_session_drop() {
    let root = directory();
    let rel_path = root.join("leased.rel");
    let phy_path = root.join("leased.phy");

    let rel = Cva::create(&rel_path).unwrap();
    let rel_id = rel.owner_uuid();
    assert!(matches!(
        Cva::open(&rel_path),
        Err(CvaError::Container(ContainerError::WriterConflict))
    ));
    drop(rel);
    assert_eq!(Cva::open(&rel_path).unwrap().owner_uuid(), rel_id);

    let phy = Phylactery::create(&phy_path).unwrap();
    let phy_id = phy.owner_uuid();
    assert!(matches!(
        Phylactery::open(&phy_path),
        Err(PhylacteryError::Container(ContainerError::WriterConflict))
    ));
    drop(phy);
    assert_eq!(Phylactery::open(&phy_path).unwrap().owner_uuid(), phy_id);
}

#[test]
#[ignore = "explicit local promoted six-file corpus admission"]
fn promoted_canonical_corpus_reopens_after_runtime_retirement() {
    use sha2::{Digest, Sha256};
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_root = std::env::var_os("RELIQUARY_CURRENT_CORPUS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("../reliquary-fixtures/local/authoritative"));
    let copy_root = directory().join("promoted-current-corpus");

    // These are the current CVAFRS07 promoted hashes and owner heads from the
    // 2026-10-23 authority markers. The 2026-10-21 manifest pins pre-CVAFRS07
    // bytes and is not an admission oracle for the current corpus.
    let fixtures: [(&str, &str, &[u64]); 6] = [
        (
            "chatgpt-first14d/project.prj.rel",
            "a02c8696db754c53a73c896133b06df9828a32923aee610c9f918a16698f76d0",
            &[18938, 7526, 3348, 150, 0, 7913, 7913, 1, 5857],
        ),
        (
            "chatgpt-first14d/user.phy",
            "7e690b4853afe0936702dc023f511dd7c78ffaae3e18e8e66f6ab1c6891a6c63",
            &[1088, 421, 0, 0, 667, 667],
        ),
        (
            "chatgpt-first28d/project.prj.rel",
            "76c15780bd7cfe7e3083dbff5684e236a8fd2090fb0a82ec3d66529d14307d87",
            &[52804, 21068, 5727, 671, 0, 25337, 25337, 1, 16465],
        ),
        (
            "chatgpt-first28d/user.phy",
            "32970f3975bfcd6de0fcdb78df9c80c78b76787832f528c1b09fd75dfa4f501d",
            &[2215, 490, 35, 0, 1690, 1637],
        ),
        (
            "rhelm-david-r-ellis-pre-relationship/project.prj.rel",
            "cbc38d055936fc5702a17c4d569557d8c35e8887d1946c5562ba419038de6ee3",
            &[11725, 3763, 1452, 199, 0, 6309, 6309, 2, 2972],
        ),
        (
            "rhelm-david-r-ellis-pre-relationship/user.phy",
            "58ac7872215e126749f024fc742828abc9a8a8f84775b39c0706d638392ce9e0",
            &[5221, 976, 86, 0, 4159, 3889],
        ),
    ];

    let mut failures = Vec::new();
    for (key, expected_hash, expected_heads) in fixtures {
        let source = source_root.join(key);
        let path = copy_root.join(key);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let source_bytes = fs::read(&source).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&source_bytes)),
            expected_hash,
            "source is not the promoted CVAFRS07 fixture {key}"
        );
        fs::copy(&source, &path).unwrap();
        let before = fs::read(&path).unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&before)),
            expected_hash,
            "copied fixture hash {key}"
        );
        let heads = if key.ends_with(".rel") {
            Cva::open(&path)
                .map(|owner| {
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
                })
                .map_err(|error| error.to_string())
        } else {
            Phylactery::open(&path)
                .map(|owner| {
                    vec![
                        owner.latest_global_version(),
                        owner.memory_version(),
                        owner.entity_version(),
                        owner.relationship_version(),
                        owner.graph_version(),
                        owner.memory_graph_version(),
                    ]
                })
                .map_err(|error| error.to_string())
        };
        match heads {
            Ok(heads) => {
                assert_eq!(heads, expected_heads, "owner heads {key}");
                println!("current runtime admitted disposable promoted copy {key}");
            }
            Err(error) => {
                eprintln!("current runtime rejected disposable promoted copy {key}: {error}");
                failures.push(format!("{key}: {error}"));
            }
        }
        assert_eq!(
            fs::read(&path).unwrap(),
            before,
            "open changed canonical bytes {key}"
        );
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    }
    assert!(
        failures.is_empty(),
        "promoted corpus admission failures: {failures:#?}"
    );
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
