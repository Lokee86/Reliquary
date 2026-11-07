//! One-shot pre-conversion physical census over the V01 hash/length copy set.
//!
//! This mode has no owner UUID, kind, scope, target hash, or PG-C01 custody
//! input. Header values are recorded only as observations and remain untrusted.

#[path = "f03_preconversion_physical_census/historical_state.rs"]
pub mod historical_state;

use super::{
    BASE_READER_COMMIT, BATCH_FRAME_LIMIT, HASH_BUFFER_BYTES, INITIAL_BATCH_PREDECESSOR,
    MAX_CLOCK_STATE_ENTRIES, MAX_FRAME_PAYLOAD_BYTES, MAX_OUTPUT_ARTIFACT_BYTES,
    MAX_SOURCE_FILE_BYTES, SourceInventoryRecord, SourceTreeInventory, ToolResult,
    ensure_directory_guard, ensure_directory_guards, ensure_no_reparse_components, file_identity,
    frame_observation, hash_container_handle, hash_file_snapshot, inspect_existing_output,
    lower_hex, open_directory_guard, open_output_ancestor_guards, physical_link_count,
    reader_source_sha256, resolve_output_path, save_immutable, save_immutable_json, sha256_bytes,
    snapshots_match, verify_immutable_output, verify_source_inventory,
};
use crate::ReliquaryScopeKind;
use crate::container::Container;
use crate::f03_identity_observation::{
    self, ExpectedCopy, V01_COPY_PROOF_SHA256, V01_CUSTODY_RECEIPT_SHA256,
    V01_EXPECTED_COPY_TABLE_SHA256, V01_PLANNING_COMMIT, V01_PLANNING_RECORD_BLOB_SHA1,
    V01_SOURCE_COMMIT,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, File},
    path::{Path, PathBuf},
    sync::Arc,
};

