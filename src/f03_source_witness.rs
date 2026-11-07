//! Test-only, fail-closed F03 physical carrier census.
//!
//! This exporter records frame hashes and opaque prefix fingerprints only. It
//! deliberately does not interpret frame payloads as semantic records, cuts,
//! or absences. Until an authentic carrier registry and source schemas are
//! reviewed, every frame and semantic group remains unclassified and the
//! machine report is INCOMPLETE.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::UNIX_EPOCH,
};

const BASE_READER_COMMIT: &str = "c6010fd30dd52acb81c667f7aed99b831307e49f";
const PG_C01_COMMIT: &str = "0f824a09fe01f5ff327ab56f3296f36d8f4d7c87";
const PG_C01_BUNDLE_SHA256: &str =
    "7a1abda477874b2a1ff78f75520e075f909750d45a17ce3d6528110334f33688";
const BATCH_FRAME_LIMIT: usize = 128;
const HASH_BUFFER_BYTES: usize = 64 * 1024;
const PREFIX_FINGERPRINT_BYTES: usize = 16;
const BATCH_SCHEMA_ID: &str = "reliquary-f03-physical-frame-batch-v1";
const INITIAL_BATCH_PREDECESSOR: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_CUSTODY_BYTES: u64 = 2 * 1024 * 1024;
const MAX_SOURCE_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_FRAME_PAYLOAD_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CLOCK_STATE_ENTRIES: usize = 65_536;
const MAX_OUTPUT_ARTIFACT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_OUTPUT_ARTIFACTS: usize = 65_536;
const MAX_SOURCE_TREE_ENTRIES: usize = 4_096;
const MAX_SOURCE_TREE_DEPTH: usize = 32;
const MAX_SOURCE_DIRECTORIES: usize = 256;
const MAX_SOURCE_TREE_TOTAL_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const SEMANTIC_GROUPS: [&str; 11] = [
    "container_lineage",
    "archive_echo",
    "memory",
    "entity_relationship",
    "graph_community",
    "ego",
    "freshness",
    "insomnia_dream_import",
    "vectors",
    "external_project_operational",
    "maintenance",
];

const EMBEDDED_MODULE: &[u8] = include_bytes!("f03_source_witness.rs");
const EMBEDDED_LIB: &[u8] = include_bytes!("lib.rs");
const EMBEDDED_CLI: &[u8] = include_bytes!("../examples/f03_source_witness.rs");
const EMBEDDED_CARGO_TOML: &[u8] = include_bytes!("../Cargo.toml");
const EMBEDDED_CARGO_LOCK: &[u8] = include_bytes!("../Cargo.lock");
const EMBEDDED_CONTAINER_RS: &[u8] = include_bytes!("container/container.rs");
const EMBEDDED_CONTAINER_ERROR_RS: &[u8] = include_bytes!("container/container_error.rs");
const EMBEDDED_CONTAINER_SCAN_RS: &[u8] = include_bytes!("container/container_scan.rs");
const EMBEDDED_CONTAINER_VERSION_RS: &[u8] = include_bytes!("container/container_version.rs");

/// One-shot pre-conversion physical-only census. It has a distinct report schema
/// and never loads or infers the strict PG-C01 full-custody document.
#[path = "f03_preconversion_physical_census.rs"]
pub mod preconversion_physical_census;

type ToolResult<T> = Result<T, String>;

#[derive(Debug)]
struct Options {
    source_root: PathBuf,
    expected_custody: PathBuf,
    output: PathBuf,
    resume: bool,
    verify_only: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CustodyDocument {
    schema_id: String,
    contract_id: String,
    owner_files: Vec<CustodyOwnerFile>,
    #[serde(rename = "relocations")]
    _relocations: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CustodyOwnerFile {
    owner_uuid: String,
    owner_kind: String,
    scope: String,
    source_file_sha256: String,
    source_byte_length: String,
    target_file_sha256: String,
    target_byte_length: String,
}

#[derive(Clone, Debug)]
struct CustodySnapshot {
    document: CustodyDocument,
    sha256: String,
    byte_length: u64,
    modified_unix_ns: String,
    file_identity: (u64, u64),
    _read_guard: Arc<File>,
}

#[derive(Clone, Debug)]
struct SourceFile {
    path: PathBuf,
    source_id: String,
    expected: CustodyOwnerFile,
    sha256: String,
    byte_length: u64,
    modified_unix_ns: String,
    file_identity: (u64, u64),
    _read_guard: Arc<File>,
}

#[derive(Clone, Debug)]
struct DirectoryGuard {
    path: PathBuf,
    file_identity: (u64, u64),
    _read_guard: Arc<File>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceInventoryRecord {
    relative_path: String,
    entry_type: String,
    byte_length: u64,
    modified_unix_ns: String,
    file_identity: (u64, u64),
}

#[derive(Debug)]
struct SourceTreeInventory {
    records: Vec<SourceInventoryRecord>,
    sha256: String,
    directory_guards: Vec<DirectoryGuard>,
    _file_guards: Vec<Arc<File>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct FrameObservation {
    source_id: String,
    source_file_sha256: String,
    physical_frame_ordinal: String,
    source_offset: String,
    source_end_offset: String,
    payload_byte_length: String,
    latest_global_clock: String,
    payload_sha256: String,
    prefix_candidate_sha256: String,
    prefix_candidate_byte_length: String,
    semantic_classification: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct BatchPayload {
    schema_id: String,
    reader_commit: String,
    reader_source_sha256: String,
    evidence_contract_sha256: String,
    previous_batch_sha256: String,
    source_id: String,
    source_file_sha256: String,
    batch_index: String,
    first_frame_ordinal: String,
    first_source_offset: String,
    end_source_offset: String,
    frame_count: String,
    frames: Vec<FrameObservation>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct StoredBatch {
    payload: BatchPayload,
    batch_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct SourceSummary {
    source_id: String,
    source_file_sha256: String,
    source_byte_length: String,
    source_modified_unix_ns: String,
    owner_kind: String,
    owner_uuid: String,
    custody_scope: String,
    header_scope_tag: String,
    scope_binding: String,
    physical_frame_count: String,
    physical_end_offset: String,
    physical_latest_global_clock: String,
    batch_count: String,
    batch_stream_sha256: String,
    frame_stream_sha256: String,
    classification: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct SemanticGroupStatus {
    group_id: String,
    presence: String,
    completeness: String,
    row_count: Option<String>,
    reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct CoverageReport {
    schema_id: String,
    contract_id: String,
    status: String,
    evidence_scope: String,
    authentic_admission: bool,
    custody_sha256: String,
    expected_owner_count: String,
    observed_owner_count: String,
    physical_frame_scan: String,
    physical_frame_completeness: String,
    unclassified_frame_count: String,
    semantic_family_inventory: String,
    semantic_family_completeness: String,
    cut_inventory: String,
    cut_completeness: String,
    sidecar_discovery_scope: String,
    source_root_non_owner_file_count: String,
    source_root_inventory_entry_count: String,
    source_root_inventory_sha256: String,
    external_sidecars: String,
    semantic_groups: Vec<SemanticGroupStatus>,
    sources: Vec<SourceSummary>,
    reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ManifestSource {
    source_id: String,
    source_file_sha256: String,
    owner_kind: String,
    owner_uuid: String,
    scope_binding: String,
    physical_frame_count: String,
    batch_count: String,
    batch_stream_sha256: String,
    frame_stream_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct WitnessManifest {
    schema_id: String,
    contract_id: String,
    status: String,
    authentic_admission: bool,
    base_reader_commit: String,
    pg_c01_commit: String,
    pg_c01_bundle_sha256: String,
    reader_source_sha256: String,
    producer_code_sha256: String,
    validation_patch_sha256: String,
    producer_binary_sha256: String,
    cargo_lock_sha256: String,
    custody_sha256: String,
    source_coverage_sha256: String,
    source_root_inventory_entry_count: String,
    source_root_inventory_sha256: String,
    source_count: String,
    sources: Vec<ManifestSource>,
    output_scope: String,
    reason: String,
}

#[derive(Clone, Debug)]
struct ScanResult {
    summary: SourceSummary,
    batch_count: u64,
    batch_sha256s: Vec<String>,
}

#[derive(Debug, Default)]
struct ExistingOutput {
    batch_indexes: BTreeMap<String, BTreeSet<u64>>,
}

#[derive(Clone, Debug)]
struct HashedFile {
    sha256: String,
    byte_length: u64,
    modified_unix_ns: String,
    file_identity: (u64, u64),
}

fn lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn sha256_bytes(bytes: &[u8]) -> String {
    lower_hex(&Sha256::digest(bytes))
}

fn modified_unix_ns(metadata: &fs::Metadata) -> ToolResult<String> {
    let duration = metadata
        .modified()
        .map_err(|error| format!("source metadata modification time unavailable: {error}"))?
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "source modification time predates Unix epoch".to_string())?;
    Ok(duration.as_nanos().to_string())
}

fn canonical_u64(value: &str, field: &str) -> ToolResult<u64> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!("{field} is not canonical unsigned decimal"));
    }
    value
        .parse::<u64>()
        .map_err(|_| format!("{field} exceeds u64"))
}

fn valid_lower_hex(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn file_identity(file: &File) -> ToolResult<(u64, u64)> {
    crate::Container::identity_for_file(file)
        .map_err(|_| "stable file identity unavailable".to_string())?
        .ok_or_else(|| "stable file identity unsupported on this platform".to_string())
}

fn ensure_no_reparse_components(path: &Path, label: &str) -> ToolResult<()> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|_| format!("{label} path base is unavailable"))?
            .join(path)
    };
    let mut current = PathBuf::new();
    let mut missing_prefix = false;
    let mut prefix_pending_root = false;
    let mut component_count = 0_usize;
    for component in absolute.components() {
        component_count = component_count.saturating_add(1);
        if component_count > 256 {
            return Err(format!("{label} path exceeds the component limit"));
        }
        match component {
            Component::CurDir => continue,
            Component::Prefix(prefix) => {
                current.push(prefix.as_os_str());
                prefix_pending_root = true;
                continue;
            }
            Component::RootDir => {
                current.push(component.as_os_str());
                prefix_pending_root = false;
            }
            Component::ParentDir => {
                return Err(format!("{label} path may not contain parent traversal"));
            }
            Component::Normal(name) => current.push(name),
        }
        if prefix_pending_root || missing_prefix {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if is_link_or_reparse(&metadata) => {
                return Err(format!("{label} path contains a symlink or reparse point"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing_prefix = true;
            }
            Err(_) => return Err(format!("{label} path component could not be verified")),
        }
    }
    Ok(())
}

fn open_scoped_handle(path: &Path, directory: bool, label: &str) -> ToolResult<File> {
    ensure_no_reparse_components(path, label)?;
    let path_metadata =
        fs::symlink_metadata(path).map_err(|_| format!("{label} metadata is unavailable"))?;
    if is_link_or_reparse(&path_metadata)
        || (directory && !path_metadata.is_dir())
        || (!directory && !path_metadata.is_file())
    {
        return Err(format!("{label} is not a regular non-reparse entry"));
    }

    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        options.share_mode(FILE_SHARE_READ);
        if directory {
            options.custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
        }
    }
    let file = options
        .open(path)
        .map_err(|_| format!("{label} could not be opened read-only"))?;
    let handle_metadata = file
        .metadata()
        .map_err(|_| format!("{label} handle metadata is unavailable"))?;
    if (directory && !handle_metadata.is_dir()) || (!directory && !handle_metadata.is_file()) {
        return Err(format!("{label} handle type changed while opening"));
    }
    let handle_identity = file_identity(&file)?;
    let path_after =
        fs::symlink_metadata(path).map_err(|_| format!("{label} path changed while opening"))?;
    if is_link_or_reparse(&path_after) {
        return Err(format!("{label} path became a reparse point"));
    }
    let verify_file = {
        let mut verify_options = OpenOptions::new();
        verify_options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            verify_options.share_mode(0x0000_0001);
            if directory {
                verify_options.custom_flags(0x0200_0000);
            }
        }
        verify_options
            .open(path)
            .map_err(|_| format!("{label} path identity could not be reopened"))?
    };
    if file_identity(&verify_file)? != handle_identity {
        return Err(format!("{label} path identity changed while opening"));
    }
    Ok(file)
}

fn open_directory_guard(path: &Path, label: &str) -> ToolResult<DirectoryGuard> {
    let file = open_scoped_handle(path, true, label)?;
    Ok(DirectoryGuard {
        path: path.to_path_buf(),
        file_identity: file_identity(&file)?,
        _read_guard: Arc::new(file),
    })
}

fn open_output_ancestor_guards(parent: &Path) -> ToolResult<Vec<DirectoryGuard>> {
    let canonical_parent = fs::canonicalize(parent)
        .map_err(|_| "output parent directory could not be canonicalized".to_string())?;
    let mut paths = vec![canonical_parent.clone()];
    if let Some(grandparent) = canonical_parent.parent() {
        if grandparent != canonical_parent {
            paths.push(grandparent.to_path_buf());
        }
    }
    let mut guards = Vec::with_capacity(paths.len());
    for path in paths {
        guards.push(open_directory_guard(&path, "output ancestor directory")?);
    }
    Ok(guards)
}

fn ensure_directory_guards(guards: &[DirectoryGuard]) -> ToolResult<()> {
    for guard in guards {
        ensure_directory_guard(guard)?;
    }
    Ok(())
}

fn ensure_directory_guard(guard: &DirectoryGuard) -> ToolResult<()> {
    let handle_identity = file_identity(&guard._read_guard)?;
    if handle_identity != guard.file_identity {
        return Err("held directory identity changed".to_string());
    }
    let path_handle = open_scoped_handle(&guard.path, true, "held directory")?;
    if file_identity(&path_handle)? != guard.file_identity {
        return Err("held directory path no longer names the guarded directory".to_string());
    }
    Ok(())
}

fn snapshots_match(left: &HashedFile, right: &HashedFile) -> bool {
    left.sha256 == right.sha256
        && left.byte_length == right.byte_length
        && left.modified_unix_ns == right.modified_unix_ns
        && left.file_identity == right.file_identity
}

fn hash_file_snapshot(path: &Path) -> ToolResult<HashedFile> {
    let mut file = open_scoped_handle(path, false, "source file")?;
    let file_meta_before = file
        .metadata()
        .map_err(|_| "opened source metadata is unavailable".to_string())?;
    let held_identity = file_identity(&file)?;
    let length_before = file_meta_before.len();
    if length_before > MAX_SOURCE_FILE_BYTES {
        return Err("source file exceeds bounded scan size".to_string());
    }
    let modified_before = modified_unix_ns(&file_meta_before)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| "source file read failed".to_string())?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| "source byte count overflow".to_string())?;
        if total > MAX_SOURCE_FILE_BYTES {
            return Err("source file exceeds bounded scan size".to_string());
        }
        hasher.update(&buffer[..count]);
    }
    let file_meta_after = file
        .metadata()
        .map_err(|_| "opened source metadata reread failed".to_string())?;
    let path_handle = open_scoped_handle(path, false, "source file")?;
    let path_meta_after = path_handle
        .metadata()
        .map_err(|_| "source path metadata reread failed".to_string())?;
    if file_identity(&path_handle)? != held_identity
        || total != length_before
        || file_meta_after.len() != length_before
        || path_meta_after.len() != length_before
        || modified_unix_ns(&file_meta_after)? != modified_before
        || modified_unix_ns(&path_meta_after)? != modified_before
    {
        return Err("source changed during identity-bound hashing".to_string());
    }
    Ok(HashedFile {
        sha256: lower_hex(&hasher.finalize()),
        byte_length: total,
        modified_unix_ns: modified_before,
        file_identity: held_identity,
    })
}

fn hash_container_handle(
    container: &crate::Container,
    expected_identity: (u64, u64),
) -> ToolResult<HashedFile> {
    let mut file = container
        .clone_backing_file()
        .map_err(|_| "historical reader handle could not be cloned".to_string())?;
    let before = file
        .metadata()
        .map_err(|_| "historical reader metadata is unavailable".to_string())?;
    let held_identity = file_identity(&file)?;
    if held_identity != expected_identity || before.len() > MAX_SOURCE_FILE_BYTES {
        return Err("historical reader identity or size is outside custody bounds".to_string());
    }
    let modified = modified_unix_ns(&before)?;
    file.seek(SeekFrom::Start(0))
        .map_err(|_| "historical reader handle could not seek".to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| "historical reader handle read failed".to_string())?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| "historical reader byte count overflow".to_string())?;
        if total > MAX_SOURCE_FILE_BYTES {
            return Err("historical reader source exceeds bounded scan size".to_string());
        }
        hasher.update(&buffer[..count]);
    }
    let after = file
        .metadata()
        .map_err(|_| "historical reader metadata reread failed".to_string())?;
    if file_identity(&file)? != expected_identity
        || total != before.len()
        || after.len() != before.len()
        || modified_unix_ns(&after)? != modified
    {
        return Err("historical reader source changed during handle hashing".to_string());
    }
    Ok(HashedFile {
        sha256: lower_hex(&hasher.finalize()),
        byte_length: total,
        modified_unix_ns: modified,
        file_identity: held_identity,
    })
}

