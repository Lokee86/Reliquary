//! Bounded header-only observation of V01-hash-bound disposable copies.
//!
//! This tool does not read frames or semantic payloads. It observes only the
//! native Container header after verifying each copy against the reviewed V01
//! hash/length table. It is not an expected-custody document or admission proof.

use crate::{Container, ContainerIdentity, FileKind, ReliquaryScopeKind};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const HASH_BUFFER_BYTES: usize = 64 * 1024;
const REPORT_NAME: &str = "identity-observation.json";
const MAX_REPORT_BYTES: usize = 64 * 1024;

pub(crate) const V01_SOURCE_COMMIT: &str = "c6010fd30dd52acb81c667f7aed99b831307e49f";
pub(crate) const V01_PLANNING_COMMIT: &str = "deb6ce6ee7dc94dd97e2fa58d3e910541240b9dd";
pub(crate) const V01_PLANNING_RECORD_BLOB_SHA1: &str = "47a0f58da7a4011ed39ed59d29399c7492410652";
pub(crate) const V01_COPY_PROOF_SHA256: &str =
    "6568bb11fe9993d59f0129bfbe3613676de24442fba263ec2387741e11a475fd";
pub(crate) const V01_CUSTODY_RECEIPT_SHA256: &str =
    "bd2c6bee9966a3e62bdd1deb6a0e29e34f74cc13bad622f7898c151084153c5d";
pub(crate) const V01_EXPECTED_COPY_TABLE_SHA256: &str =
    "51731c7965b029cc26ff4d15e705f9cfbb0ca37e1f9cd29007e08ed2604d333d";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExpectedCopy {
    pub(crate) relative_path: &'static str,
    pub(crate) sha256: &'static str,
    pub(crate) byte_length: u64,
}

// These paths, hashes, and lengths are pinned from the reviewed V01 copy-proof
// test and custody receipt. This table carries no owner UUID/kind/scope claim.
pub(crate) const V01_EXPECTED_COPIES: [ExpectedCopy; 6] = [
    ExpectedCopy {
        relative_path: "chatgpt-first14d/project.prj.rel",
        sha256: "a02c8696db754c53a73c896133b06df9828a32923aee610c9f918a16698f76d0",
        byte_length: 29_222_523,
    },
    ExpectedCopy {
        relative_path: "chatgpt-first14d/user.phy",
        sha256: "7e690b4853afe0936702dc023f511dd7c78ffaae3e18e8e66f6ab1c6891a6c63",
        byte_length: 675_300,
    },
    ExpectedCopy {
        relative_path: "chatgpt-first28d/project.prj.rel",
        sha256: "76c15780bd7cfe7e3083dbff5684e236a8fd2090fb0a82ec3d66529d14307d87",
        byte_length: 72_905_208,
    },
    ExpectedCopy {
        relative_path: "chatgpt-first28d/user.phy",
        sha256: "32970f3975bfcd6de0fcdb78df9c80c78b76787832f528c1b09fd75dfa4f501d",
        byte_length: 1_507_220,
    },
    ExpectedCopy {
        relative_path: "rhelm-david-r-ellis-pre-relationship/project.prj.rel",
        sha256: "cbc38d055936fc5702a17c4d569557d8c35e8887d1946c5562ba419038de6ee3",
        byte_length: 17_174_285,
    },
    ExpectedCopy {
        relative_path: "rhelm-david-r-ellis-pre-relationship/user.phy",
        sha256: "58ac7872215e126749f024fc742828abc9a8a8f84775b39c0706d638392ce9e0",
        byte_length: 3_180_063,
    },
];

type ToolResult<T> = Result<T, String>;
type FileIdentity = (u64, u64);

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileSnapshot {
    sha256: String,
    byte_length: u64,
    modified_unix_ns: String,
    file_identity: FileIdentity,
    physical_link_count: u64,
}

struct BoundCopy {
    source_id: String,
    expected: ExpectedCopy,
    path: PathBuf,
    file: File,
    snapshot: FileSnapshot,
}

struct DirectoryGuard {
    path: PathBuf,
    file_identity: FileIdentity,
    file: File,
}

struct SourceTree {
    canonical_root: PathBuf,
    directories: Vec<DirectoryGuard>,
    copies: Vec<BoundCopy>,
}

struct ReportGuard {
    output_path: PathBuf,
    report_path: PathBuf,
    output_directory: DirectoryGuard,
    output_ancestors: Vec<DirectoryGuard>,
    file: File,
    file_identity: FileIdentity,
    expected_bytes: Vec<u8>,
}

#[derive(Serialize)]
struct BindingProvenance {
    source_commit: &'static str,
    planning_commit: &'static str,
    planning_record_blob_sha1: &'static str,
    v01_copy_proof_sha256: &'static str,
    v01_custody_receipt_sha256: &'static str,
}