const COVERAGE_NAME: &str = "preconversion-physical-coverage.json";
const MANIFEST_NAME: &str = "preconversion-physical-manifest.json";
const BATCH_SCHEMA_ID: &str = "reliquary-f03-preconversion-physical-batch-v1";
const COVERAGE_SCHEMA_ID: &str = "reliquary-f03-preconversion-physical-coverage-v1";
const MANIFEST_SCHEMA_ID: &str = "reliquary-f03-preconversion-physical-manifest-v1";
const MAX_BATCH_ARTIFACTS: usize = 4096;
const MAX_TOTAL_BATCH_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
struct CopySpec {
    relative_path: String,
    sha256: String,
    byte_length: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
struct BindingProvenance {
    binding_kind: String,
    source_commit: String,
    planning_commit: String,
    planning_record_blob_sha1: String,
    copy_proof_sha256: String,
    custody_receipt_sha256: String,
    copy_table_sha256: String,
}

#[derive(Debug)]
struct Options {
    source_root: PathBuf,
    output: PathBuf,
}

#[derive(Clone, Debug)]
struct VerifiedCopy {
    source_id: String,
    expected: CopySpec,
    path: PathBuf,
    held_file: Arc<File>,
    snapshot: super::HashedFile,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
struct FileIdentityObservation {
    volume_or_device_hex: String,
    file_index_or_inode_hex: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
struct HeaderObservation {
    identity_header_present: bool,
    owner_kind_observed_from_header: Option<String>,
    owner_uuid_hex_observed_from_header: Option<String>,
    owner_uuid_observation_state: String,
    native_scope_observed_from_header: Option<String>,
    native_scope_observation_state: String,
    values_independently_authenticated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
struct PreconversionSourceSummary {
    copy_id: String,
    v01_copy_sha256: String,
    v01_copy_byte_length: String,
    observed_file_sha256: String,
    observed_file_byte_length: String,
    observed_file_modified_unix_ns: String,
    physical_file_identity: FileIdentityObservation,
    physical_link_count: String,
    format_major: String,
    format_minor: String,
    header_length: String,
    header_observation: HeaderObservation,
    physical_frame_count: String,
    physical_end_offset: String,
    physical_latest_global_clock: String,
    batch_count: String,
    batch_stream_sha256: String,
    batch_artifact_set_sha256: String,
    classification: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
struct PreconversionCoverage {
    schema_id: String,
    status: String,
    evidence_scope: String,
    authentic_admission: bool,
    semantic_history_assessed: bool,
    owner_identity_independently_authenticated: bool,
    scope_independently_authenticated: bool,
    output_access_control_authenticated: bool,
    semantic_family_inventory: String,
    semantic_family_completeness: String,
    cut_inventory: String,
    cut_completeness: String,
    sidecar_discovery_scope: String,
    binding_provenance: BindingProvenance,
    source_root_inventory_sha256: String,
    source_set_sha256: String,
    expected_copy_table_sha256: String,
    expected_copy_count: String,
    observed_copy_count: String,
    physical_frame_scan: String,
    physical_frame_completeness: String,
    physical_frame_count: String,
    unclassified_frame_count: String,
    sources: Vec<PreconversionSourceSummary>,
    reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
struct PreconversionManifest {
    schema_id: String,
    status: String,
    authentic_admission: bool,
    semantic_history_assessed: bool,
    binding_provenance: BindingProvenance,
    reader_commit: String,
    reader_source_sha256: String,
    coverage_sha256: String,
    source_root_inventory_sha256: String,
    source_set_sha256: String,
    expected_copy_count: String,
    observed_copy_count: String,
    sources: Vec<PreconversionSourceSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
struct PreconversionBatchPayload {
    schema_id: String,
    reader_commit: String,
    reader_source_sha256: String,
    copy_table_sha256: String,
    previous_batch_sha256: String,
    copy_id: String,
    copy_file_sha256: String,
    batch_index: String,
    first_frame_ordinal: String,
    first_source_offset: String,
    end_source_offset: String,
    frame_count: String,
    frames: Vec<super::FrameObservation>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
struct StoredPreconversionBatch {
    payload: PreconversionBatchPayload,
    batch_sha256: String,
}

struct BatchArtifact {
    path: PathBuf,
    sha256: String,
    byte_length: u64,
    file_identity: (u64, u64),
    guard: Arc<File>,
}

impl CopySpec {
    fn from_expected(value: ExpectedCopy) -> Self {
        Self {
            relative_path: value.relative_path.to_string(),
            sha256: value.sha256.to_string(),
            byte_length: value.byte_length,
        }
    }
}

fn v01_copy_specs() -> Vec<CopySpec> {
    let mut specs: Vec<_> = f03_identity_observation::V01_EXPECTED_COPIES
        .iter()
        .copied()
        .map(CopySpec::from_expected)
        .collect();
    specs.sort_by(|left, right| {
        left.relative_path
            .as_bytes()
            .cmp(right.relative_path.as_bytes())
    });
    specs
}

fn copy_table_sha256(specs: &[CopySpec]) -> String {
    let mut ordered = specs.to_vec();
    ordered.sort_by(|left, right| {
        left.relative_path
            .as_bytes()
            .cmp(right.relative_path.as_bytes())
    });
    let mut hasher = Sha256::new();
    hasher.update(b"reliquary:f03:identity-observation-v01-copy-table:v1\0");
    hasher.update((ordered.len() as u64).to_le_bytes());
    for spec in &ordered {
        for value in [spec.relative_path.as_bytes(), spec.sha256.as_bytes()] {
            hasher.update((value.len() as u64).to_le_bytes());
            hasher.update(value);
        }
        hasher.update(spec.byte_length.to_le_bytes());
    }
    lower_hex(&hasher.finalize())
}

fn validate_specs(specs: &[CopySpec]) -> ToolResult<BTreeMap<(String, String), Option<u64>>> {
    if specs.len() != 6 {
        return Err("pre-conversion census requires exactly six hash-pinned copies".to_string());
    }
    let mut expected = BTreeMap::new();
    let mut hashes = BTreeSet::new();
    for spec in specs {
        let path = Path::new(&spec.relative_path);
        if path.is_absolute()
            || !spec.relative_path.contains('/')
            || spec.relative_path.contains('\\')
            || path
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
            || spec.sha256.len() != 64
            || !spec
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || spec.byte_length == 0
            || spec.byte_length > MAX_SOURCE_FILE_BYTES
            || !hashes.insert(spec.sha256.clone())
        {
            return Err("pre-conversion copy table is invalid".to_string());
        }
        let components: Vec<_> = spec.relative_path.split('/').collect();
        if components
            .iter()
            .any(|part| part.is_empty() || *part == "." || *part == "..")
        {
            return Err("pre-conversion copy path is invalid".to_string());
        }
        let mut directory = String::new();
        for part in &components[..components.len() - 1] {
            if !directory.is_empty() {
                directory.push('/');
            }
            directory.push_str(part);
            expected.insert((directory.clone(), "directory".to_string()), None);
        }
        let key = (spec.relative_path.clone(), "file".to_string());
        if expected.insert(key, Some(spec.byte_length)).is_some() {
            return Err("pre-conversion copy table contains duplicate paths".to_string());
        }
    }
    expected.insert((String::new(), "directory".to_string()), None);
    Ok(expected)
}

fn verify_exact_layout(specs: &[CopySpec], inventory: &SourceTreeInventory) -> ToolResult<()> {
    let expected = validate_specs(specs)?;
    if inventory.records.len() != expected.len() {
        return Err("source root does not match the exact V01 copy layout".to_string());
    }
    let mut observed = BTreeMap::new();
    for record in &inventory.records {
        let key = (record.relative_path.clone(), record.entry_type.clone());
        let length = if record.entry_type == "file" {
            Some(record.byte_length)
        } else if record.entry_type == "directory" && record.byte_length == 0 {
            None
        } else {
            return Err("source tree contains an unsupported entry type".to_string());
        };
        if observed.insert(key, length).is_some() {
            return Err("source tree contains duplicate inventory entries".to_string());
        }
    }
    if observed != expected {
        return Err("source root membership differs from the V01 copy layout".to_string());
    }
    Ok(())
}

fn bind_sources(
    source_root: &Path,
    specs: &[CopySpec],
    inventory: &SourceTreeInventory,
) -> ToolResult<Vec<VerifiedCopy>> {
    verify_exact_layout(specs, inventory)?;
    let file_records: Vec<&SourceInventoryRecord> = inventory
        .records
        .iter()
        .filter(|record| record.entry_type == "file")
        .collect();
    if file_records.len() != specs.len() || inventory._file_guards.len() != specs.len() {
        return Err("source file handle count differs from V01 custody".to_string());
    }
    let mut sources = Vec::with_capacity(specs.len());
    for (index, ((spec, record), held_file)) in specs
        .iter()
        .zip(file_records.iter())
        .zip(inventory._file_guards.iter())
        .enumerate()
    {
        if record.relative_path != spec.relative_path || record.byte_length != spec.byte_length {
            return Err("source path or length differs from V01 hash/length custody".to_string());
        }
        let path = source_root.join(Path::new(&spec.relative_path));
        ensure_no_reparse_components(&path, "source copy")?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| "source copy metadata is unavailable".to_string())?;
        if !metadata.is_file() || super::is_link_or_reparse(&metadata) {
            return Err("source copy is not a plain file".to_string());
        }
        let identity = file_identity(held_file)?;
        if identity != record.file_identity || physical_link_count(held_file)? != 1 {
            return Err("source copy identity or physical link count is invalid".to_string());
        }
        let hashed = hash_file_snapshot(&path)?;
        if hashed.sha256 != spec.sha256
            || hashed.byte_length != spec.byte_length
            || hashed.file_identity != identity
            || hashed.modified_unix_ns != record.modified_unix_ns
        {
            return Err("source copy does not match its V01 hash/length custody".to_string());
        }
        sources.push(VerifiedCopy {
            source_id: format!("copy-{:04}", index + 1),
            expected: spec.clone(),
            path,
            held_file: Arc::clone(held_file),
            snapshot: hashed,
        });
    }
    Ok(sources)
}

fn verify_copy_stable(copy: &VerifiedCopy) -> ToolResult<()> {
    if file_identity(&copy.held_file)? != copy.snapshot.file_identity
        || physical_link_count(&copy.held_file)? != 1
    {
        return Err("held source copy identity or link count changed".to_string());
    }
    ensure_no_reparse_components(&copy.path, "source copy")?;
    let current = hash_file_snapshot(&copy.path)?;
    if !snapshots_match(&current, &copy.snapshot)
        || current.sha256 != copy.expected.sha256
        || current.byte_length != copy.expected.byte_length
    {
        return Err("source copy changed during the pre-conversion census".to_string());
    }
    Ok(())
}

fn source_set_sha256(specs_sha256: &str, sources: &[VerifiedCopy]) -> String {
    let mut ordered: Vec<_> = sources.iter().collect();
    ordered.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    let mut hasher = Sha256::new();
    hasher.update(b"reliquary-f03-preconversion-source-set-v1\0");
    hasher.update(specs_sha256.as_bytes());
    hasher.update((ordered.len() as u64).to_le_bytes());
    for source in ordered {
        for value in [
            source.source_id.as_bytes(),
            source.expected.sha256.as_bytes(),
            source.snapshot.sha256.as_bytes(),
        ] {
            hasher.update((value.len() as u64).to_le_bytes());
            hasher.update(value);
        }
        hasher.update(source.expected.byte_length.to_le_bytes());
        hasher.update(source.snapshot.byte_length.to_le_bytes());
    }
    lower_hex(&hasher.finalize())
}

fn scope_name_observed(scope: ReliquaryScopeKind) -> &'static str {
    match scope {
        ReliquaryScopeKind::Organization => "organization",
        ReliquaryScopeKind::Project => "project",
        ReliquaryScopeKind::Connection => "connection",
    }
}

fn header_observation(container: &Container) -> HeaderObservation {
    let identity = container.identity();
    let owner_kind = identity.map(|value| super::kind_label(value.file_kind).to_string());
    let owner_uuid = container.owner_uuid().map(|value| super::hex_uuid(&value));
    let owner_uuid_state = match (identity, owner_uuid.is_some()) {
        (None, _) => "no_identity_header",
        (Some(_), true) => "observed_from_header",
        (Some(_), false) => "field_not_present_in_header",
    };
    let native_scope = identity
        .and_then(|value| value.scope)
        .map(scope_name_observed)
        .map(str::to_string);
    let scope_state = match identity {
        None => "no_identity_header",
        Some(value) if value.scope.is_some() => "observed_from_header",
        Some(_) => "field_not_present_in_header",
    };
    HeaderObservation {
        identity_header_present: identity.is_some(),
        owner_kind_observed_from_header: owner_kind,
        owner_uuid_hex_observed_from_header: owner_uuid,
        owner_uuid_observation_state: owner_uuid_state.to_string(),
        native_scope_observed_from_header: native_scope,
        native_scope_observation_state: scope_state.to_string(),
        values_independently_authenticated: false,
    }
}

fn physical_identity_observation(identity: (u64, u64)) -> FileIdentityObservation {
    FileIdentityObservation {
        volume_or_device_hex: format!("{:016x}", identity.0),
        file_index_or_inode_hex: format!("{:016x}", identity.1),
    }
}
#[derive(Debug)]
struct ScannedCopy {
    summary: PreconversionSourceSummary,
    frame_count: u64,
}

fn copy_table_binding(specs: &[CopySpec]) -> BindingProvenance {
    BindingProvenance {
        binding_kind: "v01_hash_length_only".to_string(),
        source_commit: V01_SOURCE_COMMIT.to_string(),
        planning_commit: V01_PLANNING_COMMIT.to_string(),
        planning_record_blob_sha1: V01_PLANNING_RECORD_BLOB_SHA1.to_string(),
        copy_proof_sha256: V01_COPY_PROOF_SHA256.to_string(),
        custody_receipt_sha256: V01_CUSTODY_RECEIPT_SHA256.to_string(),
        copy_table_sha256: copy_table_sha256(specs),
    }
}

fn encode_preconversion_batch(payload: PreconversionBatchPayload) -> ToolResult<(Vec<u8>, String)> {
    if payload.schema_id != BATCH_SCHEMA_ID
        || payload.reader_commit != BASE_READER_COMMIT
        || payload.reader_source_sha256 != reader_source_sha256()
        || !valid_digest(&payload.copy_table_sha256)
        || !valid_digest(&payload.previous_batch_sha256)
        || payload.frames.is_empty()
        || payload.frames.len() > BATCH_FRAME_LIMIT
        || payload.frame_count != payload.frames.len().to_string()
    {
        return Err("physical batch binding or bounds are invalid".to_string());
    }
    let canonical = serde_json::to_vec(&payload)
        .map_err(|_| "physical batch serialization failed".to_string())?;
    let batch_sha256 = sha256_bytes(&canonical);
    let stored = StoredPreconversionBatch {
        payload,
        batch_sha256: batch_sha256.clone(),
    };
    let bytes = super::serialize_with_newline(&stored)?;
    if bytes.len() as u64 > MAX_OUTPUT_ARTIFACT_BYTES {
        return Err("physical batch exceeds the bounded artifact size".to_string());
    }
    Ok((bytes, batch_sha256))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn batch_path(output: &Path, copy_id: &str, index: u64) -> PathBuf {
    output.join(format!("{copy_id}-batch-{index:06}.json"))
}

fn verify_output_membership(output: &Path, expected: &BTreeSet<String>) -> ToolResult<()> {
    ensure_no_reparse_components(output, "output directory")?;
    let mut observed = BTreeSet::new();
    let entries =
        fs::read_dir(output).map_err(|_| "output directory could not be enumerated".to_string())?;
    for entry in entries {
        if observed.len() >= expected.len() {
            return Err("output artifact count exceeds the bounded inventory".to_string());
        }
        let entry = entry.map_err(|_| "output entry is unavailable".to_string())?;
        let path = entry.path();
        ensure_no_reparse_components(&path, "output entry")?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| "output entry metadata is unavailable".to_string())?;
        if !metadata.is_file() || super::is_link_or_reparse(&metadata) {
            return Err("output contains a non-file or reparse entry".to_string());
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !observed.insert(name) {
            return Err("output contains duplicate names".to_string());
        }
    }
    if &observed != expected {
        return Err("output membership differs from the exact generated artifact set".to_string());
    }
    Ok(())
}

fn verify_batch_artifacts(artifacts: &[BatchArtifact]) -> ToolResult<()> {
    if artifacts.len() > MAX_BATCH_ARTIFACTS {
        return Err("physical batch artifact count exceeds its limit".to_string());
    }
    for artifact in artifacts {
        ensure_no_reparse_components(&artifact.path, "physical batch")?;
        if file_identity(&artifact.guard)? != artifact.file_identity
            || physical_link_count(&artifact.guard)? != 1
        {
            return Err("held physical batch identity or link count changed".to_string());
        }
        let (bytes, snapshot, guard) = super::read_bounded_snapshot(
            &artifact.path,
            MAX_OUTPUT_ARTIFACT_BYTES,
            "physical batch",
        )?;
        if snapshot.file_identity != artifact.file_identity
            || snapshot.byte_length != artifact.byte_length
            || snapshot.sha256 != artifact.sha256
            || sha256_bytes(&bytes) != artifact.sha256
            || physical_link_count(&guard)? != 1
        {
            return Err("physical batch changed after it was written".to_string());
        }
    }
    Ok(())
}

fn verify_report_bytes(path: &Path, expected: &[u8], label: &str) -> ToolResult<Arc<File>> {
    if expected.len() as u64 > MAX_OUTPUT_ARTIFACT_BYTES {
        return Err(format!("{label} exceeds the bounded report size"));
    }
    ensure_no_reparse_components(path, label)?;
    let (bytes, snapshot, guard) =
        super::read_bounded_snapshot(path, MAX_OUTPUT_ARTIFACT_BYTES, label)?;
    if bytes != expected
        || snapshot.sha256 != sha256_bytes(expected)
        || physical_link_count(&guard)? != 1
    {
        return Err(format!("{label} changed after finalization"));
    }
    Ok(guard)
}

fn next_batch_budget(
    existing_count: usize,
    existing_bytes: u64,
    next_artifact_bytes: u64,
) -> ToolResult<(usize, u64)> {
    if existing_count >= MAX_BATCH_ARTIFACTS {
        return Err("physical batch artifact count exceeds its limit".to_string());
    }
    let next_count = existing_count
        .checked_add(1)
        .ok_or_else(|| "physical batch artifact count overflow".to_string())?;
    let next_bytes = existing_bytes
        .checked_add(next_artifact_bytes)
        .ok_or_else(|| "physical batch byte count overflow".to_string())?;
    if next_count > MAX_BATCH_ARTIFACTS || next_bytes > MAX_TOTAL_BATCH_BYTES {
        return Err("physical output exceeds its bounded artifact budget".to_string());
    }
    Ok((next_count, next_bytes))
}

fn persist_batch(
    source: &VerifiedCopy,
    output: &Path,
    copy_table_sha256: &str,
    batch_index: u64,
    previous_batch_sha256: &str,
    frames: Vec<super::FrameObservation>,
    artifacts: &mut Vec<BatchArtifact>,
    total_bytes: &mut u64,
) -> ToolResult<String> {
    if frames.is_empty() {
        return Err("physical batch inventory is empty".to_string());
    }
    let first = frames
        .first()
        .ok_or_else(|| "physical batch is empty".to_string())?;
    let last = frames
        .last()
        .ok_or_else(|| "physical batch is empty".to_string())?;
    let payload = PreconversionBatchPayload {
        schema_id: BATCH_SCHEMA_ID.to_string(),
        reader_commit: BASE_READER_COMMIT.to_string(),
        reader_source_sha256: reader_source_sha256(),
        copy_table_sha256: copy_table_sha256.to_string(),
        previous_batch_sha256: previous_batch_sha256.to_string(),
        copy_id: source.source_id.clone(),
        copy_file_sha256: source.expected.sha256.clone(),
        batch_index: batch_index.to_string(),
        first_frame_ordinal: first.physical_frame_ordinal.clone(),
        first_source_offset: first.source_offset.clone(),
        end_source_offset: last.source_end_offset.clone(),
        frame_count: frames.len().to_string(),
        frames,
    };
    let (bytes, batch_sha256) = encode_preconversion_batch(payload)?;
    let (next_count, next_total) =
        next_batch_budget(artifacts.len(), *total_bytes, bytes.len() as u64)?;
    let path = batch_path(output, &source.source_id, batch_index);
    save_immutable(&path, &bytes, false)?;
    let guard = verify_immutable_output(&path, &bytes, "physical batch")?;
    let identity = file_identity(&guard)?;
    if physical_link_count(&guard)? != 1 {
        return Err("new physical batch has an unexpected hardlink count".to_string());
    }
    let artifact_sha256 = sha256_bytes(&bytes);
    let (observed, snapshot, path_guard) =
        super::read_bounded_snapshot(&path, MAX_OUTPUT_ARTIFACT_BYTES, "physical batch")?;
    if observed != bytes
        || snapshot.sha256 != artifact_sha256
        || snapshot.byte_length != bytes.len() as u64
        || snapshot.file_identity != identity
        || file_identity(&path_guard)? != identity
    {
        return Err("new physical batch failed exact read-back verification".to_string());
    }
    artifacts.push(BatchArtifact {
        path,
        sha256: artifact_sha256,
        byte_length: bytes.len() as u64,
        file_identity: identity,
        guard,
    });
    if artifacts.len() != next_count {
        return Err("physical batch count changed during artifact creation".to_string());
    }
    *total_bytes = next_total;
    Ok(batch_sha256)
}

fn scan_copy(
    source: &VerifiedCopy,
    output: &Path,
    copy_table_sha256: &str,
    artifacts: &mut Vec<BatchArtifact>,
    total_batch_bytes: &mut u64,
) -> ToolResult<ScannedCopy> {
    verify_copy_stable(source)?;
    let input = source
        .held_file
        .try_clone()
        .map_err(|_| "held source handle could not be cloned for header parsing".to_string())?;
    let container = Container::open_read_only_for_scan_from_file(&source.path, input)
        .map_err(|_| "historical reader rejected a source copy header".to_string())?;
    if container.backing_file_identity().ok().flatten() != Some(source.snapshot.file_identity) {
        return Err("historical reader did not retain the V01-bound file identity".to_string());
    }
    let physical_length = container
        .physical_length()
        .map_err(|_| "source physical length is unavailable".to_string())?;
    if physical_length != source.snapshot.byte_length || container.header_len > physical_length {
        return Err("source physical bounds differ from the V01-bound file".to_string());
    }
    container
        .validate_path_identity()
        .map_err(|_| "source path identity changed before scan".to_string())?;
    let version = container.version();
    let header = header_observation(&container);
    let mut cursor = container
        .physical_cursor_for_source_witness(MAX_FRAME_PAYLOAD_BYTES, MAX_CLOCK_STATE_ENTRIES)
        .map_err(|_| "bounded physical cursor could not be opened".to_string())?;
    let mut frame_count = 0_u64;
    let mut latest_global = 0_u64;
    let mut last_end = container.header_len;
    let mut batch_index = 0_u64;
    let mut previous_batch_sha256 = INITIAL_BATCH_PREDECESSOR.to_string();
    let mut pending_frames = Vec::with_capacity(BATCH_FRAME_LIMIT);
    let mut batch_stream_hasher = Sha256::new();
    let mut frame_stream_hasher = Sha256::new();
    let copy_artifact_start = artifacts.len();

    loop {
        let frame = cursor.next_frame().map_err(|_| {
            "physical frame stream is malformed, truncated, or unavailable".to_string()
        })?;
        let Some(frame) = frame else {
            break;
        };
        let observation = frame_observation(
            &source.source_id,
            &source.expected.sha256,
            frame_count,
            &frame,
        );
        frame_stream_hasher.update(observation.payload_sha256.as_bytes());
        frame_stream_hasher.update([0]);
        frame_stream_hasher.update(observation.prefix_candidate_sha256.as_bytes());
        frame_stream_hasher.update([0]);
        frame_stream_hasher.update(observation.source_offset.as_bytes());
        frame_stream_hasher.update([0]);
        frame_stream_hasher.update(observation.source_end_offset.as_bytes());
        frame_stream_hasher.update([0]);
        frame_stream_hasher.update(observation.latest_global_clock.as_bytes());
        frame_stream_hasher.update([b'\n']);
        latest_global = frame.latest_global;
        last_end = frame.end;
        frame_count = frame_count
            .checked_add(1)
            .ok_or_else(|| "physical frame count overflow".to_string())?;
        pending_frames.push(observation);
        if pending_frames.len() == BATCH_FRAME_LIMIT {
            let batch_sha256 = persist_batch(
                source,
                output,
                copy_table_sha256,
                batch_index,
                &previous_batch_sha256,
                std::mem::take(&mut pending_frames),
                artifacts,
                total_batch_bytes,
            )?;
            batch_stream_hasher.update(batch_sha256.as_bytes());
            batch_stream_hasher.update([b'\n']);
            previous_batch_sha256 = batch_sha256;
            batch_index = batch_index
                .checked_add(1)
                .ok_or_else(|| "physical batch index overflow".to_string())?;
        }
    }
    if !pending_frames.is_empty() {
        let batch_sha256 = persist_batch(
            source,
            output,
            copy_table_sha256,
            batch_index,
            &previous_batch_sha256,
            std::mem::take(&mut pending_frames),
            artifacts,
            total_batch_bytes,
        )?;
        batch_stream_hasher.update(batch_sha256.as_bytes());
        batch_stream_hasher.update([b'\n']);
        batch_index = batch_index
            .checked_add(1)
            .ok_or_else(|| "physical batch index overflow".to_string())?;
    }
    drop(cursor);
    if last_end != physical_length {
        return Err("physical scan did not end at the verified file boundary".to_string());
    }
    container
        .validate_path_identity()
        .map_err(|_| "source path identity changed after scan".to_string())?;
    let handle_hash = hash_container_handle(&container, source.snapshot.file_identity)?;
    if !snapshots_match(&handle_hash, &source.snapshot)
        || handle_hash.sha256 != source.expected.sha256
        || handle_hash.byte_length != source.expected.byte_length
    {
        return Err("historical reader bytes changed during physical scan".to_string());
    }
    verify_copy_stable(source)?;

    let mut artifact_set_hasher = Sha256::new();
    for artifact in artifacts.iter().skip(copy_artifact_start) {
        artifact_set_hasher.update(artifact.sha256.as_bytes());
        artifact_set_hasher.update([b'\n']);
    }
    let source_summary = PreconversionSourceSummary {
        copy_id: source.source_id.clone(),
        v01_copy_sha256: source.expected.sha256.clone(),
        v01_copy_byte_length: source.expected.byte_length.to_string(),
        observed_file_sha256: handle_hash.sha256,
        observed_file_byte_length: handle_hash.byte_length.to_string(),
        observed_file_modified_unix_ns: handle_hash.modified_unix_ns,
        physical_file_identity: physical_identity_observation(source.snapshot.file_identity),
        physical_link_count: "1".to_string(),
        format_major: version.major.to_string(),
        format_minor: version.minor.to_string(),
        header_length: container.header_len.to_string(),
        header_observation: header,
        physical_frame_count: frame_count.to_string(),
        physical_end_offset: physical_length.to_string(),
        physical_latest_global_clock: latest_global.to_string(),
        batch_count: batch_index.to_string(),
        batch_stream_sha256: lower_hex(&batch_stream_hasher.finalize()),
        batch_artifact_set_sha256: lower_hex(&artifact_set_hasher.finalize()),
        classification: "unclassified".to_string(),
    };

    Ok(ScannedCopy {
        summary: source_summary,
        frame_count,
    })
}

fn run_with_specs(
    options: Options,
    specs: Vec<CopySpec>,
    binding: BindingProvenance,
) -> ToolResult<PreconversionCoverage> {
    let specs_sha256 = copy_table_sha256(&specs);
    if specs_sha256 != binding.copy_table_sha256 {
        return Err("copy table does not match its bound provenance digest".to_string());
    }
    validate_specs(&specs)?;
    ensure_no_reparse_components(&options.source_root, "source root")?;
    let source_root = fs::canonicalize(&options.source_root)
        .map_err(|_| "source root is unavailable".to_string())?;
    if !source_root.is_dir() {
        return Err("source root is not a directory".to_string());
    }
    let output = resolve_output_path(&options.output, &source_root)?;
    let _ = inspect_existing_output(&output, false)?;
    let output_parent = output
        .parent()
        .ok_or_else(|| "output path has no parent".to_string())?;
    let output_ancestor_guards = open_output_ancestor_guards(output_parent)?;
    ensure_directory_guards(&output_ancestor_guards)?;
    fs::create_dir(&output)
        .map_err(|_| "fresh output directory could not be created".to_string())?;
    let output_guard = open_directory_guard(&output, "fresh output directory")?;
    ensure_directory_guard(&output_guard)?;

    let initial_inventory = super::collect_source_inventory(&source_root)?;
    verify_exact_layout(&specs, &initial_inventory)?;
    let sources = bind_sources(&source_root, &specs, &initial_inventory)?;
    if sources.len() != 6 {
        return Err("source binding did not produce the exact six V01 copies".to_string());
    }
    let source_set = source_set_sha256(&specs_sha256, &sources);
    let reader_sha256 = reader_source_sha256();
    let mut artifacts = Vec::new();
    let mut total_batch_bytes = 0_u64;
    let mut source_summaries = Vec::with_capacity(sources.len());
    let mut total_frames = 0_u64;

    for source in &sources {
        let scanned = scan_copy(
            source,
            &output,
            &specs_sha256,
            &mut artifacts,
            &mut total_batch_bytes,
        )?;
        total_frames = total_frames
            .checked_add(scanned.frame_count)
            .ok_or_else(|| "physical frame count overflow".to_string())?;
        source_summaries.push(scanned.summary);
    }

    let mut coverage = PreconversionCoverage {
        schema_id: COVERAGE_SCHEMA_ID.to_string(),
        status: "INCOMPLETE".to_string(),
        evidence_scope: "preconversion_physical_only_untrusted_observations".to_string(),
        authentic_admission: false,
        semantic_history_assessed: false,
        owner_identity_independently_authenticated: false,
        scope_independently_authenticated: false,
        output_access_control_authenticated: false,
        semantic_family_inventory: "not_assessed".to_string(),
        semantic_family_completeness: "unknown".to_string(),
        cut_inventory: "not_started".to_string(),
        cut_completeness: "unknown".to_string(),
        sidecar_discovery_scope: "exact_v01_copy_tree_only".to_string(),
        binding_provenance: binding.clone(),
        source_root_inventory_sha256: initial_inventory.sha256.clone(),
        source_set_sha256: source_set.clone(),
        expected_copy_table_sha256: specs_sha256.clone(),
        expected_copy_count: "6".to_string(),
        observed_copy_count: source_summaries.len().to_string(),
        physical_frame_scan: "bounded_read_only_physical_scan_complete".to_string(),
        physical_frame_completeness: "all_frames_accepted_by_historical_reader".to_string(),
        physical_frame_count: total_frames.to_string(),
        unclassified_frame_count: total_frames.to_string(),
        sources: source_summaries.clone(),
        reason: "This pre-conversion mode records physical frames and header observations only. It does not independently authenticate owner UUIDs, native scopes, semantic families, or cuts.".to_string(),
    };

    let coverage_path = output.join(COVERAGE_NAME);
    let (coverage_bytes, coverage_write_guard) =
        save_immutable_json(&coverage_path, &coverage, false)?;
    let coverage_sha256 = sha256_bytes(&coverage_bytes);
    let manifest = PreconversionManifest {
        schema_id: MANIFEST_SCHEMA_ID.to_string(),
        status: "INCOMPLETE".to_string(),
        authentic_admission: false,
        semantic_history_assessed: false,
        binding_provenance: binding,
        reader_commit: BASE_READER_COMMIT.to_string(),
        reader_source_sha256: reader_sha256,
        coverage_sha256,
        source_root_inventory_sha256: initial_inventory.sha256.clone(),
        source_set_sha256: source_set,
        expected_copy_count: "6".to_string(),
        observed_copy_count: source_summaries.len().to_string(),
        sources: source_summaries.clone(),
    };
    let manifest_path = output.join(MANIFEST_NAME);
    let (manifest_bytes, manifest_write_guard) =
        save_immutable_json(&manifest_path, &manifest, false)?;

    let mut expected_names = BTreeSet::new();
    for artifact in &artifacts {
        let name = artifact
            .path
            .file_name()
            .ok_or_else(|| "physical batch filename is unavailable".to_string())?
            .to_string_lossy()
            .to_string();
        expected_names.insert(name);
    }
    expected_names.insert(COVERAGE_NAME.to_string());
    expected_names.insert(MANIFEST_NAME.to_string());

    for source in &sources {
        verify_copy_stable(source)?;
    }
    let final_inventory = verify_source_inventory(&source_root, &initial_inventory)?;
    verify_exact_layout(&specs, &final_inventory)?;
    verify_batch_artifacts(&artifacts)?;
    let coverage_guard = verify_report_bytes(&coverage_path, &coverage_bytes, "coverage report")?;
    let manifest_guard = verify_report_bytes(&manifest_path, &manifest_bytes, "manifest report")?;
    verify_output_membership(&output, &expected_names)?;
    ensure_directory_guard(&output_guard)?;
    ensure_directory_guards(&output_ancestor_guards)?;

    // Recheck the source set and every artifact immediately before returning.
    for source in &sources {
        verify_copy_stable(source)?;
    }
    let _last_inventory = verify_source_inventory(&source_root, &initial_inventory)?;
    verify_exact_layout(&specs, &_last_inventory)?;
    verify_batch_artifacts(&artifacts)?;
    let _last_coverage_guard =
        verify_report_bytes(&coverage_path, &coverage_bytes, "coverage report")?;
    let _last_manifest_guard =
        verify_report_bytes(&manifest_path, &manifest_bytes, "manifest report")?;
    verify_output_membership(&output, &expected_names)?;
    ensure_directory_guard(&output_guard)?;
    ensure_directory_guards(&output_ancestor_guards)?;

    // Keep guards live until the final boundary check completes.
    let _held_final_guards = (
        &coverage_write_guard,
        &manifest_write_guard,
        &coverage_guard,
        &manifest_guard,
        &output_guard,
        &output_ancestor_guards,
        &final_inventory,
        &artifacts,
    );
    coverage.sources = source_summaries;
    Ok(coverage)
}

fn parse_options(args: impl Iterator<Item = String>) -> ToolResult<Option<Options>> {
    let mut source_root = None;
    let mut output = None;
    let mut args = args;
    let mut saw_argument = false;
    while let Some(argument) = args.next() {
        saw_argument = true;
        match argument.as_str() {
            "--source-root" => {
                if source_root.is_some() {
                    return Err("duplicate --source-root".to_string());
                }
                source_root = Some(PathBuf::from(
                    args.next().ok_or("--source-root requires a path")?,
                ));
            }
            "--output" => {
                if output.is_some() {
                    return Err("duplicate --output".to_string());
                }
                output = Some(PathBuf::from(
                    args.next().ok_or("--output requires a path")?,
                ));
            }
            "--expected-custody" | "--resume" | "--verify-only" => {
                return Err(
                    "this physical-only mode accepts no custody or replay flags".to_string()
                );
            }
            "--help" | "-h" => return Ok(None),
            _ => return Err("unsupported physical-only command-line argument".to_string()),
        }
    }
    if !saw_argument {
        return Ok(None);
    }
    Ok(Some(Options {
        source_root: source_root.ok_or("--source-root is required")?,
        output: output.ok_or("--output is required")?,
    }))
}

fn production_v01_inputs() -> ToolResult<(Vec<CopySpec>, BindingProvenance)> {
    let specs = v01_copy_specs();
    if copy_table_sha256(&specs) != V01_EXPECTED_COPY_TABLE_SHA256 {
        return Err("pinned V01 copy table digest does not match its independent pin".to_string());
    }
    let binding = copy_table_binding(&specs);
    Ok((specs, binding))
}

pub fn cli_main() -> i32 {
    let options = match parse_options(env::args().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!(
                "Usage: f03-preconversion-physical-census --source-root <verified-copies> --output <fresh-protected-output>"
            );
            return 0;
        }
        Err(_) => {
            eprintln!("{{\"status\":\"FAIL\",\"error\":\"invalid_arguments\"}}");
            return 2;
        }
    };
    let (specs, binding) = match production_v01_inputs() {
        Ok(inputs) => inputs,
        Err(_) => {
            eprintln!("pinned V01 copy table digest mismatch");
            return 2;
        }
    };
    match run_with_specs(options, specs, binding) {
        Ok(report) => {
            println!(
                "{{\"status\":\"{}\",\"authentic_admission\":false,\"observed_copy_count\":\"{}\",\"physical_frame_count\":\"{}\"}}",
                report.status, report.observed_copy_count, report.physical_frame_count
            );
            2
        }
        Err(_) => {
            eprintln!("{{\"status\":\"FAIL\",\"error\":\"physical_census_incomplete\"}}");
            2
        }
    }
}

#[cfg(test)]
#[path = "f03_preconversion_physical_census_tests.rs"]
mod tests;