fn read_bounded_snapshot(
    path: &Path,
    max_bytes: u64,
    label: &str,
) -> ToolResult<(Vec<u8>, HashedFile, Arc<File>)> {
    let file = open_scoped_handle(path, false, label)?;
    let before = file
        .metadata()
        .map_err(|_| format!("{label} handle metadata is unavailable"))?;
    let held_identity = file_identity(&file)?;
    if before.len() > max_bytes {
        return Err(format!("{label} exceeds its byte limit"));
    }
    let modified = modified_unix_ns(&before)?;
    let mut reader = file
        .try_clone()
        .map_err(|_| format!("{label} handle could not be cloned"))?
        .take(max_bytes.saturating_add(1));
    let mut bytes = Vec::with_capacity(before.len() as usize);
    reader
        .read_to_end(&mut bytes)
        .map_err(|_| format!("{label} read failed"))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!("{label} grew beyond its byte limit"));
    }
    let after = file
        .metadata()
        .map_err(|_| format!("{label} handle metadata reread failed"))?;
    let path_handle = open_scoped_handle(path, false, label)?;
    let path_after = path_handle
        .metadata()
        .map_err(|_| format!("{label} path metadata reread failed"))?;
    if file_identity(&path_handle)? != held_identity
        || before.len() != bytes.len() as u64
        || after.len() != before.len()
        || path_after.len() != before.len()
        || modified_unix_ns(&after)? != modified
        || modified_unix_ns(&path_after)? != modified
    {
        return Err(format!("{label} changed during bounded read"));
    }
    let snapshot = HashedFile {
        sha256: sha256_bytes(&bytes),
        byte_length: bytes.len() as u64,
        modified_unix_ns: modified,
        file_identity: held_identity,
    };
    Ok((bytes, snapshot, Arc::new(file)))
}

fn load_custody(path: &Path) -> ToolResult<CustodySnapshot> {
    let (bytes, hashed, read_guard) =
        read_bounded_snapshot(path, MAX_CUSTODY_BYTES, "expected custody")?;
    let document: CustodyDocument = serde_json::from_slice(&bytes)
        .map_err(|_| "expected custody is invalid strict JSON".to_string())?;
    if document.schema_id != "reliquary-corpus-physical-custody-v1"
        || document.contract_id != "reliquary-corpus-evidence-v1"
    {
        return Err("expected custody schema or contract id is unsupported".to_string());
    }
    if document.owner_files.len() != 6 {
        return Err("expected custody must bind exactly six source owners".to_string());
    }
    let mut owner_keys = BTreeSet::new();
    let mut source_hashes = BTreeSet::new();
    for owner in &document.owner_files {
        if !valid_lower_hex(&owner.owner_uuid, 32)
            || !valid_lower_hex(&owner.source_file_sha256, 64)
            || !valid_lower_hex(&owner.target_file_sha256, 64)
        {
            return Err("expected custody contains a malformed owner binding".to_string());
        }
        if owner.owner_kind != "REL" && owner.owner_kind != "PHY" {
            return Err("expected custody contains an unsupported owner kind".to_string());
        }
        canonical_u64(&owner.source_byte_length, "source_byte_length")?;
        canonical_u64(&owner.target_byte_length, "target_byte_length")?;
        if !owner_keys.insert((
            owner.owner_uuid.clone(),
            owner.owner_kind.clone(),
            owner.scope.clone(),
        )) {
            return Err("expected custody contains a duplicate full owner key".to_string());
        }
        if !source_hashes.insert(owner.source_file_sha256.clone()) {
            return Err("expected custody contains a duplicate source file hash".to_string());
        }
    }
    let after_parse = hash_file_snapshot(path)?;
    if !snapshots_match(&after_parse, &hashed) {
        return Err("expected custody changed during validation".to_string());
    }
    let held_identity = file_identity(&read_guard)?;
    if held_identity != hashed.file_identity {
        return Err("expected custody read handle identity changed".to_string());
    }
    Ok(CustodySnapshot {
        document,
        sha256: hashed.sha256,
        byte_length: hashed.byte_length,
        modified_unix_ns: hashed.modified_unix_ns,
        file_identity: hashed.file_identity,
        _read_guard: read_guard,
    })
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
    directory_guards: &mut Vec<DirectoryGuard>,
    total_entries: &mut usize,
    depth: usize,
) -> ToolResult<()> {
    if depth > MAX_SOURCE_TREE_DEPTH {
        return Err("source-root nesting exceeds the depth limit".to_string());
    }
    if directory_guards.len() >= MAX_SOURCE_DIRECTORIES {
        return Err("source-root directory count exceeds the scan limit".to_string());
    }
    let canonical_directory = fs::canonicalize(directory)
        .map_err(|_| "source-root directory could not be canonicalized".to_string())?;
    if !canonical_directory.starts_with(root) {
        return Err("source-root directory resolves outside its verified root".to_string());
    }
    directory_guards.push(open_directory_guard(
        &canonical_directory,
        "source directory",
    )?);
    for entry in fs::read_dir(&canonical_directory)
        .map_err(|_| "source-root directory could not be enumerated".to_string())?
    {
        *total_entries = total_entries
            .checked_add(1)
            .ok_or_else(|| "source-root entry count overflow".to_string())?;
        if *total_entries > MAX_SOURCE_TREE_ENTRIES {
            return Err("source-root entry count exceeds the scan limit".to_string());
        }
        let entry = entry.map_err(|_| "source-root entry could not be read".to_string())?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| "source-root entry metadata is unavailable".to_string())?;
        if is_link_or_reparse(&metadata) {
            return Err("source-root contains a symlink or reparse point".to_string());
        }
        if metadata.is_dir() {
            collect_files(
                root,
                &path,
                files,
                directory_guards,
                total_entries,
                depth + 1,
            )?;
        } else if metadata.is_file() {
            let canonical = fs::canonicalize(&path)
                .map_err(|_| "source-root file could not be canonicalized".to_string())?;
            if !canonical.starts_with(root) {
                return Err("source-root file resolves outside its verified root".to_string());
            }
            files.push(canonical);
        } else {
            return Err("source-root contains a non-file entry".to_string());
        }
    }
    Ok(())
}

fn relative_inventory_path(root: &Path, path: &Path) -> ToolResult<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "source inventory entry escaped its root".to_string())?;
    let value = relative
        .to_str()
        .ok_or_else(|| "source inventory contains a non-UTF-8 path".to_string())?;
    Ok(value.replace('\\', "/"))
}

fn inventory_hash(records: &[SourceInventoryRecord]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"reliquary-f03-source-tree-inventory-v1\0");
    hasher.update((records.len() as u64).to_le_bytes());
    for record in records {
        for value in [
            record.relative_path.as_bytes(),
            record.entry_type.as_bytes(),
            record.modified_unix_ns.as_bytes(),
        ] {
            hasher.update((value.len() as u64).to_le_bytes());
            hasher.update(value);
        }
        hasher.update(record.byte_length.to_le_bytes());
        hasher.update(record.file_identity.0.to_le_bytes());
        hasher.update(record.file_identity.1.to_le_bytes());
    }
    lower_hex(&hasher.finalize())
}

fn physical_link_count(file: &File) -> ToolResult<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        #[repr(C)]
        struct Information {
            attributes: u32,
            creation: [u32; 2],
            access: [u32; 2],
            write: [u32; 2],
            volume: u32,
            size_high: u32,
            size_low: u32,
            links: u32,
            index_high: u32,
            index_low: u32,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetFileInformationByHandle(
                handle: *mut std::ffi::c_void,
                information: *mut Information,
            ) -> i32;
        }
        let mut information = std::mem::MaybeUninit::<Information>::uninit();
        // SAFETY: file owns a live handle and the OS fills the declared structure.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) }
            == 0
        {
            return Err("source hard-link count is unavailable".to_string());
        }
        // SAFETY: success initializes every field of Information.
        let information = unsafe { information.assume_init() };
        return Ok(u64::from(information.links));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = file
            .metadata()
            .map_err(|_| "source hard-link metadata is unavailable".to_string())?;
        return Ok(metadata.nlink());
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = file;
        Err("physical hard-link count is unsupported on this platform".to_string())
    }
}