#[derive(Serialize)]
struct PhysicalFileIdentity {
    volume_or_device_hex: String,
    file_index_or_inode_hex: String,
}

#[derive(Serialize)]
struct HeaderObservation {
    identity_header_present: bool,
    owner_kind_observed_from_header: Option<String>,
    owner_uuid_hex_observed_from_header: Option<String>,
    owner_uuid_observation_state: &'static str,
    native_scope_observed_from_header: Option<String>,
    scope_observation_state: &'static str,
    values_independently_authenticated: bool,
}

#[derive(Serialize)]
struct CopyObservation {
    source_id: String,
    v01_copy_sha256: String,
    v01_copy_byte_length: String,
    observed_file_sha256: String,
    observed_file_byte_length: String,
    observed_file_modified_unix_ns: String,
    physical_file_identity: PhysicalFileIdentity,
    physical_link_count: String,
    format_major: String,
    format_minor: String,
    header_length: String,
    header: HeaderObservation,
}

#[derive(Serialize)]
struct IdentityObservationReport {
    schema_id: &'static str,
    status: &'static str,
    evidence_scope: &'static str,
    authentic_admission: bool,
    semantic_history_assessed: bool,
    owner_identity_independently_authenticated: bool,
    scope_independently_authenticated: bool,
    binding_provenance: BindingProvenance,
    expected_copy_table_sha256: String,
    source_set_sha256: String,
    expected_copy_count: String,
    observed_copy_count: String,
    observations: Vec<CopyObservation>,
}

struct Options {
    source_root: PathBuf,
    output: PathBuf,
}

pub(crate) fn v01_specs() -> Vec<ExpectedCopy> {
    let mut copies = V01_EXPECTED_COPIES.to_vec();
    copies.sort_by(|left, right| {
        left.relative_path
            .as_bytes()
            .cmp(right.relative_path.as_bytes())
    });
    copies
}

pub(crate) fn expected_table_digest(specs: &[ExpectedCopy]) -> String {
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
    hex(&hasher.finalize())
}

fn valid_relative_path(value: &str) -> bool {
    let mut parts = value.split('/');
    let (Some(parent), Some(file)) = (parts.next(), parts.next()) else {
        return false;
    };
    parts.next().is_none()
        && !parent.is_empty()
        && !file.is_empty()
        && parent != "."
        && parent != ".."
        && file != "."
        && file != ".."
        && !parent.contains('\\')
        && !parent.contains(':')
        && !file.contains('\\')
        && !file.contains(':')
}

fn expected_layout(specs: &[ExpectedCopy]) -> ToolResult<BTreeMap<String, BTreeSet<String>>> {
    if specs.is_empty() || specs.len() > 6 {
        return Err("expected copy count is outside the supported bound".to_string());
    }
    let mut layout = BTreeMap::<String, BTreeSet<String>>::new();
    let mut paths = BTreeSet::new();
    let mut hashes = BTreeSet::new();
    let mut total_bytes = 0_u64;
    for spec in specs {
        if !valid_relative_path(spec.relative_path)
            || spec.sha256.len() != 64
            || !spec
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || spec.byte_length == 0
            || spec.byte_length > MAX_FILE_BYTES
            || !paths.insert(spec.relative_path)
            || !hashes.insert(spec.sha256)
        {
            return Err("expected copy table is invalid".to_string());
        }
        total_bytes = total_bytes
            .checked_add(spec.byte_length)
            .ok_or_else(|| "expected copy bytes overflow".to_string())?;
        if total_bytes > MAX_TOTAL_BYTES {
            return Err("expected copy total exceeds the scan bound".to_string());
        }
        let (parent, file) = spec
            .relative_path
            .split_once('/')
            .ok_or_else(|| "expected relative path is invalid".to_string())?;
        layout
            .entry(parent.to_string())
            .or_default()
            .insert(file.to_string());
    }
    Ok(layout)
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn ensure_no_reparse_components(path: &Path) -> ToolResult<()> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|_| "current directory is unavailable".to_string())?
            .join(path)
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err("path contains a parent traversal component".to_string());
            }
            Component::Normal(part) => {
                current.push(part);
                let metadata = fs::symlink_metadata(&current)
                    .map_err(|_| "path component is unavailable".to_string())?;
                if is_link_or_reparse(&metadata) {
                    return Err("path contains a symlink or reparse point".to_string());
                }
            }
        }
    }
    if current.as_os_str().is_empty() {
        return Err("path is empty".to_string());
    }
    Ok(())
}

fn raw_open(path: &Path, directory: bool) -> ToolResult<File> {
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
    options
        .open(path)
        .map_err(|_| "read-only path handle could not be opened".to_string())
}

