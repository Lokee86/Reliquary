use super::*;
use std::{fs, path::PathBuf};

struct TestRoot(PathBuf);

impl TestRoot {
    fn new(label: &str) -> Self {
        let path = env::temp_dir().join(format!(
            "reliquary-f03-identity-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&path).expect("create synthetic test root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn synthetic_sources(root: &Path) -> Vec<ExpectedCopy> {
    let paths = [
        "fixture-00/user.phy",
        "fixture-01/project.prj.rel",
        "fixture-02/user.phy",
        "fixture-03/project.prj.rel",
        "fixture-04/user.phy",
        "fixture-05/project.prj.rel",
    ];
    let identities = [
        ContainerIdentity {
            file_kind: FileKind::Reliquary,
            scope: Some(ReliquaryScopeKind::Organization),
        },
        ContainerIdentity {
            file_kind: FileKind::Phylactery,
            scope: None,
        },
        ContainerIdentity {
            file_kind: FileKind::Reliquary,
            scope: Some(ReliquaryScopeKind::Project),
        },
        ContainerIdentity {
            file_kind: FileKind::Reliquary,
            scope: Some(ReliquaryScopeKind::Connection),
        },
        ContainerIdentity {
            file_kind: FileKind::Reliquary,
            scope: None,
        },
        ContainerIdentity {
            file_kind: FileKind::Phylactery,
            scope: None,
        },
    ];
    let mut expected = Vec::new();
    for (index, (relative_path, identity)) in paths.iter().zip(identities).enumerate() {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().expect("fixture parent"))
            .expect("create fixture directory");
        let owner_uuid = [u8::try_from(index + 1).expect("small fixture index"); 16];
        drop(
            Container::create_with_identity_and_uuid(&path, identity, owner_uuid)
                .expect("create synthetic container header"),
        );
        if index == 0 {
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("open synthetic payload fixture");
            file.write_all(b"SYNTHETIC_PRIVATE_PAYLOAD_SENTINEL")
                .expect("append synthetic payload sentinel");
        }
        let bytes = fs::read(&path).expect("read synthetic fixture for test binding");
        let sha256 = Box::leak(digest(&bytes).into_boxed_str());
        expected.push(ExpectedCopy {
            relative_path,
            sha256,
            byte_length: bytes.len() as u64,
        });
    }
    expected
}

fn synthetic_binding() -> BindingProvenance {
    BindingProvenance {
        source_commit: "synthetic-test-only",
        planning_commit: "synthetic-test-only",
        planning_record_blob_sha1: "synthetic-test-only",
        v01_copy_proof_sha256: "synthetic-test-only",
        v01_custody_receipt_sha256: "synthetic-test-only",
    }
}

#[test]
fn v01_copy_table_is_bound_to_receipt_and_plan() {
    let expected = v01_specs();
    assert_eq!(expected.len(), 6);
    assert_eq!(
        expected_table_digest(&expected),
        V01_EXPECTED_COPY_TABLE_SHA256
    );
    assert_eq!(
        V01_SOURCE_COMMIT,
        "c6010fd30dd52acb81c667f7aed99b831307e49f"
    );
    assert_eq!(
        V01_PLANNING_COMMIT,
        "deb6ce6ee7dc94dd97e2fa58d3e910541240b9dd"
    );
    assert_eq!(
        V01_PLANNING_RECORD_BLOB_SHA1,
        "47a0f58da7a4011ed39ed59d29399c7492410652"
    );
    assert_eq!(
        V01_COPY_PROOF_SHA256,
        "6568bb11fe9993d59f0129bfbe3613676de24442fba263ec2387741e11a475fd"
    );
    assert_eq!(
        V01_CUSTODY_RECEIPT_SHA256,
        "bd2c6bee9966a3e62bdd1deb6a0e29e34f74cc13bad622f7898c151084153c5d"
    );
}

#[test]
fn six_copy_header_only_lifecycle_does_not_leak_paths_or_payload() {
    let root = TestRoot::new("lifecycle");
    let source_root = root.path().join("source");
    fs::create_dir(&source_root).expect("create source root");
    let expected = synthetic_sources(&source_root);
    let tree = collect_source_tree(&source_root, &expected).expect("collect exact synthetic tree");
    let table_digest = expected_table_digest(&expected);
    let report =
        observe_source_tree_with_binding(&tree, &expected, synthetic_binding(), table_digest)
            .expect("observe headers through held handles");

    assert_eq!(report.status, "IDENTITY_OBSERVATION_ONLY");
    assert!(!report.authentic_admission);
    assert!(!report.semantic_history_assessed);
    assert!(!report.owner_identity_independently_authenticated);
    assert!(!report.scope_independently_authenticated);
    assert_eq!(report.observations.len(), 6);
    // The .phy-named file deliberately contains a REL header. Classification
    // comes from the stored header and is not inferred from the filename.
    assert_eq!(
        report.observations[0]
            .header
            .owner_kind_observed_from_header
            .as_deref(),
        Some("REL")
    );
    assert_eq!(
        report.observations[0]
            .header
            .native_scope_observed_from_header
            .as_deref(),
        Some("organization")
    );
    assert_eq!(
        report.observations[0]
            .header
            .values_independently_authenticated,
        false
    );
    assert_eq!(
        report.observations[1]
            .header
            .owner_kind_observed_from_header
            .as_deref(),
        Some("PHY")
    );
    assert_eq!(
        report.observations[1].header.scope_observation_state,
        "no_scope_value_encoded"
    );
    assert!(
        report
            .observations
            .iter()
            .any(|row| row.header.owner_uuid_hex_observed_from_header.is_some())
    );

    let serialized = serialize_report(&report).expect("serialize synthetic report");
    let text = String::from_utf8(serialized.clone()).expect("report is UTF-8 JSON");
    assert!(!text.contains("SYNTHETIC_PRIVATE_PAYLOAD_SENTINEL"));
    assert!(!text.contains(source_root.to_string_lossy().as_ref()));
    assert!(!text.contains("fixture-00"));

    let guards = output_ancestor_guards(root.path()).expect("guard synthetic output parent");
    let output = root.path().join("report-output");
    let mut report_guard =
        write_fresh_report(&output, &serialized, guards).expect("write fresh synthetic report");
    let saved = fs::read(output.join(REPORT_NAME)).expect("read synthetic report");
    assert_eq!(saved, serialized);
    report_guard
        .verify()
        .expect("retain and verify report after source finalization");
    verify_source_tree(&tree, &expected).expect("source files remain unchanged");
    drop(tree);
}

#[test]
fn base_header_and_absent_scope_remain_explicitly_unknown() {
    let root = TestRoot::new("unknown-scope");
    let bare_path = root.path().join("bare.bin");
    drop(Container::create(&bare_path).expect("create bare synthetic header"));
    let bare = open_scoped_handle(&bare_path, false).expect("hold bare header");
    let bare_reader = open_header_reader(&bare_path, &bare).expect("read bare header");
    let (bare_observation, _, _, _) =
        header_observation(&bare_reader).expect("observe bare header");
    assert!(!bare_observation.identity_header_present);
    assert_eq!(
        bare_observation.owner_uuid_observation_state,
        "identity_header_absent"
    );
    assert_eq!(
        bare_observation.scope_observation_state,
        "identity_header_absent"
    );
    assert!(bare_observation.native_scope_observed_from_header.is_none());
    drop(bare_reader);

    let no_scope_path = root.path().join("no-scope.rel");
    let identity = ContainerIdentity {
        file_kind: FileKind::Reliquary,
        scope: None,
    };
    drop(
        Container::create_with_identity_and_uuid(&no_scope_path, identity, [42; 16])
            .expect("create typed header without scope"),
    );
    let held = open_scoped_handle(&no_scope_path, false).expect("hold typed header");
    let reader = open_header_reader(&no_scope_path, &held).expect("read typed header");
    let (observation, _, _, _) = header_observation(&reader).expect("observe typed header");
    assert!(observation.identity_header_present);
    assert_eq!(
        observation.scope_observation_state,
        "no_scope_value_encoded"
    );
    assert!(observation.native_scope_observed_from_header.is_none());
}

#[test]
fn unknown_scope_byte_fails_closed_with_sanitized_error() {
    let root = TestRoot::new("unknown-scope-byte");
    let path = root.path().join("synthetic.rel");
    let identity = ContainerIdentity {
        file_kind: FileKind::Reliquary,
        scope: Some(ReliquaryScopeKind::Project),
    };
    drop(
        Container::create_with_identity_and_uuid(&path, identity, [7; 16])
            .expect("create valid typed header"),
    );
    let mut bytes = fs::read(&path).expect("read synthetic header bytes");
    bytes[17] = 0xff;
    fs::write(&path, bytes).expect("write synthetic unsupported scope code");
    let held = open_scoped_handle(&path, false).expect("hold unsupported synthetic header");
    let error = open_header_reader(&path, &held)
        .err()
        .expect("unsupported scope must fail closed");
    assert_eq!(
        error,
        "historical header could not be read in read-only mode"
    );
    assert!(!root.path().join("identity-observation.json").exists());
}

#[test]
fn missing_extra_hash_mismatch_and_hardlink_alias_are_rejected() {
    let root = TestRoot::new("source-set-rejections");
    let source_root = root.path().join("source");
    fs::create_dir(&source_root).expect("create source root");
    let expected = synthetic_sources(&source_root);
    let extra_dir = source_root.join("unexpected");
    fs::create_dir(&extra_dir).expect("create extra synthetic directory");
    assert!(collect_source_tree(&source_root, &expected).is_err());
    fs::remove_dir(&extra_dir).expect("remove extra synthetic directory");

    let expected_file = source_root.join(expected[0].relative_path);
    let outside_alias = root.path().join("outside-hardlink");
    fs::hard_link(&expected_file, &outside_alias).expect("create synthetic external hardlink");
    assert!(collect_source_tree(&source_root, &expected).is_err());
    fs::remove_file(&outside_alias).expect("remove synthetic external hardlink");

    let changed = fs::read(&expected_file).expect("read expected synthetic source");
    let mut altered = changed;
    altered.push(0x55);
    fs::write(&expected_file, altered).expect("alter synthetic source bytes");
    assert!(collect_source_tree(&source_root, &expected).is_err());
    fs::remove_file(&expected_file).expect("remove changed synthetic source");
    assert!(collect_source_tree(&source_root, &expected).is_err());
}

#[test]
fn output_must_be_fresh_and_disjoint_from_sources() {
    let root = TestRoot::new("output-confinement");
    let source_root = root.path().join("source");
    fs::create_dir(&source_root).expect("create source root");
    assert!(resolve_fresh_output(&source_root.join("nested"), &source_root).is_err());
    let existing = root.path().join("already-there");
    fs::create_dir(&existing).expect("create existing output path");
    assert!(resolve_fresh_output(&existing, &source_root).is_err());
    assert!(resolve_fresh_output(&root.path().join("fresh"), &source_root).is_ok());
}

#[test]
fn late_file_or_directory_after_initial_inventory_is_rejected() {
    for add_directory in [false, true] {
        let root = TestRoot::new(if add_directory {
            "late-directory"
        } else {
            "late-file"
        });
        let source_root = root.path().join("source");
        fs::create_dir(&source_root).expect("create source root");
        let expected = synthetic_sources(&source_root);
        let tree = collect_source_tree(&source_root, &expected).expect("collect source inventory");
        let late_path = if add_directory {
            source_root.join("late-sidecar-directory")
        } else {
            source_root.join("late-sidecar.bin")
        };
        let result = verify_source_tree_with_hook(&tree, &expected, || {
            let operation = if add_directory {
                fs::create_dir(&late_path)
            } else {
                fs::write(&late_path, b"synthetic late sidecar")
            };
            operation.map_err(|_| "test late-entry injection failed".to_string())
        });
        let error = result.err().expect("late inventory change must fail");
        assert_ne!(error, "test late-entry injection failed");
        drop(tree);
    }
}

#[test]
fn report_mutation_and_rename_are_denied_or_fail_final_revalidation() {
    let root = TestRoot::new("report-finalization");
    let output = root.path().join("report-output");
    let bytes = b"{\"synthetic\":true}\n";
    let guards = output_ancestor_guards(root.path()).expect("guard report parent");
    let mut report_guard =
        write_fresh_report(&output, bytes, guards).expect("write protected synthetic report");
    let report_path = output.join(REPORT_NAME);

    #[cfg(windows)]
    {
        assert!(
            fs::OpenOptions::new()
                .write(true)
                .open(&report_path)
                .is_err()
        );
        assert!(fs::rename(&report_path, root.path().join("renamed-report.json")).is_err());
        assert!(fs::rename(&output, root.path().join("renamed-output")).is_err());
        report_guard
            .verify()
            .expect("held report and directory guards preserve final bytes and identity");
    }

    #[cfg(not(windows))]
    {
        use std::io::Write;
        let mut writer = fs::OpenOptions::new()
            .write(true)
            .open(&report_path)
            .expect("open synthetic report for adversarial mutation");
        writer.set_len(0).expect("truncate synthetic report");
        writer
            .write_all(b"{\"tampered\":true}\n")
            .expect("mutate synthetic report");
        assert!(report_guard.verify().is_err());
        fs::rename(&output, root.path().join("renamed-output"))
            .expect("rename synthetic output directory");
        assert!(report_guard.verify().is_err());
    }
}

#[cfg(windows)]
#[test]
fn held_handle_denies_write_and_rename_and_reader_uses_same_file() {
    let root = TestRoot::new("windows-held-identity");
    let source_root = root.path().join("source");
    fs::create_dir(&source_root).expect("create source root");
    let expected = synthetic_sources(&source_root);
    let path = source_root.join(expected[0].relative_path);
    let mut held = open_scoped_handle(&path, false).expect("hold deny-write source handle");
    let identity = file_identity(&held).expect("read held file identity");
    let reader = open_header_reader(&path, &held).expect("parse header from held file clone");
    assert_eq!(reader.backing_file_identity().unwrap(), Some(identity));
    let (observation, _, _, _) = header_observation(&reader).expect("observe held header");
    assert_eq!(
        observation.owner_kind_observed_from_header.as_deref(),
        Some("REL")
    );
    drop(reader);

    assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
    let moved = source_root.join("moved.rel");
    assert!(fs::rename(&path, &moved).is_err());
    assert!(path.exists());
    assert_eq!(
        hash_held_file(&path, &mut held).unwrap().sha256,
        expected[0].sha256
    );
}