fn capture_source_inventory(
    root: &Path,
    files: &[PathBuf],
    directory_guards: Vec<DirectoryGuard>,
) -> ToolResult<SourceTreeInventory> {
    if files.len() + directory_guards.len() > MAX_SOURCE_TREE_ENTRIES {
        return Err("source inventory exceeds the entry limit".to_string());
    }
    let mut records = Vec::with_capacity(files.len() + directory_guards.len());
    let mut file_guards = Vec::with_capacity(files.len());
    let mut identities = BTreeSet::new();
    let mut total_bytes = 0_u64;
    for guard in &directory_guards {
        ensure_directory_guard(guard)?;
        let metadata = guard
            ._read_guard
            .metadata()
            .map_err(|_| "source inventory directory metadata unavailable".to_string())?;
        records.push(SourceInventoryRecord {
            relative_path: relative_inventory_path(root, &guard.path)?,
            entry_type: "directory".to_string(),
            byte_length: 0,
            modified_unix_ns: modified_unix_ns(&metadata)?,
            file_identity: guard.file_identity,
        });
    }
    for path in files {
        let file = open_scoped_handle(path, false, "source inventory file")?;
        let metadata = file
            .metadata()
            .map_err(|_| "source inventory file metadata unavailable".to_string())?;
        let identity = file_identity(&file)?;
        let path_metadata = fs::symlink_metadata(path)
            .map_err(|_| "source inventory file path changed".to_string())?;
        if is_link_or_reparse(&path_metadata) || !path_metadata.is_file() {
            return Err("source inventory encountered an unsafe file entry".to_string());
        }
        if metadata.len() > MAX_SOURCE_FILE_BYTES {
            return Err("source inventory file exceeds its byte limit".to_string());
        }
        if physical_link_count(&file)? != 1 {
            return Err("source inventory requires one hard link per file".to_string());
        }
        total_bytes = total_bytes
            .checked_add(metadata.len())
            .ok_or_else(|| "source inventory byte total overflow".to_string())?;
        if total_bytes > MAX_SOURCE_TREE_TOTAL_BYTES {
            return Err("source inventory total size exceeds its limit".to_string());
        }
        if !identities.insert(identity) {
            return Err("source inventory contains an in-root hard-link alias".to_string());
        }
        records.push(SourceInventoryRecord {
            relative_path: relative_inventory_path(root, path)?,
            entry_type: "file".to_string(),
            byte_length: metadata.len(),
            modified_unix_ns: modified_unix_ns(&metadata)?,
            file_identity: identity,
        });
        file_guards.push(Arc::new(file));
    }
    records.sort_by(|left, right| {
        left.relative_path
            .as_bytes()
            .cmp(right.relative_path.as_bytes())
            .then_with(|| left.entry_type.as_bytes().cmp(right.entry_type.as_bytes()))
    });
    let sha256 = inventory_hash(&records);
    Ok(SourceTreeInventory {
        records,
        sha256,
        directory_guards,
        _file_guards: file_guards,
    })
}

fn collect_source_inventory(root: &Path) -> ToolResult<SourceTreeInventory> {
    let canonical_root =
        fs::canonicalize(root).map_err(|_| "source inventory root is unavailable".to_string())?;
    let mut files = Vec::new();
    let mut directory_guards = Vec::new();
    let mut total_entries = 0;
    collect_files(
        &canonical_root,
        &canonical_root,
        &mut files,
        &mut directory_guards,
        &mut total_entries,
        0,
    )?;
    files.sort_by(|left, right| {
        relative_inventory_path(&canonical_root, left)
            .unwrap_or_default()
            .as_bytes()
            .cmp(
                relative_inventory_path(&canonical_root, right)
                    .unwrap_or_default()
                    .as_bytes(),
            )
    });
    capture_source_inventory(&canonical_root, &files, directory_guards)
}

fn verify_source_inventory(
    root: &Path,
    expected: &SourceTreeInventory,
) -> ToolResult<SourceTreeInventory> {
    let current = collect_source_inventory(root)?;
    if current.sha256 != expected.sha256 || current.records != expected.records {
        return Err("source tree membership or identity changed before finalization".to_string());
    }
    Ok(current)
}

fn owner_extension(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(OsStr::to_str) {
        Some(extension) if extension.eq_ignore_ascii_case("rel") => Some("REL"),
        Some(extension) if extension.eq_ignore_ascii_case("phy") => Some("PHY"),
        _ => None,
    }
}

fn resolve_sources(
    source_root: &Path,
    custody: &CustodySnapshot,
) -> ToolResult<(Vec<SourceFile>, u64, SourceTreeInventory)> {
    ensure_no_reparse_components(source_root, "source root")?;
    let canonical_root =
        fs::canonicalize(source_root).map_err(|_| "source root is unavailable".to_string())?;
    if !canonical_root.is_dir() {
        return Err("source root is not a directory".to_string());
    }
    let mut files = Vec::new();
    let mut directory_guards = Vec::new();
    let mut total_entries = 0;
    collect_files(
        &canonical_root,
        &canonical_root,
        &mut files,
        &mut directory_guards,
        &mut total_entries,
        0,
    )?;
    files.sort_by(|left, right| {
        left.strip_prefix(&canonical_root)
            .unwrap_or(left)
            .to_string_lossy()
            .as_bytes()
            .cmp(
                right
                    .strip_prefix(&canonical_root)
                    .unwrap_or(right)
                    .to_string_lossy()
                    .as_bytes(),
            )
    });
    let inventory = capture_source_inventory(&canonical_root, &files, directory_guards)?;
    let mut owner_candidates = Vec::new();
    let mut non_owner_file_count = 0_u64;
    for path in files {
        if owner_extension(&path).is_some() {
            owner_candidates.push(path);
        } else {
            non_owner_file_count = non_owner_file_count
                .checked_add(1)
                .ok_or_else(|| "source-root non-owner file count overflow".to_string())?;
        }
    }
    if owner_candidates.len() != custody.document.owner_files.len() {
        return Err("source root owner-file count does not match expected custody".to_string());
    }
    let mut expected_by_hash = BTreeMap::new();
    for owner in &custody.document.owner_files {
        expected_by_hash.insert(owner.source_file_sha256.clone(), owner.clone());
    }
    let mut sources = Vec::with_capacity(owner_candidates.len());
    let mut seen_hashes = BTreeSet::new();
    for path in owner_candidates {
        let extension_kind = owner_extension(&path)
            .ok_or_else(|| "source owner extension disappeared".to_string())?;
        let read_guard = Arc::new(open_scoped_handle(&path, false, "source owner")?);
        let guard_identity = file_identity(&read_guard)?;
        let hashed = hash_file_snapshot(&path)?;
        if hashed.file_identity != guard_identity {
            return Err("source owner hash does not match its held read handle".to_string());
        }
        let expected = expected_by_hash
            .get(&hashed.sha256)
            .cloned()
            .ok_or_else(|| "source owner hash is not present in expected custody".to_string())?;
        if expected.owner_kind != extension_kind
            || canonical_u64(&expected.source_byte_length, "source_byte_length")?
                != hashed.byte_length
        {
            return Err("source owner kind or size disagrees with expected custody".to_string());
        }
        if !seen_hashes.insert(hashed.sha256.clone()) {
            return Err("source root contains a duplicate custody-bound source".to_string());
        }
        sources.push(SourceFile {
            path,
            source_id: String::new(),
            expected,
            sha256: hashed.sha256,
            byte_length: hashed.byte_length,
            modified_unix_ns: hashed.modified_unix_ns,
            file_identity: hashed.file_identity,
            _read_guard: read_guard,
        });
    }
    if seen_hashes.len() != expected_by_hash.len() {
        return Err("one or more expected source owners are missing".to_string());
    }
    sources.sort_by(|left, right| left.sha256.cmp(&right.sha256));
    for (index, source) in sources.iter_mut().enumerate() {
        source.source_id = format!("source-{:04}", index + 1);
    }
    for guard in &inventory.directory_guards {
        ensure_directory_guard(guard)?;
    }
    Ok((sources, non_owner_file_count, inventory))
}

fn header_scope_tag(scope: Option<crate::ReliquaryScopeKind>) -> &'static str {
    match scope {
        None => "none",
        Some(crate::ReliquaryScopeKind::Organization) => "organization",
        Some(crate::ReliquaryScopeKind::Project) => "project",
        Some(crate::ReliquaryScopeKind::Connection) => "connection",
    }
}

fn kind_label(kind: crate::FileKind) -> &'static str {
    match kind {
        crate::FileKind::Reliquary => "REL",
        crate::FileKind::Phylactery => "PHY",
    }
}

fn hex_uuid(bytes: &[u8; 16]) -> String {
    lower_hex(bytes)
}

fn frame_observation(
    source_id: &str,
    source_sha256: &str,
    ordinal: u64,
    frame: &crate::container::PhysicalFrame,
) -> FrameObservation {
    let payload_sha256 = sha256_bytes(&frame.payload);
    let prefix_length = frame.payload.len().min(PREFIX_FINGERPRINT_BYTES);
    let prefix_candidate_sha256 = sha256_bytes(&frame.payload[..prefix_length]);
    FrameObservation {
        source_id: source_id.to_string(),
        source_file_sha256: source_sha256.to_string(),
        physical_frame_ordinal: ordinal.to_string(),
        source_offset: frame.object.offset().to_string(),
        source_end_offset: frame.end.to_string(),
        payload_byte_length: frame.payload.len().to_string(),
        latest_global_clock: frame.latest_global.to_string(),
        payload_sha256,
        prefix_candidate_sha256,
        prefix_candidate_byte_length: prefix_length.to_string(),
        semantic_classification: "unclassified".to_string(),
    }
}

fn serialize_with_newline<T: Serialize>(value: &T) -> ToolResult<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value)
        .map_err(|error| format!("evidence serialization failed: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn encode_batch(payload: BatchPayload) -> ToolResult<(Vec<u8>, String)> {
    if payload.schema_id != BATCH_SCHEMA_ID
        || payload.reader_commit != BASE_READER_COMMIT
        || payload.reader_source_sha256 != reader_source_sha256()
        || payload.evidence_contract_sha256 != PG_C01_BUNDLE_SHA256
        || !valid_lower_hex(&payload.previous_batch_sha256, 64)
    {
        return Err("batch reader or chain binding is unsupported".to_string());
    }
    if canonical_u64(&payload.batch_index, "batch_index")? > 999_999 {
        return Err("batch count exceeds filename encoding limit".to_string());
    }
    let canonical_payload = serde_json::to_vec(&payload)
        .map_err(|error| format!("batch payload serialization failed: {error}"))?;
    let batch_sha256 = sha256_bytes(&canonical_payload);
    let stored = StoredBatch {
        payload,
        batch_sha256: batch_sha256.clone(),
    };
    Ok((serialize_with_newline(&stored)?, batch_sha256))
}

fn batch_path(output: &Path, source_id: &str, batch_index: u64) -> PathBuf {
    output.join(format!("{source_id}-batch-{batch_index:06}.json"))
}

fn save_immutable(path: &Path, bytes: &[u8], resume: bool) -> ToolResult<()> {
    if bytes.len() as u64 > MAX_OUTPUT_ARTIFACT_BYTES {
        return Err("output batch exceeds the artifact size limit".to_string());
    }
    ensure_no_reparse_components(path, "output artifact")?;
    let path_exists = match fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => return Err("output batch path could not be inspected".to_string()),
    };
    if path_exists {
        if !resume {
            return Err("output artifact already exists; refusing overwrite".to_string());
        }
        let (existing, _, _guard) =
            read_bounded_snapshot(path, MAX_OUTPUT_ARTIFACT_BYTES, "existing batch")?;
        let parsed: StoredBatch = serde_json::from_slice(&existing)
            .map_err(|_| "existing batch is invalid JSON".to_string())?;
        let canonical_payload = serde_json::to_vec(&parsed.payload)
            .map_err(|_| "existing batch payload serialization failed".to_string())?;
        if sha256_bytes(&canonical_payload) != parsed.batch_sha256 || existing != bytes {
            return Err("existing batch does not match deterministic source replay".to_string());
        }
        return Ok(());
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0x0000_0001);
    }
    let mut file = options
        .open(path)
        .map_err(|_| "new output batch could not be created".to_string())?;
    file.write_all(bytes)
        .map_err(|_| "output batch write failed".to_string())?;
    file.sync_all()
        .map_err(|_| "output batch sync failed".to_string())?;
    Ok(())
}