fn file_identity(file: &File) -> ToolResult<FileIdentity> {
    crate::Container::identity_for_file(file)
        .map_err(|_| "physical file identity is unavailable".to_string())?
        .ok_or_else(|| "physical file identity is unsupported".to_string())
}

fn open_scoped_handle(path: &Path, directory: bool) -> ToolResult<File> {
    ensure_no_reparse_components(path)?;
    let before = fs::symlink_metadata(path)
        .map_err(|_| "scoped path metadata is unavailable".to_string())?;
    if is_link_or_reparse(&before)
        || (directory && !before.is_dir())
        || (!directory && !before.is_file())
    {
        return Err("scoped path is not a regular non-reparse entry".to_string());
    }
    let file = raw_open(path, directory)?;
    let handle_metadata = file
        .metadata()
        .map_err(|_| "scoped handle metadata is unavailable".to_string())?;
    if (directory && !handle_metadata.is_dir()) || (!directory && !handle_metadata.is_file()) {
        return Err("scoped handle type changed while opening".to_string());
    }
    let identity = file_identity(&file)?;
    let after =
        fs::symlink_metadata(path).map_err(|_| "scoped path changed while opening".to_string())?;
    if is_link_or_reparse(&after) {
        return Err("scoped path became a reparse point".to_string());
    }
    let verification = raw_open(path, directory)?;
    if file_identity(&verification)? != identity {
        return Err("scoped path identity changed while opening".to_string());
    }
    Ok(file)
}

fn open_output_verification_handle(path: &Path) -> ToolResult<File> {
    ensure_no_reparse_components(path)?;
    let before = fs::symlink_metadata(path)
        .map_err(|_| "identity report path metadata is unavailable".to_string())?;
    if is_link_or_reparse(&before) || !before.is_file() {
        return Err("identity report path is not a regular file".to_string());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // The retained report handle requests read/write and shares read only.
        // This secondary read handle must share both existing access modes.
        options.share_mode(0x0000_0001 | 0x0000_0002);
    }
    let file = options
        .open(path)
        .map_err(|_| "identity report path could not be reopened for verification".to_string())?;
    let handle_metadata = file
        .metadata()
        .map_err(|_| "identity report handle metadata is unavailable".to_string())?;
    let after = fs::symlink_metadata(path)
        .map_err(|_| "identity report path changed during verification".to_string())?;
    if !handle_metadata.is_file()
        || is_link_or_reparse(&after)
        || !after.is_file()
        || before.len() != handle_metadata.len()
        || after.len() != handle_metadata.len()
    {
        return Err("identity report path identity is unstable".to_string());
    }
    Ok(file)
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
        // SAFETY: file owns a live handle and the operating system fills the structure.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) }
            == 0
        {
            return Err("physical hard-link count is unavailable".to_string());
        }
        // SAFETY: successful GetFileInformationByHandle initializes every field.
        let information = unsafe { information.assume_init() };
        Ok(u64::from(information.links))
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        file.metadata()
            .map(|metadata| metadata.nlink())
            .map_err(|_| "physical hard-link count is unavailable".to_string())
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = file;
        Err("physical hard-link count is unsupported".to_string())
    }
}

fn modified_unix_ns(metadata: &fs::Metadata) -> ToolResult<String> {
    let modified = metadata
        .modified()
        .map_err(|_| "file modification time is unavailable".to_string())?;
    let elapsed = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "pre-epoch modification time is unsupported".to_string())?;
    Ok(elapsed.as_nanos().to_string())
}

fn hash_held_file(path: &Path, file: &mut File) -> ToolResult<FileSnapshot> {
    let before = file
        .metadata()
        .map_err(|_| "held source metadata is unavailable".to_string())?;
    if !before.is_file() || before.len() > MAX_FILE_BYTES {
        return Err("held source is outside the bounded file limit".to_string());
    }
    let identity = file_identity(file)?;
    let modified = modified_unix_ns(&before)?;
    let length = before.len();
    let link_count = physical_link_count(file)?;
    if link_count != 1 {
        return Err("source copy must have exactly one physical hard link".to_string());
    }

    file.seek(SeekFrom::Start(0))
        .map_err(|_| "held source could not seek for hashing".to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| "held source read failed".to_string())?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| "source byte count overflow".to_string())?;
        if total > MAX_FILE_BYTES {
            return Err("source copy exceeds the bounded file limit".to_string());
        }
        hasher.update(&buffer[..count]);
    }

    let after = file
        .metadata()
        .map_err(|_| "held source metadata reread failed".to_string())?;
    let path_metadata =
        fs::symlink_metadata(path).map_err(|_| "source path changed during hashing".to_string())?;
    if is_link_or_reparse(&path_metadata) || !path_metadata.is_file() {
        return Err("source path became unsafe during hashing".to_string());
    }
    let path_handle = open_scoped_handle(path, false)?;
    let path_after = path_handle
        .metadata()
        .map_err(|_| "source path metadata reread failed".to_string())?;
    if file_identity(file)? != identity
        || file_identity(&path_handle)? != identity
        || total != length
        || after.len() != length
        || path_after.len() != length
        || modified_unix_ns(&after)? != modified
        || modified_unix_ns(&path_after)? != modified
        || physical_link_count(file)? != 1
    {
        return Err("source changed during identity-bound hashing".to_string());
    }
    Ok(FileSnapshot {
        sha256: hex(&hasher.finalize()),
        byte_length: total,
        modified_unix_ns: modified,
        file_identity: identity,
        physical_link_count: link_count,
    })
}

