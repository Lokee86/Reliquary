use super::*;
use crate::{ContainerIdentity, FileKind};
use std::fs::OpenOptions;
use uuid::Uuid;

struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new() -> Self {
        let path = env::temp_dir().join(format!("reliquary-f03-preconversion-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_synthetic_copy(path: &Path, index: u8) {
    let identity = if index == 2 {
        None
    } else if index % 2 == 0 {
        Some(ContainerIdentity {
            file_kind: FileKind::Reliquary,
            scope: Some(ReliquaryScopeKind::Project),
        })
    } else {
        Some(ContainerIdentity {
            file_kind: FileKind::Phylactery,
            scope: None,
        })
    };
    let mut writer = match identity {
        Some(identity) => crate::Container::create_with_identity_and_uuid(
            path,
            identity,
            [index.wrapping_add(1); 16],
        )
        .unwrap(),
        None => crate::Container::create(path).unwrap(),
    };
    let frame_count = if index == 0 { BATCH_FRAME_LIMIT + 2 } else { 1 };
    for frame in 0..frame_count {
        writer
            .append(format!("PRIVATE-SYNTHETIC-FRAME-{index}-{frame}-DO-NOT-EMIT").as_bytes())
            .unwrap();
    }
    writer.sync().unwrap();
}

fn synthetic_root(scratch: &ScratchDir) -> (PathBuf, Vec<CopySpec>) {
    let root = scratch.0.join("synthetic-copies");
    fs::create_dir(&root).unwrap();
    let mut paths = Vec::new();
    for owner in 0..3_u8 {
        let directory = root.join(format!("owner-{owner}"));
        fs::create_dir(&directory).unwrap();
        let rel = directory.join("project.prj.rel");
        let phy = directory.join("user.phy");
        write_synthetic_copy(&rel, owner.wrapping_mul(2));
        write_synthetic_copy(&phy, owner.wrapping_mul(2).wrapping_add(1));
        paths.push(rel);
        paths.push(phy);
    }
    paths.sort_by(|left, right| {
        left.strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
            .as_bytes()
            .cmp(
                right
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
                    .as_bytes(),
            )
    });
    let specs = paths
        .into_iter()
        .map(|path| {
            let relative_path = path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let hashed = hash_file_snapshot(&path).unwrap();
            CopySpec {
                relative_path,
                sha256: hashed.sha256,
                byte_length: hashed.byte_length,
            }
        })
        .collect();
    (root, specs)
}

fn synthetic_binding(specs: &[CopySpec]) -> BindingProvenance {
    BindingProvenance {
        binding_kind: "synthetic_test_only".to_string(),
        source_commit: "synthetic".to_string(),
        planning_commit: "synthetic".to_string(),
        planning_record_blob_sha1: "synthetic".to_string(),
        copy_proof_sha256: "a".repeat(64),
        custody_receipt_sha256: "b".repeat(64),
        copy_table_sha256: copy_table_sha256(specs),
    }
}

#[test]
fn v01_table_digest_binds_only_copy_paths_hashes_and_lengths() {
    let specs = v01_copy_specs();
    assert_eq!(specs.len(), 6);
    assert_eq!(copy_table_sha256(&specs), V01_EXPECTED_COPY_TABLE_SHA256);
    assert_eq!(
        f03_identity_observation::expected_table_digest(&f03_identity_observation::v01_specs()),
        V01_EXPECTED_COPY_TABLE_SHA256
    );
    let debug = format!("{specs:?}");
    assert!(!debug.contains("owner_uuid"));
    assert!(!debug.contains("scope"));
    assert!(!debug.contains("target"));
}

#[test]
fn synthetic_census_is_incomplete_unclassified_and_payload_free() {
    let scratch = ScratchDir::new();
    let (root, specs) = synthetic_root(&scratch);
    let output = scratch.0.join("protected-output");
    let report = run_with_specs(
        Options {
            source_root: root.clone(),
            output: output.clone(),
        },
        specs.clone(),
        synthetic_binding(&specs),
    )
    .unwrap();
    assert_eq!(report.status, "INCOMPLETE");
    assert!(!report.authentic_admission);
    assert!(!report.semantic_history_assessed);
    assert!(!report.owner_identity_independently_authenticated);
    assert!(!report.scope_independently_authenticated);
    assert!(!report.output_access_control_authenticated);
    assert_eq!(report.semantic_family_inventory, "not_assessed");
    assert_eq!(report.semantic_family_completeness, "unknown");
    assert_eq!(report.cut_inventory, "not_started");
    assert_eq!(report.cut_completeness, "unknown");
    assert_eq!(report.observed_copy_count, "6");
    assert_eq!(
        report.physical_frame_count,
        (BATCH_FRAME_LIMIT + 7).to_string()
    );
    assert!(
        report
            .sources
            .iter()
            .all(|source| source.classification == "unclassified")
    );
    let output_text = fs::read_dir(&output)
        .unwrap()
        .map(|entry| fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!output_text.contains("PRIVATE-SYNTHETIC-FRAME"));
    assert!(!output_text.contains("owner-0"));
    assert!(!output_text.contains("target_file_sha256"));
    assert!(!output_text.contains("expected_owner_uuid"));
    assert!(output_text.contains("owner_uuid_hex_observed_from_header"));
    assert!(output_text.contains("native_scope_observed_from_header"));

    let unbound_hash = specs
        .iter()
        .find(|spec| spec.relative_path == "owner-1/project.prj.rel")
        .unwrap()
        .sha256
        .clone();
    let unbound = report
        .sources
        .iter()
        .find(|source| source.v01_copy_sha256 == unbound_hash)
        .unwrap();
    assert!(!unbound.header_observation.identity_header_present);
    assert_eq!(
        unbound.header_observation.owner_kind_observed_from_header,
        None
    );
    assert_eq!(
        unbound.header_observation.owner_uuid_observation_state,
        "no_identity_header"
    );
    assert_eq!(
        unbound.header_observation.native_scope_observation_state,
        "no_identity_header"
    );

    let no_scope_hash = specs
        .iter()
        .find(|spec| spec.relative_path == "owner-0/user.phy")
        .unwrap()
        .sha256
        .clone();
    let no_scope = report
        .sources
        .iter()
        .find(|source| source.v01_copy_sha256 == no_scope_hash)
        .unwrap();
    assert_eq!(
        no_scope.header_observation.native_scope_observation_state,
        "field_not_present_in_header"
    );
    assert_eq!(
        no_scope
            .header_observation
            .native_scope_observed_from_header,
        None
    );

    let first_batch: StoredPreconversionBatch =
        serde_json::from_slice(&fs::read(output.join("copy-0001-batch-000000.json")).unwrap())
            .unwrap();
    let second_batch: StoredPreconversionBatch =
        serde_json::from_slice(&fs::read(output.join("copy-0001-batch-000001.json")).unwrap())
            .unwrap();
    assert_eq!(first_batch.payload.batch_index, "0");
    assert_eq!(
        first_batch.payload.previous_batch_sha256,
        INITIAL_BATCH_PREDECESSOR
    );
    assert_eq!(
        first_batch.payload.frame_count,
        BATCH_FRAME_LIMIT.to_string()
    );
    assert_eq!(second_batch.payload.batch_index, "1");
    assert_eq!(
        second_batch.payload.previous_batch_sha256,
        first_batch.batch_sha256
    );
    assert_eq!(
        second_batch.payload.first_frame_ordinal,
        BATCH_FRAME_LIMIT.to_string()
    );
    assert_eq!(second_batch.payload.frame_count, "2");
}

#[test]
fn copy_binding_rejects_wrong_hash_length_and_extra_files() {
    let scratch = ScratchDir::new();
    let (root, specs) = synthetic_root(&scratch);
    let inventory = super::super::collect_source_inventory(&root).unwrap();
    let mut wrong_hash = specs.clone();
    wrong_hash[0].sha256 = "0".repeat(64);
    assert!(bind_sources(&root, &wrong_hash, &inventory).is_err());
    let mut wrong_length = specs.clone();
    wrong_length[0].byte_length += 1;
    assert!(bind_sources(&root, &wrong_length, &inventory).is_err());

    fs::write(root.join("unexpected-sidecar.bin"), b"synthetic sidecar").unwrap();
    let expanded = super::super::collect_source_inventory(&root).unwrap();
    assert!(bind_sources(&root, &specs, &expanded).is_err());
}

#[test]
fn source_tree_reenumeration_rejects_late_files_and_directories() {
    let scratch = ScratchDir::new();
    let (root, _) = synthetic_root(&scratch);
    let inventory = super::super::collect_source_inventory(&root).unwrap();
    let late = root.join("late-sidecar.bin");
    fs::write(&late, b"synthetic").unwrap();
    assert!(verify_source_inventory(&root, &inventory).is_err());
    fs::remove_file(&late).unwrap();
    fs::create_dir(root.join("late-directory")).unwrap();
    assert!(verify_source_inventory(&root, &inventory).is_err());
}

#[test]
fn outside_root_hardlink_is_rejected() {
    let scratch = ScratchDir::new();
    let (root, _) = synthetic_root(&scratch);
    let source = root.join("owner-0/project.prj.rel");
    let alias = scratch.0.join("outside-alias");
    fs::hard_link(&source, &alias).unwrap();
    assert!(super::super::collect_source_inventory(&root).is_err());
    fs::remove_file(alias).unwrap();
}

#[test]
fn source_and_output_roots_must_be_disjoint_and_output_is_one_shot() {
    let scratch = ScratchDir::new();
    let (root, specs) = synthetic_root(&scratch);
    assert!(resolve_output_path(&root.join("nested"), &root).is_err());
    assert!(resolve_output_path(&scratch.0, &root).is_err());
    let output = scratch.0.join("fresh-output");
    assert!(!output.exists());
    let report = run_with_specs(
        Options {
            source_root: root.clone(),
            output: output.clone(),
        },
        specs.clone(),
        synthetic_binding(&specs),
    )
    .unwrap();
    assert_eq!(report.status, "INCOMPLETE");
    let report_hash = sha256_bytes(&fs::read(output.join(COVERAGE_NAME)).unwrap());
    assert!(
        run_with_specs(
            Options {
                source_root: root,
                output: output.clone()
            },
            specs.clone(),
            synthetic_binding(&specs),
        )
        .is_err()
    );
    assert_eq!(
        sha256_bytes(&fs::read(output.join(COVERAGE_NAME)).unwrap()),
        report_hash
    );
    let mut expected = BTreeSet::new();
    for entry in fs::read_dir(&output).unwrap() {
        expected.insert(entry.unwrap().file_name().to_string_lossy().to_string());
    }
    fs::write(output.join("late-output.bin"), b"synthetic").unwrap();
    assert!(verify_output_membership(&output, &expected).is_err());
}

#[cfg(windows)]
#[test]
fn held_source_report_and_directory_guards_deny_mutation_and_rename() {
    let scratch = ScratchDir::new();
    let (root, specs) = synthetic_root(&scratch);
    let inventory = super::super::collect_source_inventory(&root).unwrap();
    let source_path = root.join(&specs[0].relative_path);
    let _source_guard = inventory._file_guards.first().unwrap().clone();
    assert!(OpenOptions::new().write(true).open(&source_path).is_err());
    assert!(fs::rename(&source_path, source_path.with_extension("moved")).is_err());

    let output = scratch.0.join("fresh-output");
    let report = run_with_specs(
        Options {
            source_root: root,
            output: output.clone(),
        },
        specs.clone(),
        synthetic_binding(&specs),
    )
    .unwrap();
    assert_eq!(report.status, "INCOMPLETE");
    let report_path = output.join(COVERAGE_NAME);
    let report_bytes = fs::read(&report_path).unwrap();
    let _report_guard =
        verify_immutable_output(&report_path, &report_bytes, "test report").unwrap();
    assert!(OpenOptions::new().write(true).open(&report_path).is_err());
    assert!(fs::rename(&report_path, report_path.with_extension("moved")).is_err());
    let output_guard = open_directory_guard(&output, "test output").unwrap();
    assert!(fs::rename(&output, output.with_extension("moved")).is_err());
    ensure_directory_guard(&output_guard).unwrap();
}

#[test]
fn parser_rejects_full_expected_custody_and_resume_modes() {
    for extra in ["--expected-custody", "--resume", "--verify-only"] {
        let args = vec![
            "--source-root".to_string(),
            "copies".to_string(),
            extra.to_string(),
            "extra".to_string(),
            "--output".to_string(),
            "out".to_string(),
        ];
        assert!(parse_options(args.into_iter()).is_err());
    }
}

fn refresh_synthetic_specs(root: &Path, specs: &mut [CopySpec]) {
    for spec in specs {
        let snapshot = hash_file_snapshot(&root.join(&spec.relative_path)).unwrap();
        spec.sha256 = snapshot.sha256;
        spec.byte_length = snapshot.byte_length;
    }
}

fn synthetic_frame_observation(
    source_id: &str,
    source_sha256: &str,
    ordinal: u64,
) -> super::super::FrameObservation {
    super::super::FrameObservation {
        source_id: source_id.to_string(),
        source_file_sha256: source_sha256.to_string(),
        physical_frame_ordinal: ordinal.to_string(),
        source_offset: (16 + ordinal * 9).to_string(),
        source_end_offset: (25 + ordinal * 9).to_string(),
        payload_byte_length: "1".to_string(),
        latest_global_clock: ordinal.to_string(),
        payload_sha256: "a".repeat(64),
        prefix_candidate_sha256: "b".repeat(64),
        prefix_candidate_byte_length: "1".to_string(),
        semantic_classification: "unclassified".to_string(),
    }
}

#[test]
fn production_v01_input_builder_uses_independent_table_digest() {
    let (specs, binding) = production_v01_inputs().unwrap();
    assert_eq!(copy_table_sha256(&specs), V01_EXPECTED_COPY_TABLE_SHA256);
    assert_eq!(binding.copy_table_sha256, V01_EXPECTED_COPY_TABLE_SHA256);
}

#[test]
fn batch_budget_checks_global_artifact_and_byte_boundaries() {
    assert_eq!(
        next_batch_budget(MAX_BATCH_ARTIFACTS - 1, MAX_TOTAL_BATCH_BYTES - 1, 1).unwrap(),
        (MAX_BATCH_ARTIFACTS, MAX_TOTAL_BATCH_BYTES)
    );
    assert!(next_batch_budget(MAX_BATCH_ARTIFACTS, 0, 1).is_err());
    assert!(next_batch_budget(0, MAX_TOTAL_BATCH_BYTES, 1).is_err());
}

#[test]
fn aggregate_batch_limit_rejects_next_file_before_write() {
    let scratch = ScratchDir::new();
    let (root, specs) = synthetic_root(&scratch);
    let inventory = super::super::collect_source_inventory(&root).unwrap();
    let sources = bind_sources(&root, &specs, &inventory).unwrap();
    let source = &sources[0];
    let output = scratch.0.join("batch-budget-output");
    fs::create_dir(&output).unwrap();
    let table_sha256 = copy_table_sha256(&specs);
    let mut artifacts = Vec::with_capacity(MAX_BATCH_ARTIFACTS);
    for _ in 0..(MAX_BATCH_ARTIFACTS - 1) {
        artifacts.push(BatchArtifact {
            path: source.path.clone(),
            sha256: source.expected.sha256.clone(),
            byte_length: source.expected.byte_length,
            file_identity: source.snapshot.file_identity,
            guard: Arc::clone(&source.held_file),
        });
    }
    let mut total_bytes = 0;
    let first_sha256 = persist_batch(
        source,
        &output,
        &table_sha256,
        0,
        INITIAL_BATCH_PREDECESSOR,
        vec![synthetic_frame_observation(
            &source.source_id,
            &source.expected.sha256,
            0,
        )],
        &mut artifacts,
        &mut total_bytes,
    )
    .unwrap();
    assert_eq!(artifacts.len(), MAX_BATCH_ARTIFACTS);
    assert!(output.join("copy-0001-batch-000000.json").exists());

    let second_path = output.join("copy-0001-batch-000001.json");
    assert!(
        persist_batch(
            source,
            &output,
            &table_sha256,
            1,
            &first_sha256,
            vec![synthetic_frame_observation(
                &source.source_id,
                &source.expected.sha256,
                1
            )],
            &mut artifacts,
            &mut total_bytes,
        )
        .is_err()
    );
    assert!(!second_path.exists());
}

#[cfg(windows)]
#[test]
fn source_root_and_ancestor_reparse_aliases_are_rejected() {
    use std::os::windows::fs::symlink_dir;

    let scratch = ScratchDir::new();
    let (root, specs) = synthetic_root(&scratch);

    let direct_alias = scratch.0.join("direct-root-alias");
    symlink_dir(&root, &direct_alias)
        .expect("Windows test environment must support directory symlinks");
    let direct_output = scratch.0.join("direct-alias-output");
    assert!(
        run_with_specs(
            Options {
                source_root: direct_alias,
                output: direct_output.clone(),
            },
            specs.clone(),
            synthetic_binding(&specs),
        )
        .is_err()
    );
    assert!(!direct_output.exists());

    let ancestor_alias = scratch.0.join("ancestor-alias");
    symlink_dir(&scratch.0, &ancestor_alias)
        .expect("Windows test environment must support ancestor directory symlinks");
    let aliased_root = ancestor_alias.join("synthetic-copies");
    let ancestor_output = scratch.0.join("ancestor-alias-output");
    assert!(
        run_with_specs(
            Options {
                source_root: aliased_root,
                output: ancestor_output.clone(),
            },
            specs.clone(),
            synthetic_binding(&specs),
        )
        .is_err()
    );
    assert!(!ancestor_output.exists());
}

#[test]
fn truncated_physical_tail_fails_without_final_reports() {
    let scratch = ScratchDir::new();
    let (root, mut specs) = synthetic_root(&scratch);
    let path = root.join(&specs[0].relative_path);
    let mut bytes = fs::read(&path).unwrap();
    bytes.truncate(bytes.len() - 1);
    fs::write(&path, bytes).unwrap();
    refresh_synthetic_specs(&root, &mut specs);

    let output = scratch.0.join("truncated-output");
    assert!(
        run_with_specs(
            Options {
                source_root: root,
                output: output.clone(),
            },
            specs.clone(),
            synthetic_binding(&specs),
        )
        .is_err()
    );
    assert!(!output.join(COVERAGE_NAME).exists());
    assert!(!output.join(MANIFEST_NAME).exists());
}

#[test]
fn oversized_physical_frame_fails_before_final_reports() {
    let scratch = ScratchDir::new();
    let (root, mut specs) = synthetic_root(&scratch);
    let path = root.join(&specs[0].relative_path);
    fs::remove_file(&path).unwrap();
    let mut writer = crate::Container::create(&path).unwrap();
    let payload = vec![0x5a; MAX_FRAME_PAYLOAD_BYTES as usize + 1];
    writer.append(&payload).unwrap();
    writer.sync().unwrap();
    drop(writer);
    refresh_synthetic_specs(&root, &mut specs);

    let output = scratch.0.join("oversized-frame-output");
    assert!(
        run_with_specs(
            Options {
                source_root: root,
                output: output.clone(),
            },
            specs.clone(),
            synthetic_binding(&specs),
        )
        .is_err()
    );
    assert!(!output.join(COVERAGE_NAME).exists());
    assert!(!output.join(MANIFEST_NAME).exists());
}

#[test]
fn clock_state_limit_fails_closed() {
    let scratch = ScratchDir::new();
    let (root, mut specs) = synthetic_root(&scratch);
    let path = root.join(&specs[0].relative_path);
    fs::remove_file(&path).unwrap();
    let mut writer = crate::Container::create(&path).unwrap();
    for clock in 0..=MAX_CLOCK_STATE_ENTRIES {
        writer.set_next_transaction_time_override(Some(clock as i64));
        writer
            .append_publication_envelope(b"synthetic-clock-frame")
            .unwrap();
    }
    writer.sync().unwrap();
    drop(writer);
    refresh_synthetic_specs(&root, &mut specs);

    let output = scratch.0.join("clock-limit-output");
    assert!(
        run_with_specs(
            Options {
                source_root: root,
                output: output.clone(),
            },
            specs.clone(),
            synthetic_binding(&specs),
        )
        .is_err()
    );
    assert!(!output.join(COVERAGE_NAME).exists());
    assert!(!output.join(MANIFEST_NAME).exists());
}