fn save_immutable_json<T: Serialize + for<'de> Deserialize<'de>>(
    path: &Path,
    value: &T,
    resume: bool,
) -> ToolResult<(Vec<u8>, Arc<File>)> {
    let bytes = serialize_with_newline(value)?;
    if bytes.len() as u64 > MAX_OUTPUT_ARTIFACT_BYTES {
        return Err("final output exceeds the artifact size limit".to_string());
    }
    ensure_no_reparse_components(path, "final output")?;
    let path_exists = match fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => return Err("final output path could not be inspected".to_string()),
    };
    if path_exists {
        if !resume {
            return Err("final output artifact already exists; refusing overwrite".to_string());
        }
        let (existing, _, _guard) =
            read_bounded_snapshot(path, MAX_OUTPUT_ARTIFACT_BYTES, "final output")?;
        if existing != bytes {
            return Err("existing final output does not match deterministic replay".to_string());
        }
        let _: T = serde_json::from_slice(&existing)
            .map_err(|_| "existing final output is invalid JSON".to_string())?;
    } else {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0x0000_0001);
        }
        let mut file = options
            .open(path)
            .map_err(|_| "final output could not be created".to_string())?;
        file.write_all(&bytes)
            .map_err(|_| "final output write failed".to_string())?;
        file.sync_all()
            .map_err(|_| "final output sync failed".to_string())?;
    }
    let guard = verify_immutable_output(path, &bytes, "final output")?;
    Ok((bytes, guard))
}

fn verify_immutable_output(path: &Path, expected: &[u8], label: &str) -> ToolResult<Arc<File>> {
    if expected.len() as u64 > MAX_OUTPUT_ARTIFACT_BYTES {
        return Err(format!("{label} exceeds the artifact size limit"));
    }
    let (observed, _, guard) = read_bounded_snapshot(path, MAX_OUTPUT_ARTIFACT_BYTES, label)?;
    if observed != expected {
        return Err(format!("{label} changed from its serialized value"));
    }
    Ok(guard)
}

fn inspect_existing_output(output: &Path, resume: bool) -> ToolResult<ExistingOutput> {
    ensure_no_reparse_components(output, "output directory")?;
    let exists = match fs::symlink_metadata(output) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => return Err("output directory could not be inspected".to_string()),
    };
    if !resume {
        if exists {
            return Err("output directory already exists; use a new path".to_string());
        }
        return Ok(ExistingOutput::default());
    }
    if !exists {
        return Err("resume output directory does not exist".to_string());
    }
    let guard = open_directory_guard(output, "resume output directory")?;
    let mut existing = ExistingOutput::default();
    let mut artifact_count = 0_usize;
    for entry in fs::read_dir(output)
        .map_err(|_| "resume output directory could not be enumerated".to_string())?
    {
        artifact_count = artifact_count
            .checked_add(1)
            .ok_or_else(|| "resume artifact count overflow".to_string())?;
        if artifact_count > MAX_OUTPUT_ARTIFACTS {
            return Err("resume output artifact count exceeds the limit".to_string());
        }
        let entry = entry.map_err(|_| "resume output entry could not be read".to_string())?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|_| "resume output artifact metadata is unavailable".to_string())?;
        if !metadata.is_file()
            || is_link_or_reparse(&metadata)
            || metadata.len() > MAX_OUTPUT_ARTIFACT_BYTES
        {
            return Err("resume output contains an invalid or oversized artifact".to_string());
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "resume output filename is not UTF-8".to_string())?;
        if name == "source-coverage.json" || name == "source-witness-manifest.json" {
            continue;
        }
        let Some((source_id, batch_index)) = parse_batch_filename(&name) else {
            return Err("resume output contains an unrecognized artifact".to_string());
        };
        existing
            .batch_indexes
            .entry(source_id)
            .or_default()
            .insert(batch_index);
    }
    for indexes in existing.batch_indexes.values() {
        if indexes
            .iter()
            .copied()
            .enumerate()
            .any(|(expected, actual)| expected as u64 != actual)
        {
            return Err("resume output has a batch gap".to_string());
        }
    }
    ensure_directory_guard(&guard)?;
    Ok(existing)
}

fn parse_batch_filename(name: &str) -> Option<(String, u64)> {
    let (source_id, rest) = name.split_once("-batch-")?;
    if !source_id.starts_with("source-")
        || source_id.len() != 11
        || !source_id[7..].bytes().all(|byte| byte.is_ascii_digit())
        || !rest.ends_with(".json")
    {
        return None;
    }
    let index = &rest[..rest.len().checked_sub(5)?];
    if index.len() != 6 || !index.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((source_id.to_string(), index.parse().ok()?))
}

fn flush_batch(
    output: Option<&Path>,
    resume: bool,
    source: &SourceFile,
    batch_index: u64,
    reader_sha256: &str,
    previous_batch_sha256: &mut String,
    batch_sha256s: &mut Vec<String>,
    frames: &mut Vec<FrameObservation>,
    batch_stream_hasher: &mut Sha256,
) -> ToolResult<()> {
    if frames.is_empty() {
        return Ok(());
    }
    if batch_index >= (MAX_OUTPUT_ARTIFACTS / 6) as u64 {
        return Err("source batch count exceeds bounded output inventory".to_string());
    }
    let first = frames.first().expect("non-empty batch");
    let last = frames.last().expect("non-empty batch");
    let payload = BatchPayload {
        schema_id: BATCH_SCHEMA_ID.to_string(),
        reader_commit: BASE_READER_COMMIT.to_string(),
        reader_source_sha256: reader_sha256.to_string(),
        evidence_contract_sha256: PG_C01_BUNDLE_SHA256.to_string(),
        previous_batch_sha256: previous_batch_sha256.clone(),
        source_id: source.source_id.clone(),
        source_file_sha256: source.sha256.clone(),
        batch_index: batch_index.to_string(),
        first_frame_ordinal: first.physical_frame_ordinal.clone(),
        first_source_offset: first.source_offset.clone(),
        end_source_offset: last.source_end_offset.clone(),
        frame_count: frames.len().to_string(),
        frames: std::mem::take(frames),
    };
    let (bytes, batch_sha256) = encode_batch(payload)?;
    if let Some(output) = output {
        save_immutable(
            &batch_path(output, &source.source_id, batch_index),
            &bytes,
            resume,
        )?;
    }
    batch_stream_hasher.update(batch_sha256.as_bytes());
    *previous_batch_sha256 = batch_sha256.clone();
    batch_sha256s.push(batch_sha256);
    Ok(())
}

fn scan_source_frames(
    source: &SourceFile,
    expected_file_size: u64,
    expected_file_identity: (u64, u64),
    output: Option<&Path>,
    resume: bool,
) -> ToolResult<ScanResult> {
    let container = crate::Container::open_read_only_for_scan(&source.path)
        .map_err(|error| format!("read-only historical container open failed: {error}"))?;
    let opened_identity = container
        .backing_file_identity()
        .map_err(|_| "opened source file identity is unavailable".to_string())?
        .ok_or_else(|| "opened source file identity is unsupported".to_string())?;
    if opened_identity != expected_file_identity {
        return Err("historical reader handle does not match the custody-hashed file".to_string());
    }
    let identity = container
        .identity()
        .ok_or_else(|| "container has no identity header".to_string())?;
    let actual_kind = kind_label(identity.file_kind);
    let actual_uuid = container
        .owner_uuid()
        .map(|uuid| hex_uuid(&uuid))
        .ok_or_else(|| "container has no owner UUID".to_string())?;
    if actual_kind != source.expected.owner_kind || actual_uuid != source.expected.owner_uuid {
        return Err("container header owner binding disagrees with custody".to_string());
    }
    let actual_scope = header_scope_tag(identity.scope).to_string();
    let physical_length = container
        .physical_length()
        .map_err(|error| format!("container physical length read failed: {error}"))?;
    let handle_before = hash_container_handle(&container, expected_file_identity)?;
    if handle_before.sha256 != source.sha256
        || handle_before.byte_length != source.byte_length
        || handle_before.modified_unix_ns != source.modified_unix_ns
    {
        return Err("historical reader handle does not match the custody-hashed bytes".to_string());
    }
    if physical_length != expected_file_size {
        return Err("container length changed after custody verification".to_string());
    }
    container
        .validate_path_identity()
        .map_err(|error| format!("container identity validation failed: {error}"))?;
    let mut cursor = container
        .physical_cursor_for_source_witness(MAX_FRAME_PAYLOAD_BYTES, MAX_CLOCK_STATE_ENTRIES)
        .map_err(|_| "bounded physical cursor could not be opened".to_string())?;
    let mut observations = Vec::with_capacity(BATCH_FRAME_LIMIT);
    let mut frame_count = 0_u64;
    let mut batch_index = 0_u64;
    let mut latest_global = 0_u64;
    let mut frame_stream_hasher = Sha256::new();
    let mut batch_stream_hasher = Sha256::new();
    let reader_sha256 = reader_source_sha256();
    let mut previous_batch_sha256 = INITIAL_BATCH_PREDECESSOR.to_string();
    let mut batch_sha256s = Vec::new();
    let mut last_end = container.header_len;
    loop {
        let frame = cursor
            .next_frame()
            .map_err(|error| format!("physical source frame is invalid or unavailable: {error}"))?;
        let Some(frame) = frame else {
            break;
        };
        let observation = frame_observation(&source.source_id, &source.sha256, frame_count, &frame);
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
        observations.push(observation);
        if observations.len() == BATCH_FRAME_LIMIT {
            flush_batch(
                output,
                resume,
                source,
                batch_index,
                &reader_sha256,
                &mut previous_batch_sha256,
                &mut batch_sha256s,
                &mut observations,
                &mut batch_stream_hasher,
            )?;
            batch_index = batch_index
                .checked_add(1)
                .ok_or_else(|| "physical batch count overflow".to_string())?;
        }
    }
    if !observations.is_empty() {
        flush_batch(
            output,
            resume,
            source,
            batch_index,
            &reader_sha256,
            &mut previous_batch_sha256,
            &mut batch_sha256s,
            &mut observations,
            &mut batch_stream_hasher,
        )?;
        batch_index = batch_index
            .checked_add(1)
            .ok_or_else(|| "physical batch count overflow".to_string())?;
    }
    drop(cursor);
    if last_end != physical_length {
        return Err("physical scan did not end at the verified file length".to_string());
    }
    container
        .validate_path_identity()
        .map_err(|error| format!("container identity validation failed after scan: {error}"))?;
    let final_physical_length = container
        .physical_length()
        .map_err(|error| format!("container physical length reread failed: {error}"))?;
    if final_physical_length != physical_length {
        return Err("source physical length changed during frame scan".to_string());
    }
    if container.backing_file_identity().ok().flatten() != Some(expected_file_identity) {
        return Err("historical reader identity changed during frame scan".to_string());
    }
    let handle_after = hash_container_handle(&container, expected_file_identity)?;
    if !snapshots_match(&handle_after, &handle_before) || handle_after.sha256 != source.sha256 {
        return Err("historical reader handle changed during frame scan".to_string());
    }
    Ok(ScanResult {
        summary: SourceSummary {
            source_id: source.source_id.clone(),
            source_file_sha256: source.sha256.clone(),
            source_byte_length: source.byte_length.to_string(),
            source_modified_unix_ns: source.modified_unix_ns.clone(),
            owner_kind: actual_kind.to_string(),
            owner_uuid: actual_uuid,
            custody_scope: source.expected.scope.clone(),
            header_scope_tag: actual_scope,
            scope_binding: "unverified-by-physical-census".to_string(),
            physical_frame_count: frame_count.to_string(),
            physical_end_offset: physical_length.to_string(),
            physical_latest_global_clock: latest_global.to_string(),
            batch_count: batch_index.to_string(),
            batch_stream_sha256: lower_hex(&batch_stream_hasher.finalize()),
            frame_stream_sha256: lower_hex(&frame_stream_hasher.finalize()),
            classification: "unclassified".to_string(),
        },
        batch_count: batch_index,
        batch_sha256s,
    })
}