fn open_directory_guard(path: &Path) -> ToolResult<DirectoryGuard> {
    let file = open_scoped_handle(path, true)?;
    Ok(DirectoryGuard {
        path: path.to_path_buf(),
        file_identity: file_identity(&file)?,
        file,
    })
}

fn ensure_directory_guard(guard: &DirectoryGuard) -> ToolResult<()> {
    if file_identity(&guard.file)? != guard.file_identity {
        return Err("held directory identity changed".to_string());
    }
    let path_file = open_scoped_handle(&guard.path, true)?;
    if file_identity(&path_file)? != guard.file_identity {
        return Err("directory path no longer names the held directory".to_string());
    }
    Ok(())
}

fn enumerate_exact(
    directory: &Path,
    expected_names: &BTreeSet<String>,
    expect_directories: bool,
) -> ToolResult<()> {
    let mut observed = BTreeSet::new();
    for entry in fs::read_dir(directory)
        .map_err(|_| "source directory could not be enumerated".to_string())?
    {
        let entry = entry.map_err(|_| "source directory entry is unavailable".to_string())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "source tree contains a non-UTF-8 name".to_string())?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| "source entry metadata is unavailable".to_string())?;
        if is_link_or_reparse(&metadata)
            || (expect_directories && !metadata.is_dir())
            || (!expect_directories && !metadata.is_file())
        {
            return Err("source tree contains an unsafe or unexpected entry".to_string());
        }
        if !expected_names.contains(&name) || !observed.insert(name) {
            return Err("source tree membership differs from the V01 copy set".to_string());
        }
    }
    if &observed != expected_names {
        return Err("source tree is missing an expected V01 copy entry".to_string());
    }
    Ok(())
}

fn source_path(root: &Path, relative_path: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for component in relative_path.split('/') {
        path.push(component);
    }
    path
}

fn collect_source_tree(root: &Path, expected: &[ExpectedCopy]) -> ToolResult<SourceTree> {
    let layout = expected_layout(expected)?;
    ensure_no_reparse_components(root)?;
    let canonical_root =
        fs::canonicalize(root).map_err(|_| "source root could not be canonicalized".to_string())?;
    let root_metadata = fs::symlink_metadata(&canonical_root)
        .map_err(|_| "source root metadata is unavailable".to_string())?;
    if is_link_or_reparse(&root_metadata) || !root_metadata.is_dir() {
        return Err("source root is not a regular directory".to_string());
    }

    let root_guard = open_directory_guard(&canonical_root)?;
    let expected_directories = layout.keys().cloned().collect::<BTreeSet<_>>();
    enumerate_exact(&canonical_root, &expected_directories, true)?;
    let mut directories = vec![root_guard];
    let mut copies = Vec::with_capacity(expected.len());
    let mut identities = BTreeSet::new();
    let mut total_bytes = 0_u64;
    let mut ordered = expected.to_vec();
    ordered.sort_by(|left, right| {
        left.relative_path
            .as_bytes()
            .cmp(right.relative_path.as_bytes())
    });

    for (index, spec) in ordered.iter().enumerate() {
        let (parent, _) = spec
            .relative_path
            .split_once('/')
            .ok_or_else(|| "expected relative path is invalid".to_string())?;
        let directory = canonical_root.join(parent);
        let directory_meta = fs::symlink_metadata(&directory)
            .map_err(|_| "source directory is unavailable".to_string())?;
        if is_link_or_reparse(&directory_meta) || !directory_meta.is_dir() {
            return Err("source subdirectory is not a regular directory".to_string());
        }
        if !directories.iter().any(|guard| guard.path == directory) {
            directories.push(open_directory_guard(&directory)?);
        }

        let expected_names = layout
            .get(parent)
            .ok_or_else(|| "expected source directory is missing".to_string())?;
        enumerate_exact(&directory, expected_names, false)?;

        let path = source_path(&canonical_root, spec.relative_path);
        let path_meta = fs::symlink_metadata(&path)
            .map_err(|_| "expected source copy is unavailable".to_string())?;
        if is_link_or_reparse(&path_meta) || !path_meta.is_file() {
            return Err("expected source copy is not a regular file".to_string());
        }
        let mut file = open_scoped_handle(&path, false)?;
        let snapshot = hash_held_file(&path, &mut file)?;
        if snapshot.sha256 != spec.sha256 || snapshot.byte_length != spec.byte_length {
            return Err("source copy does not match its V01 hash/length binding".to_string());
        }
        if !identities.insert(snapshot.file_identity) {
            return Err("source copy set contains a physical file alias".to_string());
        }
        total_bytes = total_bytes
            .checked_add(snapshot.byte_length)
            .ok_or_else(|| "source copy byte total overflow".to_string())?;
        if total_bytes > MAX_TOTAL_BYTES {
            return Err("source copy total exceeds the scan bound".to_string());
        }
        copies.push(BoundCopy {
            source_id: format!("source-{:04}", index + 1),
            expected: *spec,
            path,
            file,
            snapshot,
        });
    }

    let tree = SourceTree {
        canonical_root,
        directories,
        copies,
    };
    verify_source_tree(&tree, expected)?;
    Ok(tree)
}

fn verify_source_membership(
    tree: &SourceTree,
    layout: &BTreeMap<String, BTreeSet<String>>,
) -> ToolResult<()> {
    for guard in &tree.directories {
        ensure_directory_guard(guard)?;
    }
    let expected_directories = layout.keys().cloned().collect::<BTreeSet<_>>();
    enumerate_exact(&tree.canonical_root, &expected_directories, true)?;
    for (directory, file_names) in layout {
        enumerate_exact(&tree.canonical_root.join(directory), file_names, false)?;
    }
    for guard in &tree.directories {
        ensure_directory_guard(guard)?;
    }
    Ok(())
}

fn verify_source_tree(tree: &SourceTree, expected: &[ExpectedCopy]) -> ToolResult<()> {
    verify_source_tree_with_hook(tree, expected, || Ok(()))
}

fn verify_source_tree_with_hook<F>(
    tree: &SourceTree,
    expected: &[ExpectedCopy],
    after_initial_membership: F,
) -> ToolResult<()>
where
    F: FnOnce() -> ToolResult<()>,
{
    let layout = expected_layout(expected)?;
    verify_source_membership(tree, &layout)?;
    if tree.copies.len() != expected.len() {
        return Err("source copy count changed during observation".to_string());
    }
    after_initial_membership()?;
    for source in &tree.copies {
        if file_identity(&source.file)? != source.snapshot.file_identity {
            return Err("held source identity changed during observation".to_string());
        }
        let mut file = source
            .file
            .try_clone()
            .map_err(|_| "held source handle could not be cloned".to_string())?;
        let current = hash_held_file(&source.path, &mut file)?;
        if current != source.snapshot
            || current.sha256 != source.expected.sha256
            || current.byte_length != source.expected.byte_length
        {
            return Err("source copy changed during observation".to_string());
        }
    }
    // Files and sidecars can be added without changing the identity of a held
    // directory handle, so exact membership is checked again after all hashes.
    verify_source_membership(tree, &layout)
}

fn scope_name(scope: ReliquaryScopeKind) -> &'static str {
    match scope {
        ReliquaryScopeKind::Organization => "organization",
        ReliquaryScopeKind::Project => "project",
        ReliquaryScopeKind::Connection => "connection",
    }
}

fn owner_kind_name(kind: FileKind) -> &'static str {
    match kind {
        FileKind::Reliquary => "REL",
        FileKind::Phylactery => "PHY",
    }
}

fn header_observation(
    container: &Container,
) -> ToolResult<(HeaderObservation, String, String, String)> {
    let identity = container.identity();
    let owner_uuid = container.owner_uuid();
    if identity.is_none() && owner_uuid.is_some() {
        return Err("header owner identity is internally inconsistent".to_string());
    }
    let owner_kind = identity.map(|identity| owner_kind_name(identity.file_kind).to_string());
    let scope =
        identity.and_then(|identity| identity.scope.map(|value| scope_name(value).to_string()));
    let scope_state = match (identity, scope.is_some()) {
        (None, _) => "identity_header_absent",
        (Some(_), false) => "no_scope_value_encoded",
        (Some(_), true) => "scope_value_encoded",
    };
    let owner_uuid_state = match (identity, owner_uuid.is_some()) {
        (None, _) => "identity_header_absent",
        (Some(_), false) => "owner_uuid_not_available",
        (Some(_), true) => "owner_uuid_encoded",
    };
    let uuid_hex = owner_uuid.map(|bytes| hex(&bytes));
    let version = container.version();
    Ok((
        HeaderObservation {
            identity_header_present: identity.is_some(),
            owner_kind_observed_from_header: owner_kind,
            owner_uuid_hex_observed_from_header: uuid_hex,
            owner_uuid_observation_state: owner_uuid_state,
            native_scope_observed_from_header: scope,
            scope_observation_state: scope_state,
            values_independently_authenticated: false,
        },
        version.major.to_string(),
        version.minor.to_string(),
        container.header_len.to_string(),
    ))
}