fn scan_source_with_stability(
    source: &SourceFile,
    output: Option<&Path>,
    resume: bool,
) -> ToolResult<ScanResult> {
    let held_identity = file_identity(&source._read_guard)?;
    if held_identity != source.file_identity {
        return Err("held source identity no longer matches custody".to_string());
    }
    let before = hash_file_snapshot(&source.path)?;
    if before.sha256 != source.sha256
        || before.byte_length != source.byte_length
        || before.modified_unix_ns != source.modified_unix_ns
        || before.file_identity != source.file_identity
    {
        return Err("source changed after initial custody binding".to_string());
    }
    let scan = scan_source_frames(
        source,
        source.byte_length,
        source.file_identity,
        output,
        resume,
    );
    let after = hash_file_snapshot(&source.path)?;
    if after.sha256 != before.sha256
        || after.byte_length != before.byte_length
        || after.modified_unix_ns != before.modified_unix_ns
        || after.file_identity != before.file_identity
    {
        return Err("source changed during physical scan".to_string());
    }
    scan
}

fn ensure_source_stable(source: &SourceFile) -> ToolResult<()> {
    if file_identity(&source._read_guard)? != source.file_identity {
        return Err("held source identity changed before finalization".to_string());
    }
    let current = hash_file_snapshot(&source.path)?;
    if current.sha256 != source.sha256
        || current.byte_length != source.byte_length
        || current.modified_unix_ns != source.modified_unix_ns
        || current.file_identity != source.file_identity
    {
        return Err("source changed before finalization".to_string());
    }
    Ok(())
}

fn ensure_sources_stable(sources: &[SourceFile]) -> ToolResult<()> {
    for source in sources {
        ensure_source_stable(source)?;
    }
    Ok(())
}

fn ensure_custody_stable(path: &Path, custody: &CustodySnapshot) -> ToolResult<()> {
    if file_identity(&custody._read_guard)? != custody.file_identity {
        return Err("held custody identity changed".to_string());
    }
    let after = hash_file_snapshot(path)?;
    let expected = HashedFile {
        sha256: custody.sha256.clone(),
        byte_length: custody.byte_length,
        modified_unix_ns: custody.modified_unix_ns.clone(),
        file_identity: custody.file_identity,
    };
    if !snapshots_match(&after, &expected) {
        return Err("expected custody changed during export".to_string());
    }
    Ok(())
}

fn resolve_output_path(output: &Path, source_root: &Path) -> ToolResult<PathBuf> {
    ensure_no_reparse_components(source_root, "source root")?;
    ensure_no_reparse_components(output, "output")?;
    let source_root = fs::canonicalize(source_root)
        .map_err(|_| "source root canonicalization failed".to_string())?;
    let output_exists = match fs::symlink_metadata(output) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => return Err("output path could not be inspected".to_string()),
    };
    let resolved = if output_exists {
        let metadata = fs::symlink_metadata(output)
            .map_err(|_| "output metadata is unavailable".to_string())?;
        if is_link_or_reparse(&metadata) || !metadata.is_dir() {
            return Err("output path must be a non-reparse directory".to_string());
        }
        fs::canonicalize(output).map_err(|_| "output canonicalization failed".to_string())?
    } else {
        let parent = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| "output must have an existing parent directory".to_string())?;
        ensure_no_reparse_components(parent, "output parent")?;
        let canonical_parent = fs::canonicalize(parent)
            .map_err(|_| "output parent directory unavailable".to_string())?;
        if !canonical_parent.is_dir() {
            return Err("output parent is not a directory".to_string());
        }
        let name = output
            .file_name()
            .ok_or_else(|| "output path must name a directory".to_string())?;
        canonical_parent.join(name)
    };
    if resolved.starts_with(&source_root) || source_root.starts_with(&resolved) {
        return Err("output and source root must be disjoint directories".to_string());
    }
    Ok(resolved)
}

fn parse_options(args: impl Iterator<Item = String>) -> ToolResult<Option<Options>> {
    let mut source_root = None;
    let mut expected_custody = None;
    let mut output = None;
    let mut resume = false;
    let mut verify_only = false;
    let mut args = args;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--source-root" => {
                source_root = Some(PathBuf::from(
                    args.next().ok_or("--source-root requires a path")?,
                ))
            }
            "--expected-custody" => {
                expected_custody = Some(PathBuf::from(
                    args.next().ok_or("--expected-custody requires a path")?,
                ))
            }
            "--output" => {
                output = Some(PathBuf::from(
                    args.next().ok_or("--output requires a path")?,
                ))
            }
            "--resume" => resume = true,
            "--verify-only" => verify_only = true,
            "--help" | "-h" => return Ok(None),
            _ => return Err("unknown command-line argument".to_string()),
        }
    }
    if resume && verify_only {
        return Err("--resume and --verify-only cannot be combined".to_string());
    }
    Ok(Some(Options {
        source_root: source_root.ok_or("--source-root is required")?,
        expected_custody: expected_custody.ok_or("--expected-custody is required")?,
        output: output.ok_or("--output is required")?,
        resume,
        verify_only,
    }))
}

fn verify_existing_batch_set(
    existing: &ExistingOutput,
    sources: &[SourceFile],
    scan_results: &[ScanResult],
    output: &Path,
) -> ToolResult<Vec<Arc<File>>> {
    if sources.len() != scan_results.len() {
        return Err("source replay result count does not match custody".to_string());
    }
    let mut verified_guards = Vec::new();
    for (source, result) in sources.iter().zip(scan_results) {
        let actual = existing
            .batch_indexes
            .get(&source.source_id)
            .cloned()
            .unwrap_or_default();
        if actual.len() as u64 != result.batch_count
            || actual
                .iter()
                .copied()
                .enumerate()
                .any(|(expected, observed)| expected as u64 != observed)
            || result.batch_sha256s.len() as u64 != result.batch_count
        {
            return Err("resume output batch inventory does not match source replay".to_string());
        }
        let mut expected_predecessor = INITIAL_BATCH_PREDECESSOR.to_string();
        for (index, expected_digest) in result.batch_sha256s.iter().enumerate() {
            let path = batch_path(output, &source.source_id, index as u64);
            let (bytes, _, guard) =
                read_bounded_snapshot(&path, MAX_OUTPUT_ARTIFACT_BYTES, "stored batch")?;
            let stored: StoredBatch = serde_json::from_slice(&bytes)
                .map_err(|_| "stored batch is invalid JSON".to_string())?;
            let payload = &stored.payload;
            let canonical_payload = serde_json::to_vec(payload)
                .map_err(|_| "stored batch payload serialization failed".to_string())?;
            let canonical_stored = serialize_with_newline(&stored)?;
            if bytes != canonical_stored
                || payload.schema_id != BATCH_SCHEMA_ID
                || payload.reader_commit != BASE_READER_COMMIT
                || payload.reader_source_sha256 != reader_source_sha256()
                || payload.evidence_contract_sha256 != PG_C01_BUNDLE_SHA256
                || payload.previous_batch_sha256 != expected_predecessor
                || payload.source_id != source.source_id
                || payload.source_file_sha256 != source.sha256
                || canonical_u64(&payload.batch_index, "batch_index")? != index as u64
                || canonical_u64(&payload.frame_count, "frame_count")? as usize
                    != payload.frames.len()
                || sha256_bytes(&canonical_payload) != stored.batch_sha256
                || stored.batch_sha256 != *expected_digest
            {
                return Err("stored batch differs from replayed source witness".to_string());
            }
            expected_predecessor = stored.batch_sha256.clone();
            verified_guards.push(guard);
        }
    }
    if existing
        .batch_indexes
        .keys()
        .any(|source_id| !sources.iter().any(|source| source.source_id == *source_id))
    {
        return Err("resume output includes an unbound source id".to_string());
    }
    Ok(verified_guards)
}

fn semantic_groups_incomplete() -> Vec<SemanticGroupStatus> {
    SEMANTIC_GROUPS
        .iter()
        .map(|group| SemanticGroupStatus {
            group_id: (*group).to_string(),
            presence: "unclassified".to_string(),
            completeness: "incomplete".to_string(),
            row_count: None,
            reason: "No reviewed authentic carrier-to-family mapping; unknown is not absent."
                .to_string(),
        })
        .collect()
}

fn make_coverage(
    custody_sha256: &str,
    owner_count: usize,
    non_owner_file_count: u64,
    frame_count: u64,
    sources: Vec<SourceSummary>,
    inventory: &SourceTreeInventory,
) -> CoverageReport {
    CoverageReport {
        schema_id: "reliquary-f03-source-coverage-v1".to_string(),
        contract_id: "reliquary-corpus-evidence-v1".to_string(),
        status: "INCOMPLETE".to_string(),
        evidence_scope: "custody_bound_source_root_physical_inventory".to_string(),
        authentic_admission: false,
        custody_sha256: custody_sha256.to_string(),
        expected_owner_count: owner_count.to_string(),
        observed_owner_count: sources.len().to_string(),
        physical_frame_scan: "exhaustive-for-hash-verified-owner-files".to_string(),
        physical_frame_completeness: "exhaustive".to_string(),
        unclassified_frame_count: frame_count.to_string(),
        semantic_family_inventory: "not_started_authentic_registry_empty".to_string(),
        semantic_family_completeness: "incomplete".to_string(),
        cut_inventory: "not_started".to_string(),
        cut_completeness: "incomplete".to_string(),
        sidecar_discovery_scope: "source_root_recursive_only".to_string(),
        source_root_non_owner_file_count: non_owner_file_count.to_string(),
        source_root_inventory_entry_count: inventory.records.len().to_string(),
        source_root_inventory_sha256: inventory.sha256.clone(),
        external_sidecars: "unaudited_outside_source_root".to_string(),
        semantic_groups: semantic_groups_incomplete(),
        sources,
        reason: "Physical frame census is not a semantic family/cut census; every frame remains unclassified until source-derived mappings and typed historical readers are independently reviewed.".to_string(),
    }
}

fn embedded_source_hash(bytes: &[(&str, &[u8])]) -> String {
    let mut hash = Sha256::new();
    for (name, value) in bytes {
        hash.update(name.as_bytes());
        hash.update([0]);
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value);
    }
    lower_hex(&hash.finalize())
}

fn reader_source_sha256() -> String {
    embedded_source_hash(&[
        ("src/container/container.rs", EMBEDDED_CONTAINER_RS),
        (
            "src/container/container_error.rs",
            EMBEDDED_CONTAINER_ERROR_RS,
        ),
        (
            "src/container/container_scan.rs",
            EMBEDDED_CONTAINER_SCAN_RS,
        ),
        (
            "src/container/container_version.rs",
            EMBEDDED_CONTAINER_VERSION_RS,
        ),
    ])
}

fn current_executable_sha256() -> ToolResult<String> {
    let path =
        env::current_exe().map_err(|error| format!("executable path unavailable: {error}"))?;
    Ok(hash_file_snapshot(&path)?.sha256)
}

fn build_manifest(
    custody_sha256: &str,
    coverage_bytes: &[u8],
    sources: &[SourceSummary],
    inventory: &SourceTreeInventory,
) -> ToolResult<WitnessManifest> {
    let code_hash = embedded_source_hash(&[
        ("src/f03_source_witness.rs", EMBEDDED_MODULE),
        ("src/lib.rs", EMBEDDED_LIB),
        ("examples/f03_source_witness.rs", EMBEDDED_CLI),
        ("Cargo.toml", EMBEDDED_CARGO_TOML),
        ("Cargo.lock", EMBEDDED_CARGO_LOCK),
        ("src/container/container.rs", EMBEDDED_CONTAINER_RS),
        (
            "src/container/container_error.rs",
            EMBEDDED_CONTAINER_ERROR_RS,
        ),
        (
            "src/container/container_scan.rs",
            EMBEDDED_CONTAINER_SCAN_RS,
        ),
        (
            "src/container/container_version.rs",
            EMBEDDED_CONTAINER_VERSION_RS,
        ),
    ]);
    let patch_hash = embedded_source_hash(&[
        ("src/f03_source_witness.rs", EMBEDDED_MODULE),
        ("src/lib.rs", EMBEDDED_LIB),
        ("examples/f03_source_witness.rs", EMBEDDED_CLI),
        ("Cargo.toml", EMBEDDED_CARGO_TOML),
        ("src/container/container.rs", EMBEDDED_CONTAINER_RS),
        (
            "src/container/container_error.rs",
            EMBEDDED_CONTAINER_ERROR_RS,
        ),
        (
            "src/container/container_scan.rs",
            EMBEDDED_CONTAINER_SCAN_RS,
        ),
        (
            "src/container/container_version.rs",
            EMBEDDED_CONTAINER_VERSION_RS,
        ),
    ]);
    let source_summaries = sources
        .iter()
        .map(|source| ManifestSource {
            source_id: source.source_id.clone(),
            source_file_sha256: source.source_file_sha256.clone(),
            owner_kind: source.owner_kind.clone(),
            owner_uuid: source.owner_uuid.clone(),
            scope_binding: source.scope_binding.clone(),
            physical_frame_count: source.physical_frame_count.clone(),
            batch_count: source.batch_count.clone(),
            batch_stream_sha256: source.batch_stream_sha256.clone(),
            frame_stream_sha256: source.frame_stream_sha256.clone(),
        })
        .collect();
    Ok(WitnessManifest {
        schema_id: "reliquary-f03-source-witness-manifest-v1".to_string(),
        contract_id: "reliquary-corpus-evidence-v1".to_string(),
        status: "INCOMPLETE".to_string(),
        authentic_admission: false,
        base_reader_commit: BASE_READER_COMMIT.to_string(),
        pg_c01_commit: PG_C01_COMMIT.to_string(),
        pg_c01_bundle_sha256: PG_C01_BUNDLE_SHA256.to_string(),
        reader_source_sha256: reader_source_sha256(),
        producer_code_sha256: code_hash,
        validation_patch_sha256: patch_hash,
        producer_binary_sha256: current_executable_sha256()?,
        cargo_lock_sha256: sha256_bytes(EMBEDDED_CARGO_LOCK),
        custody_sha256: custody_sha256.to_string(),
        source_coverage_sha256: sha256_bytes(coverage_bytes),
        source_root_inventory_entry_count: inventory.records.len().to_string(),
        source_root_inventory_sha256: inventory.sha256.clone(),
        source_count: sources.len().to_string(),
        sources: source_summaries,
        output_scope: "private physical frame fingerprints; no raw payload bytes".to_string(),
        reason: "This is a physical carrier census only. Semantic family/cut coverage is not complete and authentic admission is false.".to_string(),
    })
}

fn verify_only(
    custody_path: &Path,
    custody: &CustodySnapshot,
    sources: &[SourceFile],
    source_root: &Path,
    inventory: &SourceTreeInventory,
) -> ToolResult<String> {
    let mut frame_count = 0_u64;
    for source in sources {
        let scanned = scan_source_with_stability(source, None, false)?;
        frame_count = frame_count
            .checked_add(
                scanned
                    .summary
                    .physical_frame_count
                    .parse::<u64>()
                    .map_err(|_| "invalid frame count".to_string())?,
            )
            .ok_or_else(|| "physical frame total overflow".to_string())?;
    }
    ensure_sources_stable(sources)?;
    let _final_inventory = verify_source_inventory(source_root, inventory)?;
    for guard in &inventory.directory_guards {
        ensure_directory_guard(guard)?;
    }
    ensure_custody_stable(custody_path, custody)?;
    Ok(format!(
        "{{\"status\":\"INCOMPLETE_CUSTODY_ONLY\",\"authentic_admission\":false,\"owner_count\":{},\"physical_frame_count\":{},\"custody_sha256\":\"{}\",\"semantic_coverage\":\"INCOMPLETE\"}}",
        sources.len(),
        frame_count,
        custody.sha256
    ))
}

fn run_export(options: Options) -> ToolResult<(String, bool)> {
    ensure_no_reparse_components(&options.source_root, "source root")?;
    let source_root = fs::canonicalize(&options.source_root)
        .map_err(|_| "source root unavailable".to_string())?;
    let custody = load_custody(&options.expected_custody)?;
    let (sources, non_owner_file_count, inventory) = resolve_sources(&source_root, &custody)?;
    if options.verify_only {
        return Ok((
            verify_only(
                &options.expected_custody,
                &custody,
                &sources,
                &source_root,
                &inventory,
            )?,
            true,
        ));
    }
    let output = resolve_output_path(&options.output, &source_root)?;
    let output_parent = output
        .parent()
        .ok_or_else(|| "output parent directory is unavailable".to_string())?;
    let output_ancestor_guards = open_output_ancestor_guards(output_parent)?;
    ensure_directory_guards(&output_ancestor_guards)?;
    let _existing = inspect_existing_output(&output, options.resume)?;
    if !options.resume {
        ensure_directory_guards(&output_ancestor_guards)?;
        fs::create_dir(&output).map_err(|_| "output directory could not be created".to_string())?;
    }
    let output_guard = open_directory_guard(&output, "output directory")?;
    let mut scan_results = Vec::with_capacity(sources.len());
    for source in &sources {
        ensure_directory_guards(&output_ancestor_guards)?;
        ensure_directory_guard(&output_guard)?;
        scan_results.push(scan_source_with_stability(
            source,
            Some(&output),
            options.resume,
        )?);
    }
    let completed_output = inspect_existing_output(&output, true)?;
    let batch_guards =
        verify_existing_batch_set(&completed_output, &sources, &scan_results, &output)?;
    let final_inventory_before_report = verify_source_inventory(&source_root, &inventory)?;
    ensure_sources_stable(&sources)?;
    for guard in &inventory.directory_guards {
        ensure_directory_guard(guard)?;
    }
    ensure_custody_stable(&options.expected_custody, &custody)?;
    let source_summaries: Vec<_> = scan_results
        .iter()
        .map(|scan| scan.summary.clone())
        .collect();
    let frame_count = scan_results.iter().try_fold(0_u64, |total, scan| {
        let count = scan
            .summary
            .physical_frame_count
            .parse::<u64>()
            .map_err(|_| "invalid physical frame count".to_string())?;
        total
            .checked_add(count)
            .ok_or_else(|| "physical frame total overflow".to_string())
    })?;
    let coverage = make_coverage(
        &custody.sha256,
        custody.document.owner_files.len(),
        non_owner_file_count,
        frame_count,
        source_summaries.clone(),
        &inventory,
    );
    ensure_directory_guard(&output_guard)?;
    let (coverage_bytes, coverage_guard) = save_immutable_json(
        &output.join("source-coverage.json"),
        &coverage,
        options.resume,
    )?;
    let manifest = build_manifest(
        &custody.sha256,
        &coverage_bytes,
        &source_summaries,
        &inventory,
    )?;
    ensure_directory_guard(&output_guard)?;
    let (manifest_bytes, manifest_guard) = save_immutable_json(
        &output.join("source-witness-manifest.json"),
        &manifest,
        options.resume,
    )?;
    let final_output = inspect_existing_output(&output, true)?;
    let final_batch_guards =
        verify_existing_batch_set(&final_output, &sources, &scan_results, &output)?;
    let final_coverage_guard = verify_immutable_output(
        &output.join("source-coverage.json"),
        &coverage_bytes,
        "source coverage report",
    )?;
    let final_manifest_guard = verify_immutable_output(
        &output.join("source-witness-manifest.json"),
        &manifest_bytes,
        "source witness manifest",
    )?;
    ensure_sources_stable(&sources)?;
    let _final_inventory_after_report = verify_source_inventory(&source_root, &inventory)?;
    for guard in &inventory.directory_guards {
        ensure_directory_guard(guard)?;
    }
    ensure_custody_stable(&options.expected_custody, &custody)?;
    ensure_directory_guards(&output_ancestor_guards)?;
    ensure_directory_guard(&output_guard)?;
    let _held_batch_guard_count = batch_guards.len() + final_batch_guards.len();
    let _inventory_guard_count = final_inventory_before_report._file_guards.len();
    let _held_report_guard_count = [
        &coverage_guard,
        &manifest_guard,
        &final_coverage_guard,
        &final_manifest_guard,
    ]
    .len();
    let json = serde_json::to_string(&coverage)
        .map_err(|_| "coverage report serialization failed".to_string())?;
    Ok((json, true))
}