fn open_header_reader(path: &Path, held_source: &File) -> ToolResult<Container> {
    let mut reader_file = held_source
        .try_clone()
        .map_err(|_| "held source handle could not be cloned".to_string())?;
    reader_file
        .seek(SeekFrom::Start(0))
        .map_err(|_| "held source could not seek for header observation".to_string())?;
    Container::open_read_only_for_scan_from_file(path, reader_file)
        .map_err(|_| "historical header could not be read in read-only mode".to_string())
}

fn physical_identity_report(identity: FileIdentity) -> PhysicalFileIdentity {
    PhysicalFileIdentity {
        volume_or_device_hex: format!("{:016x}", identity.0),
        file_index_or_inode_hex: format!("{:016x}", identity.1),
    }
}

fn source_set_digest(copies: &[BoundCopy]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"reliquary:f03:identity-observation-source-set:v1\0");
    hasher.update((copies.len() as u64).to_le_bytes());
    for copy in copies {
        for value in [
            copy.source_id.as_bytes(),
            copy.expected.relative_path.as_bytes(),
            copy.expected.sha256.as_bytes(),
            copy.snapshot.sha256.as_bytes(),
            copy.snapshot.modified_unix_ns.as_bytes(),
        ] {
            hasher.update((value.len() as u64).to_le_bytes());
            hasher.update(value);
        }
        hasher.update(copy.expected.byte_length.to_le_bytes());
        hasher.update(copy.snapshot.byte_length.to_le_bytes());
        hasher.update(copy.snapshot.file_identity.0.to_le_bytes());
        hasher.update(copy.snapshot.file_identity.1.to_le_bytes());
        hasher.update(copy.snapshot.physical_link_count.to_le_bytes());
    }
    hex(&hasher.finalize())
}

fn observe_source_tree(
    tree: &SourceTree,
    expected: &[ExpectedCopy],
) -> ToolResult<IdentityObservationReport> {
    let table_digest = expected_table_digest(expected);
    if table_digest != V01_EXPECTED_COPY_TABLE_SHA256 {
        return Err("V01 copy table does not match its committed binding".to_string());
    }
    observe_source_tree_with_binding(
        tree,
        expected,
        BindingProvenance {
            source_commit: V01_SOURCE_COMMIT,
            planning_commit: V01_PLANNING_COMMIT,
            planning_record_blob_sha1: V01_PLANNING_RECORD_BLOB_SHA1,
            v01_copy_proof_sha256: V01_COPY_PROOF_SHA256,
            v01_custody_receipt_sha256: V01_CUSTODY_RECEIPT_SHA256,
        },
        table_digest,
    )
}

fn observe_source_tree_with_binding(
    tree: &SourceTree,
    expected: &[ExpectedCopy],
    binding_provenance: BindingProvenance,
    table_digest: String,
) -> ToolResult<IdentityObservationReport> {
    let mut observations = Vec::with_capacity(tree.copies.len());
    for source in &tree.copies {
        if file_identity(&source.file)? != source.snapshot.file_identity {
            return Err("held source file identity changed before header observation".to_string());
        }
        let container = open_header_reader(&source.path, &source.file)?;
        let container_identity = container
            .backing_file_identity()
            .map_err(|_| "historical reader file identity is unavailable".to_string())?;
        if container_identity != Some(source.snapshot.file_identity) {
            return Err("historical header handle differs from V01-bound source file".to_string());
        }
        let container_metadata = container
            .backing_metadata()
            .map_err(|_| "historical reader metadata is unavailable".to_string())?;
        if container_metadata.len() != source.snapshot.byte_length
            || modified_unix_ns(&container_metadata)? != source.snapshot.modified_unix_ns
        {
            return Err("historical header source metadata changed".to_string());
        }
        let (header, major, minor, header_length) = header_observation(&container)?;
        drop(container);

        let mut held_file = source
            .file
            .try_clone()
            .map_err(|_| "held source handle could not be cloned".to_string())?;
        let after_header = hash_held_file(&source.path, &mut held_file)?;
        if after_header != source.snapshot {
            return Err("source changed during header-only observation".to_string());
        }

        observations.push(CopyObservation {
            source_id: source.source_id.clone(),
            v01_copy_sha256: source.expected.sha256.to_string(),
            v01_copy_byte_length: source.expected.byte_length.to_string(),
            observed_file_sha256: after_header.sha256,
            observed_file_byte_length: after_header.byte_length.to_string(),
            observed_file_modified_unix_ns: after_header.modified_unix_ns,
            physical_file_identity: physical_identity_report(after_header.file_identity),
            physical_link_count: after_header.physical_link_count.to_string(),
            format_major: major,
            format_minor: minor,
            header_length,
            header,
        });
    }
    verify_source_tree(tree, expected)?;
    Ok(IdentityObservationReport {
        schema_id: "reliquary-f03-identity-observation-v1",
        status: "IDENTITY_OBSERVATION_ONLY",
        evidence_scope: "physical_container_header_identity_only",
        authentic_admission: false,
        semantic_history_assessed: false,
        owner_identity_independently_authenticated: false,
        scope_independently_authenticated: false,
        binding_provenance,
        expected_copy_table_sha256: table_digest,
        source_set_sha256: source_set_digest(&tree.copies),
        expected_copy_count: expected.len().to_string(),
        observed_copy_count: observations.len().to_string(),
        observations,
    })
}