/// Entry point used only by the feature-gated validation executable.
pub fn cli_main() -> i32 {
    let options = match parse_options(env::args().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!(
                "Usage: f03-source-witness --source-root <copies> --expected-custody <custody.json> --output <private-output> [--resume | --verify-only]"
            );
            return 0;
        }
        Err(error) => {
            eprintln!(
                "{{\"status\":\"FAIL\",\"error\":\"invalid_arguments\",\"detail\":{}}}",
                serde_json::to_string(&error)
                    .unwrap_or_else(|_| "\"argument parsing failed\"".to_string())
            );
            return 2;
        }
    };
    match run_export(options) {
        Ok((json, incomplete)) => {
            println!("{json}");
            if incomplete { 2 } else { 0 }
        }
        Err(error) => {
            eprintln!(
                "{{\"status\":\"FAIL\",\"error\":\"source_witness_failed\",\"detail\":{}}}",
                serde_json::to_string(&error)
                    .unwrap_or_else(|_| "\"source witness failed\"".to_string())
            );
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_fingerprint_is_deterministic_and_payload_is_not_serialized() {
        let payload = b"PRIVATE-SYNTHETIC-CONTENT-DO-NOT-EMIT";
        let mut frame = crate::container::PhysicalFrame {
            object: crate::container::ObjectRef::from_legacy_bytes([0; 16]),
            payload: payload.to_vec(),
            latest_global: 7,
            end: 64,
        };
        let observed = frame_observation("source-0001", &"a".repeat(64), 0, &frame);
        let encoded = serde_json::to_vec(&observed).unwrap();
        assert!(!String::from_utf8_lossy(&encoded).contains("PRIVATE-SYNTHETIC-CONTENT"));
        assert_eq!(observed.semantic_classification, "unclassified");
        assert_eq!(
            observed.prefix_candidate_sha256,
            sha256_bytes(&payload[..PREFIX_FINGERPRINT_BYTES])
        );
        frame.payload.clear();
    }

    #[test]
    fn semantic_coverage_never_claims_zero_rows_or_complete_cuts() {
        let groups = semantic_groups_incomplete();
        assert_eq!(groups.len(), 11);
        assert!(groups.iter().all(|group| {
            group.presence == "unclassified"
                && group.completeness == "incomplete"
                && group.row_count.is_none()
        }));
    }

    #[test]
    fn canonical_decimal_rejects_leading_zero_and_overflow() {
        assert_eq!(canonical_u64("0", "ordinal").unwrap(), 0);
        assert_eq!(canonical_u64("42", "ordinal").unwrap(), 42);
        assert!(canonical_u64("042", "ordinal").is_err());
        assert!(canonical_u64("18446744073709551616", "ordinal").is_err());
    }

    #[test]
    fn batch_fingerprint_detects_payload_or_membership_change() {
        let payload = BatchPayload {
            schema_id: "reliquary-f03-physical-frame-batch-v1".to_string(),
            reader_commit: BASE_READER_COMMIT.to_string(),
            reader_source_sha256: reader_source_sha256(),
            evidence_contract_sha256: PG_C01_BUNDLE_SHA256.to_string(),
            previous_batch_sha256: INITIAL_BATCH_PREDECESSOR.to_string(),
            source_id: "source-0001".to_string(),
            source_file_sha256: "a".repeat(64),
            batch_index: "0".to_string(),
            first_frame_ordinal: "0".to_string(),
            first_source_offset: "40".to_string(),
            end_source_offset: "48".to_string(),
            frame_count: "1".to_string(),
            frames: vec![],
        };
        let (bytes, digest) = encode_batch(payload.clone()).unwrap();
        let stored: StoredBatch = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(stored.batch_sha256, digest);
        assert_eq!(stored.payload, payload);
        let mut changed = stored.payload;
        changed.end_source_offset = "49".to_string();
        let changed_bytes = serde_json::to_vec(&changed).unwrap();
        assert_ne!(sha256_bytes(&changed_bytes), digest);
    }

    struct ScratchDir(PathBuf);

    impl ScratchDir {
        fn new() -> Self {
            let path = env::temp_dir().join(format!(
                "reliquary-f03-source-witness-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn synthetic_source(path: &Path, owner_uuid: [u8; 16], payload: &[u8]) -> SourceFile {
        let extension = path.extension().and_then(OsStr::to_str).unwrap();
        let (kind, scope, owner_kind, scope_label) = if extension.eq_ignore_ascii_case("phy") {
            (crate::FileKind::Phylactery, None, "PHY", "none")
        } else {
            (
                crate::FileKind::Reliquary,
                Some(crate::ReliquaryScopeKind::Project),
                "REL",
                "project",
            )
        };
        let identity = crate::ContainerIdentity {
            file_kind: kind,
            scope,
        };
        let mut writer =
            crate::Container::create_with_identity_and_uuid(path, identity, owner_uuid).unwrap();
        writer.append(payload).unwrap();
        writer.sync().unwrap();
        drop(writer);

        let read_guard = Arc::new(open_scoped_handle(path, false, "synthetic source").unwrap());
        let hashed = hash_file_snapshot(path).unwrap();
        assert_eq!(file_identity(&read_guard).unwrap(), hashed.file_identity);
        SourceFile {
            path: path.to_path_buf(),
            source_id: "source-0001".to_string(),
            expected: CustodyOwnerFile {
                owner_uuid: hex_uuid(&owner_uuid),
                owner_kind: owner_kind.to_string(),
                scope: scope_label.to_string(),
                source_file_sha256: hashed.sha256.clone(),
                source_byte_length: hashed.byte_length.to_string(),
                target_file_sha256: "b".repeat(64),
                target_byte_length: "0".to_string(),
            },
            sha256: hashed.sha256,
            byte_length: hashed.byte_length,
            modified_unix_ns: hashed.modified_unix_ns,
            file_identity: hashed.file_identity,
            _read_guard: read_guard,
        }
    }

    fn synthetic_custody(
        scratch: &ScratchDir,
        owner_files: Vec<CustodyOwnerFile>,
    ) -> CustodySnapshot {
        let path = scratch.0.join("synthetic-custody.fixture");
        fs::write(&path, b"synthetic custody bytes").unwrap();
        let (bytes, hashed, read_guard) =
            read_bounded_snapshot(&path, MAX_CUSTODY_BYTES, "synthetic custody").unwrap();
        assert_eq!(hashed.sha256, sha256_bytes(&bytes));
        CustodySnapshot {
            document: CustodyDocument {
                schema_id: "reliquary-corpus-physical-custody-v1".to_string(),
                contract_id: "reliquary-corpus-evidence-v1".to_string(),
                owner_files,
                _relocations: vec![],
            },
            sha256: hashed.sha256,
            byte_length: hashed.byte_length,
            modified_unix_ns: hashed.modified_unix_ns,
            file_identity: hashed.file_identity,
            _read_guard: read_guard,
        }
    }

    fn write_synthetic_custody(
        scratch: &ScratchDir,
        owner_files: Vec<CustodyOwnerFile>,
    ) -> PathBuf {
        let path = scratch.0.join("synthetic-custody.json");
        let document = CustodyDocument {
            schema_id: "reliquary-corpus-physical-custody-v1".to_string(),
            contract_id: "reliquary-corpus-evidence-v1".to_string(),
            owner_files,
            _relocations: vec![],
        };
        fs::write(&path, serialize_with_newline(&document).unwrap()).unwrap();
        path
    }

    fn synthetic_multiframe_source(path: &Path, owner_uuid: [u8; 16], count: usize) -> SourceFile {
        let identity = crate::ContainerIdentity {
            file_kind: crate::FileKind::Reliquary,
            scope: Some(crate::ReliquaryScopeKind::Project),
        };
        let mut writer =
            crate::Container::create_with_identity_and_uuid(path, identity, owner_uuid).unwrap();
        for index in 0..count {
            writer
                .append(format!("synthetic-frame-{index:04}").as_bytes())
                .unwrap();
        }
        writer.sync().unwrap();
        drop(writer);
        let read_guard = Arc::new(open_scoped_handle(path, false, "synthetic source").unwrap());
        let hashed = hash_file_snapshot(path).unwrap();
        SourceFile {
            path: path.to_path_buf(),
            source_id: "source-0001".to_string(),
            expected: CustodyOwnerFile {
                owner_uuid: hex_uuid(&owner_uuid),
                owner_kind: "REL".to_string(),
                scope: "project".to_string(),
                source_file_sha256: hashed.sha256.clone(),
                source_byte_length: hashed.byte_length.to_string(),
                target_file_sha256: "b".repeat(64),
                target_byte_length: "0".to_string(),
            },
            sha256: hashed.sha256,
            byte_length: hashed.byte_length,
            modified_unix_ns: hashed.modified_unix_ns,
            file_identity: hashed.file_identity,
            _read_guard: read_guard,
        }
    }

    fn prepared_multibatch_output(
        scratch: &ScratchDir,
        label: &str,
    ) -> (SourceFile, PathBuf, ScanResult) {
        let source = synthetic_multiframe_source(
            &scratch.0.join(format!("{label}.rel")),
            [0x74; 16],
            BATCH_FRAME_LIMIT + 2,
        );
        let output = scratch.0.join(format!("{label}-output"));
        fs::create_dir(&output).unwrap();
        let scan = scan_source_with_stability(&source, Some(&output), false).unwrap();
        assert_eq!(scan.batch_count, 2);
        (source, output, scan)
    }

    fn six_owner_synthetic_root(scratch: &ScratchDir) -> (PathBuf, Vec<CustodyOwnerFile>) {
        let root = scratch.0.join("six-owner-copies");
        fs::create_dir(&root).unwrap();
        let mut owners = Vec::new();
        for index in 0..6_u8 {
            let is_phy = index % 2 == 1;
            let extension = if is_phy { "phy" } else { "rel" };
            let path = root.join(format!("synthetic-owner-{index}.{extension}"));
            let source = synthetic_source(
                &path,
                [index.wrapping_add(1); 16],
                format!("private synthetic owner bytes {index}").as_bytes(),
            );
            owners.push(source.expected);
        }
        (root, owners)
    }

    #[test]
    fn physical_scan_is_read_only_and_preserves_synthetic_source() {
        let scratch = ScratchDir::new();
        let path = scratch.0.join("source.rel");
        let source = synthetic_source(&path, [0x21; 16], b"PRIVATE-SYNTHETIC-SOURCE-DO-NOT-EMIT");
        let before = hash_file_snapshot(&path).unwrap();

        let result = scan_source_with_stability(&source, None, false).unwrap();
        assert_eq!(result.summary.physical_frame_count, "1");
        assert_eq!(result.summary.classification, "unclassified");

        let mut read_only = crate::Container::open_read_only_for_scan(&path).unwrap();
        assert!(read_only.append(b"forbidden write").is_err());
        drop(read_only);

        let after = hash_file_snapshot(&path).unwrap();
        assert_eq!(after.sha256, before.sha256);
        assert_eq!(after.byte_length, before.byte_length);
        assert_eq!(after.modified_unix_ns, before.modified_unix_ns);
    }

    #[test]
    fn output_path_must_be_disjoint_from_source_tree() {
        let scratch = ScratchDir::new();
        let source_root = scratch.0.join("copies");
        fs::create_dir(&source_root).unwrap();

        let nested = source_root.join("output");
        assert!(resolve_output_path(&nested, &source_root).is_err());
        assert!(resolve_output_path(&scratch.0, &source_root).is_err());

        let sibling = scratch.0.join("private-output");
        let resolved = resolve_output_path(&sibling, &source_root).unwrap();
        assert_eq!(
            resolved.file_name().and_then(OsStr::to_str),
            Some("private-output")
        );
        assert!(!resolved.starts_with(fs::canonicalize(&source_root).unwrap()));
        assert!(!sibling.exists(), "path validation must not create outputs");
    }

    #[test]
    fn unknown_extra_owner_fails_closed_against_six_file_custody() {
        let scratch = ScratchDir::new();
        let root = scratch.0.join("copies");
        fs::create_dir(&root).unwrap();
        let mut owners = Vec::new();
        for index in 0..6_u8 {
            let is_phy = index % 2 == 1;
            let extension = if is_phy { "phy" } else { "rel" };
            let path = root.join(format!("known-{index}.{extension}"));
            let source = synthetic_source(&path, [index + 1; 16], b"synthetic record");
            owners.push(source.expected);
        }
        let custody = synthetic_custody(&scratch, owners);
        fs::write(root.join("unknown-extra.rel"), b"unbound synthetic carrier").unwrap();

        let error = resolve_sources(&root, &custody).unwrap_err();
        assert!(error.contains("owner-file count"));
        assert!(semantic_groups_incomplete().iter().all(|group| {
            group.presence == "unclassified"
                && group.completeness == "incomplete"
                && group.row_count.is_none()
        }));
    }

    #[test]
    fn source_inventory_rejects_late_sidecar_and_late_owner() {
        let scratch = ScratchDir::new();
        let (root, owners) = six_owner_synthetic_root(&scratch);
        let custody = synthetic_custody(&scratch, owners.clone());
        let (sources, _, inventory) = resolve_sources(&root, &custody).unwrap();
        assert_eq!(sources.len(), 6);

        let late_sidecar = root.join("late-sidecar.fixture");
        fs::write(&late_sidecar, b"synthetic sidecar only").unwrap();
        assert!(
            verify_source_inventory(&root, &inventory)
                .unwrap_err()
                .contains("membership or identity changed")
        );
        fs::remove_file(&late_sidecar).unwrap();

        let (_, _, refreshed_inventory) = resolve_sources(&root, &custody).unwrap();
        fs::write(root.join("late-owner.rel"), b"synthetic unbound owner").unwrap();
        assert!(
            verify_source_inventory(&root, &refreshed_inventory)
                .unwrap_err()
                .contains("membership or identity changed")
        );
    }

    #[test]
    fn source_inventory_rejects_hard_link_aliases() {
        let scratch = ScratchDir::new();
        let root = scratch.0.join("copies");
        fs::create_dir(&root).unwrap();
        let owner = synthetic_source(
            &root.join("one.rel"),
            [0x73; 16],
            b"synthetic hardlink data",
        );
        let owner_path = owner.path.clone();
        drop(owner);
        let alias = root.join("alias.fixture");
        fs::hard_link(&owner_path, &alias).unwrap();
        assert!(
            collect_source_inventory(&root)
                .unwrap_err()
                .contains("one hard link per file")
        );
    }

    #[test]
    fn source_inventory_rejects_external_hard_link_to_authoritative_file() {
        let scratch = ScratchDir::new();
        let root = scratch.0.join("copies");
        fs::create_dir(&root).unwrap();
        let source = synthetic_source(
            &root.join("copied-owner.rel"),
            [0x76; 16],
            b"synthetic outside-alias target",
        );
        let source_path = source.path.clone();
        drop(source);
        let outside_alias = scratch.0.join("outside-authoritative-original.rel");
        fs::hard_link(&source_path, &outside_alias).unwrap();
        assert!(
            collect_source_inventory(&root)
                .unwrap_err()
                .contains("one hard link per file")
        );
    }

    #[test]
    fn six_owner_synthetic_export_is_deterministic_and_incomplete() {
        let scratch = ScratchDir::new();
        let (root, owners) = six_owner_synthetic_root(&scratch);
        let custody_path = write_synthetic_custody(&scratch, owners);
        let output = scratch.0.join("private-output");
        let before = collect_source_inventory(&root).unwrap();

        let first = run_export(Options {
            source_root: root.clone(),
            expected_custody: custody_path.clone(),
            output: output.clone(),
            resume: false,
            verify_only: false,
        })
        .unwrap();
        assert!(first.1);
        assert!(first.0.contains("\"status\":\"INCOMPLETE\""));
        assert!(first.0.contains("\"authentic_admission\":false"));
        assert_eq!(
            before.sha256,
            collect_source_inventory(&root).unwrap().sha256
        );

        let coverage_bytes = fs::read(output.join("source-coverage.json")).unwrap();
        let coverage: CoverageReport = serde_json::from_slice(&coverage_bytes).unwrap();
        assert_eq!(coverage.sources.len(), 6);
        assert_eq!(
            coverage.evidence_scope,
            "custody_bound_source_root_physical_inventory"
        );
        assert_eq!(coverage.source_root_inventory_sha256, before.sha256);
        assert!(
            coverage
                .sources
                .iter()
                .all(|summary| summary.classification == "unclassified")
        );
        for entry in fs::read_dir(&output).unwrap() {
            let entry = entry.unwrap();
            let bytes = fs::read(entry.path()).unwrap();
            assert!(!String::from_utf8_lossy(&bytes).contains("private synthetic owner bytes"));
        }
        let first_batch_hashes = batch_artifact_hashes(&output);

        let replay = run_export(Options {
            source_root: root.clone(),
            expected_custody: custody_path,
            output: output.clone(),
            resume: true,
            verify_only: false,
        })
        .unwrap();
        assert!(replay.1);
        assert_eq!(first.0, replay.0);
        assert_eq!(first_batch_hashes, batch_artifact_hashes(&output));
    }

    fn batch_artifact_hashes(output: &Path) -> BTreeMap<String, String> {
        fs::read_dir(output)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                let name = entry.file_name().into_string().unwrap();
                if parse_batch_filename(&name).is_some() {
                    (name, sha256_bytes(&fs::read(entry.path()).unwrap()))
                } else {
                    (String::new(), String::new())
                }
            })
            .filter(|(name, _)| !name.is_empty())
            .collect()
    }

    #[test]
    fn resume_replays_and_rejects_tampered_synthetic_batch() {
        let scratch = ScratchDir::new();
        let source_path = scratch.0.join("source.rel");
        let source = synthetic_source(&source_path, [0x31; 16], b"resume synthetic payload");
        let output = scratch.0.join("witness-output");
        fs::create_dir(&output).unwrap();

        let first = scan_source_with_stability(&source, Some(&output), false).unwrap();
        let existing = inspect_existing_output(&output, true).unwrap();
        verify_existing_batch_set(
            &existing,
            std::slice::from_ref(&source),
            std::slice::from_ref(&first),
            &output,
        )
        .unwrap();
        let replay = scan_source_with_stability(&source, Some(&output), true).unwrap();
        assert_eq!(first.summary, replay.summary);

        let batch = batch_path(&output, &source.source_id, 0);
        let mut tampered = fs::read(&batch).unwrap();
        tampered.push(b' ');
        fs::write(&batch, tampered).unwrap();
        assert!(scan_source_with_stability(&source, Some(&output), true).is_err());
    }

    #[test]
    fn verified_batch_guards_and_final_replay_detect_stored_output_mutation() {
        let scratch = ScratchDir::new();
        let source_path = scratch.0.join("guarded-source.rel");
        let source = synthetic_source(&source_path, [0x75; 16], b"guard synthetic payload");
        let output = scratch.0.join("guarded-output");
        fs::create_dir(&output).unwrap();
        let scan = scan_source_with_stability(&source, Some(&output), false).unwrap();
        let existing = inspect_existing_output(&output, true).unwrap();
        let guards = verify_existing_batch_set(
            &existing,
            std::slice::from_ref(&source),
            std::slice::from_ref(&scan),
            &output,
        )
        .unwrap();
        assert_eq!(guards.len(), 1);

        let batch = batch_path(&output, &source.source_id, 0);
        #[cfg(windows)]
        assert!(fs::write(&batch, b"guarded mutation").is_err());
        drop(guards);
        let mut bytes = fs::read(&batch).unwrap();
        bytes.push(b' ');
        fs::write(&batch, bytes).unwrap();
        assert!(
            verify_existing_batch_set(
                &inspect_existing_output(&output, true).unwrap(),
                std::slice::from_ref(&source),
                std::slice::from_ref(&scan),
                &output,
            )
            .is_err()
        );
    }

    #[test]
    fn final_report_guard_rejects_post_save_mutation() {
        let scratch = ScratchDir::new();
        let path = scratch.0.join("source-coverage.json");
        let report = serde_json::json!({
            "status": "INCOMPLETE",
            "authentic_admission": false,
            "synthetic_marker": "private synthetic report content"
        });
        let (expected_bytes, guard) = save_immutable_json(&path, &report, false).unwrap();
        let final_guard =
            verify_immutable_output(&path, &expected_bytes, "synthetic report").unwrap();
        assert_eq!(
            final_guard.metadata().unwrap().len(),
            expected_bytes.len() as u64
        );
        #[cfg(windows)]
        assert!(fs::write(&path, b"mutation while held").is_err());
        drop((guard, final_guard));

        fs::write(&path, b"{\"status\":\"TAMPERED\"}\n").unwrap();
        assert!(verify_immutable_output(&path, &expected_bytes, "synthetic report").is_err());
    }

    #[test]
    fn multi_batch_resume_rejects_gaps_partial_crashes_and_foreign_sources() {
        let gap_scratch = ScratchDir::new();
        let (gap_source, gap_output, gap_scan) =
            prepared_multibatch_output(&gap_scratch, "gap-case");
        fs::remove_file(batch_path(&gap_output, &gap_source.source_id, 0)).unwrap();
        assert!(
            inspect_existing_output(&gap_output, true)
                .unwrap_err()
                .contains("batch gap")
        );
        assert_eq!(gap_scan.batch_count, 2);

        let partial_scratch = ScratchDir::new();
        let (partial_source, partial_output, partial_scan) =
            prepared_multibatch_output(&partial_scratch, "partial-case");
        fs::write(
            batch_path(&partial_output, &partial_source.source_id, 1),
            b"{\"payload\":",
        )
        .unwrap();
        let partial_inventory = inspect_existing_output(&partial_output, true).unwrap();
        assert!(
            verify_existing_batch_set(
                &partial_inventory,
                std::slice::from_ref(&partial_source),
                std::slice::from_ref(&partial_scan),
                &partial_output,
            )
            .unwrap_err()
            .contains("invalid JSON")
        );

        let foreign_scratch = ScratchDir::new();
        let (foreign_source, foreign_output, foreign_scan) =
            prepared_multibatch_output(&foreign_scratch, "foreign-case");
        let foreign_path = batch_path(&foreign_output, &foreign_source.source_id, 1);
        let bytes = fs::read(&foreign_path).unwrap();
        let mut stored: StoredBatch = serde_json::from_slice(&bytes).unwrap();
        stored.payload.source_id = "source-9999".to_string();
        let canonical_payload = serde_json::to_vec(&stored.payload).unwrap();
        stored.batch_sha256 = sha256_bytes(&canonical_payload);
        fs::write(&foreign_path, serialize_with_newline(&stored).unwrap()).unwrap();
        assert!(
            verify_existing_batch_set(
                &inspect_existing_output(&foreign_output, true).unwrap(),
                std::slice::from_ref(&foreign_source),
                std::slice::from_ref(&foreign_scan),
                &foreign_output,
            )
            .unwrap_err()
            .contains("differs from replayed source witness")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_read_guards_deny_source_custody_writes_and_rename() {
        let scratch = ScratchDir::new();
        let source = scratch.0.join("source.rel");
        fs::write(&source, b"synthetic source").unwrap();
        let custody = scratch.0.join("custody.json");
        fs::write(&custody, b"synthetic custody").unwrap();
        let source_guard = open_scoped_handle(&source, false, "synthetic source").unwrap();
        let custody_guard = open_scoped_handle(&custody, false, "synthetic custody").unwrap();
        assert!(OpenOptions::new().write(true).open(&source).is_err());
        assert!(OpenOptions::new().write(true).open(&custody).is_err());
        assert!(fs::rename(&source, scratch.0.join("renamed-source.rel")).is_err());
        assert!(fs::rename(&custody, scratch.0.join("renamed-custody.json")).is_err());
        drop((source_guard, custody_guard));
    }

    #[cfg(windows)]
    #[test]
    fn windows_output_parent_and_grandparent_guards_block_replacement() {
        let scratch = ScratchDir::new();
        let grandparent = scratch.0.join("grandparent");
        let parent = grandparent.join("parent");
        let output = parent.join("output");
        fs::create_dir_all(&output).unwrap();
        let ancestor_guards = open_output_ancestor_guards(&parent).unwrap();
        assert_eq!(ancestor_guards.len(), 2);
        let output_guard = open_directory_guard(&output, "synthetic output").unwrap();
        assert!(fs::rename(&parent, grandparent.join("replacement-parent")).is_err());
        assert!(fs::rename(&grandparent, scratch.0.join("replacement-grandparent")).is_err());
        ensure_directory_guards(&ancestor_guards).unwrap();
        ensure_directory_guard(&output_guard).unwrap();
    }

    #[test]
    fn reader_rejects_a_different_file_identity_even_with_same_owner_and_size() {
        let scratch = ScratchDir::new();
        let original_path = scratch.0.join("original.rel");
        let alternate_path = scratch.0.join("alternate.rel");
        let original = synthetic_source(&original_path, [0x42; 16], b"same-length payload");
        let mut alternate = synthetic_source(&alternate_path, [0x42; 16], b"same-length payload");
        assert_ne!(original.file_identity, alternate.file_identity);

        alternate.sha256 = original.sha256.clone();
        alternate.byte_length = original.byte_length;
        alternate.modified_unix_ns = original.modified_unix_ns.clone();
        alternate.file_identity = original.file_identity;
        let error = scan_source_frames(
            &alternate,
            original.byte_length,
            original.file_identity,
            None,
            false,
        )
        .unwrap_err();
        assert!(error.contains("does not match the custody-hashed file"));
    }

    #[test]
    fn bounded_clock_state_rejects_large_synthetic_group_without_expansion() {
        let scratch = ScratchDir::new();
        let path = scratch.0.join("large-clock.rel");
        let identity = crate::ContainerIdentity {
            file_kind: crate::FileKind::Reliquary,
            scope: Some(crate::ReliquaryScopeKind::Project),
        };
        let mut writer =
            crate::Container::create_with_identity_and_uuid(&path, identity, [0x51; 16]).unwrap();
        let mut group = vec![0_u8; 84];
        group[..8].copy_from_slice(b"CVAINSC7");
        group[64..72].copy_from_slice(&1_u64.to_le_bytes());
        group[72..76].copy_from_slice(&u32::MAX.to_le_bytes());
        group[76..84].copy_from_slice(&0_i64.to_le_bytes());
        writer.append(&group).unwrap();
        writer.sync().unwrap();
        drop(writer);

        let reader = crate::Container::open_read_only_for_scan(&path).unwrap();
        let mut cursor = reader.physical_cursor_for_source_witness(1024, 1).unwrap();
        assert!(matches!(
            cursor.next_frame(),
            Err(crate::ContainerError::ScanResourceLimitExceeded)
        ));
    }

    #[test]
    fn bounded_custody_read_hashes_exact_bytes_and_hides_parse_details() {
        let scratch = ScratchDir::new();
        let path = scratch.0.join("custody.json");
        let bytes = b"{\"schema_id\":\"private-parse-sentinel\"}";
        fs::write(&path, bytes).unwrap();
        let (observed, snapshot, guard) =
            read_bounded_snapshot(&path, 1024, "synthetic custody").unwrap();
        assert_eq!(observed, bytes);
        assert_eq!(snapshot.sha256, sha256_bytes(&observed));
        drop(guard);

        let error = load_custody(&path).unwrap_err();
        assert_eq!(error, "expected custody is invalid strict JSON");
        assert!(!error.contains("private-parse-sentinel"));
    }

    #[test]
    fn bounded_source_tree_stops_at_depth_limit() {
        let scratch = ScratchDir::new();
        let root = scratch.0.join("copies");
        fs::create_dir(&root).unwrap();
        let mut current = root.clone();
        for index in 0..(MAX_SOURCE_TREE_DEPTH + 2) {
            current = current.join(format!("level-{index:02}"));
            fs::create_dir(&current).unwrap();
        }
        let custody = synthetic_custody(&scratch, vec![]);
        let error = resolve_sources(&root, &custody).unwrap_err();
        assert!(error.contains("nesting exceeds the depth limit"));
    }

    #[test]
    fn bounded_resume_reader_rejects_oversized_artifact_before_parse() {
        let scratch = ScratchDir::new();
        let path = scratch.0.join("oversized.json");
        fs::write(&path, vec![b'X'; 32]).unwrap();
        assert!(read_bounded_snapshot(&path, 16, "synthetic batch").is_err());
    }

    #[test]
    fn serialized_batch_never_contains_synthetic_payload_bytes() {
        let secret = b"PRIVATE-BATCH-PAYLOAD-DO-NOT-EMIT";
        let frame = crate::container::PhysicalFrame {
            object: crate::container::ObjectRef::from_legacy_bytes([0; 16]),
            payload: secret.to_vec(),
            latest_global: 4,
            end: 96,
        };
        let observation = frame_observation("source-0001", &"c".repeat(64), 0, &frame);
        let payload = BatchPayload {
            schema_id: "reliquary-f03-physical-frame-batch-v1".to_string(),
            reader_commit: BASE_READER_COMMIT.to_string(),
            reader_source_sha256: reader_source_sha256(),
            evidence_contract_sha256: PG_C01_BUNDLE_SHA256.to_string(),
            previous_batch_sha256: INITIAL_BATCH_PREDECESSOR.to_string(),
            source_id: "source-0001".to_string(),
            source_file_sha256: "c".repeat(64),
            batch_index: "0".to_string(),
            first_frame_ordinal: "0".to_string(),
            first_source_offset: "40".to_string(),
            end_source_offset: "96".to_string(),
            frame_count: "1".to_string(),
            frames: vec![observation],
        };
        let (encoded, _) = encode_batch(payload).unwrap();
        assert!(!String::from_utf8_lossy(&encoded).contains("PRIVATE-BATCH-PAYLOAD-DO-NOT-EMIT"));
    }
}