fn canonical_source_root(root: &Path) -> ToolResult<PathBuf> {
    ensure_no_reparse_components(root)?;
    let canonical =
        fs::canonicalize(root).map_err(|_| "source root could not be canonicalized".to_string())?;
    let metadata = fs::symlink_metadata(&canonical)
        .map_err(|_| "source root metadata is unavailable".to_string())?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err("source root is not a regular directory".to_string());
    }
    Ok(canonical)
}

fn output_ancestor_guards(parent: &Path) -> ToolResult<Vec<DirectoryGuard>> {
    let canonical_parent =
        fs::canonicalize(parent).map_err(|_| "output parent is unavailable".to_string())?;
    let mut paths = vec![canonical_parent.clone()];
    if let Some(grandparent) = canonical_parent.parent() {
        if grandparent != canonical_parent {
            paths.push(grandparent.to_path_buf());
        }
    }
    let mut guards = Vec::with_capacity(paths.len());
    for path in paths {
        guards.push(open_directory_guard(&path)?);
    }
    Ok(guards)
}

fn verify_directory_guards(guards: &[DirectoryGuard]) -> ToolResult<()> {
    for guard in guards {
        ensure_directory_guard(guard)?;
    }
    Ok(())
}

fn resolve_fresh_output(output: &Path, source_root: &Path) -> ToolResult<PathBuf> {
    ensure_no_reparse_components(source_root)?;
    let source_root = fs::canonicalize(source_root)
        .map_err(|_| "source root could not be canonicalized".to_string())?;
    let exists = match fs::symlink_metadata(output) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => return Err("output path could not be inspected".to_string()),
    };
    if exists {
        return Err("fresh output path already exists".to_string());
    }
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| "output path has no parent".to_string())?;
    ensure_no_reparse_components(parent)?;
    let canonical_parent = fs::canonicalize(parent)
        .map_err(|_| "output parent could not be canonicalized".to_string())?;
    let name = output
        .file_name()
        .ok_or_else(|| "output path has no directory name".to_string())?;
    let canonical_output = canonical_parent.join(name);
    if canonical_output.starts_with(&source_root) || source_root.starts_with(&canonical_output) {
        return Err("output and source root must be disjoint".to_string());
    }
    Ok(canonical_output)
}

fn serialize_report(report: &IdentityObservationReport) -> ToolResult<Vec<u8>> {
    let mut bytes = serde_json::to_vec(report)
        .map_err(|_| "identity report serialization failed".to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

impl ReportGuard {
    fn verify(&mut self) -> ToolResult<()> {
        verify_directory_guards(&self.output_ancestors)?;
        ensure_directory_guard(&self.output_directory)?;
        let expected_names = BTreeSet::from([REPORT_NAME.to_string()]);
        enumerate_exact(&self.output_path, &expected_names, false)?;

        let held_metadata = self
            .file
            .metadata()
            .map_err(|_| "held identity report metadata is unavailable".to_string())?;
        if file_identity(&self.file)? != self.file_identity
            || !held_metadata.is_file()
            || held_metadata.len() != self.expected_bytes.len() as u64
            || physical_link_count(&self.file)? != 1
        {
            return Err("held identity report changed before finalization".to_string());
        }

        // The secondary handle shares the retained writer's existing read and
        // write access modes; the retained handle still denies new writers/deletes.
        let path_handle = open_output_verification_handle(&self.report_path)?;
        let path_metadata = path_handle
            .metadata()
            .map_err(|_| "identity report path handle metadata is unavailable".to_string())?;
        if file_identity(&path_handle)? != self.file_identity
            || !path_metadata.is_file()
            || path_metadata.len() != self.expected_bytes.len() as u64
            || physical_link_count(&path_handle)? != 1
        {
            return Err("identity report path no longer names the held report".to_string());
        }

        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|_| "held identity report could not seek for verification".to_string())?;
        let mut verified = Vec::with_capacity(self.expected_bytes.len());
        let limit = (self.expected_bytes.len() as u64)
            .checked_add(1)
            .ok_or_else(|| "identity report length overflow".to_string())?;
        (&mut self.file)
            .take(limit)
            .read_to_end(&mut verified)
            .map_err(|_| "held identity report could not be reread".to_string())?;
        if verified != self.expected_bytes {
            return Err("identity report bytes differ from the finalized report".to_string());
        }

        ensure_directory_guard(&self.output_directory)?;
        verify_directory_guards(&self.output_ancestors)?;
        enumerate_exact(&self.output_path, &expected_names, false)?;
        if file_identity(&path_handle)? != self.file_identity {
            return Err("identity report path identity changed during finalization".to_string());
        }
        Ok(())
    }
}

fn write_fresh_report(
    output: &Path,
    bytes: &[u8],
    output_ancestors: Vec<DirectoryGuard>,
) -> ToolResult<ReportGuard> {
    if bytes.is_empty() || bytes.len() > MAX_REPORT_BYTES {
        return Err("identity report exceeds its bounded size".to_string());
    }
    verify_directory_guards(&output_ancestors)?;
    match fs::symlink_metadata(output) {
        Ok(_) => return Err("fresh output path appeared before creation".to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("fresh output path could not be inspected".to_string()),
    }
    fs::create_dir(output)
        .map_err(|_| "fresh output directory could not be created".to_string())?;
    let output_guard = open_directory_guard(output)?;
    let report_path = output.join(REPORT_NAME);
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0x0000_0001);
    }
    let mut file = options
        .open(&report_path)
        .map_err(|_| "identity report could not be created without overwrite".to_string())?;
    file.write_all(bytes)
        .map_err(|_| "identity report write failed".to_string())?;
    file.sync_all()
        .map_err(|_| "identity report sync failed".to_string())?;
    let mut guard = ReportGuard {
        output_path: output.to_path_buf(),
        report_path,
        output_directory: output_guard,
        output_ancestors,
        file_identity: file_identity(&file)?,
        file,
        expected_bytes: bytes.to_vec(),
    };
    guard.verify()?;
    Ok(guard)
}

fn observe_and_write(options: Options) -> ToolResult<IdentityObservationReport> {
    let source_root = canonical_source_root(&options.source_root)?;
    let output = resolve_fresh_output(&options.output, &source_root)?;
    let output_parent = output
        .parent()
        .ok_or_else(|| "output parent is unavailable".to_string())?;
    let output_ancestors = output_ancestor_guards(output_parent)?;

    let expected = v01_specs();
    if expected.len() != 6 || expected_table_digest(&expected) != V01_EXPECTED_COPY_TABLE_SHA256 {
        return Err("V01 expected copy table is incomplete".to_string());
    }
    let tree = collect_source_tree(&source_root, &expected)?;
    let report = observe_source_tree(&tree, &expected)?;
    let bytes = serialize_report(&report)?;
    verify_source_tree(&tree, &expected)?;
    let mut report_guard = write_fresh_report(&output, &bytes, output_ancestors)?;
    verify_source_tree(&tree, &expected)?;
    report_guard.verify()?;
    Ok(report)
}

fn parse_options(args: impl Iterator<Item = String>) -> ToolResult<Option<Options>> {
    let mut source_root = None;
    let mut output = None;
    let mut args = args;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--source-root" => {
                if source_root.is_some() {
                    return Err("duplicate source-root argument".to_string());
                }
                source_root = Some(PathBuf::from(
                    args.next().ok_or("--source-root requires a path")?,
                ));
            }
            "--output" => {
                if output.is_some() {
                    return Err("duplicate output argument".to_string());
                }
                output = Some(PathBuf::from(
                    args.next().ok_or("--output requires a path")?,
                ));
            }
            "--help" | "-h" => return Ok(None),
            _ => return Err("unknown identity-probe argument".to_string()),
        }
    }
    Ok(Some(Options {
        source_root: source_root.ok_or("--source-root is required")?,
        output: output.ok_or("--output is required")?,
    }))
}

/// Entry point for the separate, header-only identity observation example.
pub fn cli_main() -> i32 {
    let options = match parse_options(env::args().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!(
                "Usage: f03-identity-observation --source-root <verified-copies> --output <fresh-protected-output>"
            );
            return 0;
        }
        Err(_) => {
            eprintln!("{}", r#"{"status":"FAIL","error":"invalid_arguments"}"#);
            return 2;
        }
    };
    match observe_and_write(options) {
        Ok(report) => {
            println!(
                r#"{{"status":"IDENTITY_OBSERVATION_ONLY","observed_copy_count":"{}","authentic_admission":false}}"#,
                report.observed_copy_count
            );
            0
        }
        Err(_) => {
            eprintln!(
                "{}",
                r#"{"status":"FAIL","error":"identity_observation_failed"}"#
            );
            2
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
#[path = "f03_identity_observation_tests.rs"]
mod tests;
