#[path = "migration_dream_cooldown.rs"]
mod dream_cooldown_legacy;
#[path = "migration_phy.rs"]
mod phy;
#[path = "migration_rel.rs"]
mod rel;

use self::dream_cooldown_legacy::legacy_dream_cooldown_records;
use crate::dream_cooldown::{DREAM_CADENCE_VERSION, DREAM_PROCESSING_LANE};
use crate::memory_store::MemoryStore;
use crate::processing_epoch_model::{ProcessingEpochState, ProcessingLaneId};
use crate::processing_epoch_store::merge_processing_epoch_records;
use crate::{Container, ContainerIdentity, FileKind, MemoryId, ReliquaryScopeKind};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::path::Path;
use uuid::Uuid;

const LEGACY_WORKSPACE_MAGIC: &[u8; 8] = b"CVAWKSP1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationResult {
    pub owner_id: String,
    pub file_kind: FileKind,
    pub scope: Option<ReliquaryScopeKind>,
    pub derived_from_legacy_workspace_id: bool,
}

#[derive(Debug)]
pub enum MigrationError {
    SourceAndOutputMatch,
    OutputExists,
    AlreadyCurrent,
    InvalidLegacyWorkspaceMetadata(&'static str),
    Operation(String),
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceAndOutputMatch => write!(f, "migration source and output must differ"),
            Self::OutputExists => write!(f, "migration output already exists"),
            Self::AlreadyCurrent => write!(f, "source already has a durable owner ID"),
            Self::InvalidLegacyWorkspaceMetadata(message) => {
                write!(f, "invalid legacy workspace metadata: {message}")
            }
            Self::Operation(message) => write!(f, "migration failed: {message}"),
        }
    }
}

impl std::error::Error for MigrationError {}

pub fn migrate_file(
    source: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<MigrationResult, MigrationError> {
    let source = source.as_ref();
    let output = output.as_ref();
    reject_invalid_paths(source, output)?;

    let inspection = inspect_source(source)?;
    let (owner_uuid, derived_from_legacy_workspace_id) = match inspection.legacy_workspace_id {
        Some(id) => (legacy_workspace_uuid(&id), true),
        None => (*Uuid::new_v4().as_bytes(), false),
    };

    let migrated = match inspection.identity.file_kind {
        FileKind::Reliquary => rel::migrate(
            source,
            output,
            inspection
                .identity
                .scope
                .expect("validated Reliquary scope"),
            owner_uuid,
        ),
        FileKind::Phylactery => phy::migrate(source, output, owner_uuid),
    };
    let owner_id = match migrated {
        Ok(owner_id) => owner_id,
        Err(error) => {
            let _ = fs::remove_file(output);
            return Err(error);
        }
    };

    Ok(MigrationResult {
        owner_id,
        file_kind: inspection.identity.file_kind,
        scope: inspection.identity.scope,
        derived_from_legacy_workspace_id,
    })
}

struct SourceInspection {
    identity: ContainerIdentity,
    legacy_workspace_id: Option<String>,
}

fn inspect_source(source: &Path) -> Result<SourceInspection, MigrationError> {
    let mut container = op(Container::open(source))?;
    if container.owner_uuid().is_some() {
        return Err(MigrationError::AlreadyCurrent);
    }
    let identity = container.identity().unwrap_or(ContainerIdentity {
        file_kind: FileKind::Reliquary,
        scope: Some(ReliquaryScopeKind::Project),
    });
    let legacy_workspace_id = if identity.file_kind == FileKind::Reliquary {
        legacy_workspace_id(&mut container)?
    } else {
        None
    };
    Ok(SourceInspection {
        identity,
        legacy_workspace_id,
    })
}

fn legacy_workspace_id(container: &mut Container) -> Result<Option<String>, MigrationError> {
    let mut found = None;
    for chunk in op(container.chunks())? {
        let payload = op(container.read(chunk))?;
        let Some(id) = decode_legacy_workspace_id(&payload)? else {
            continue;
        };
        if found.is_some() {
            return Err(MigrationError::InvalidLegacyWorkspaceMetadata(
                "multiple workspace records",
            ));
        }
        found = Some(id);
    }
    Ok(found)
}

fn decode_legacy_workspace_id(payload: &[u8]) -> Result<Option<String>, MigrationError> {
    if payload.len() < 8 || &payload[..8] != LEGACY_WORKSPACE_MAGIC {
        return Ok(None);
    }
    let mut cursor = 8;
    let id = read_legacy_string(payload, &mut cursor)?;
    let _name = read_legacy_string(payload, &mut cursor)?;
    let _workspace_type = read_legacy_string(payload, &mut cursor)?;
    if cursor != payload.len() || id.trim().is_empty() {
        return Err(MigrationError::InvalidLegacyWorkspaceMetadata(
            "invalid workspace record",
        ));
    }
    Ok(Some(id))
}

fn read_legacy_string(payload: &[u8], cursor: &mut usize) -> Result<String, MigrationError> {
    let end = cursor
        .checked_add(4)
        .ok_or(MigrationError::InvalidLegacyWorkspaceMetadata(
            "length overflow",
        ))?;
    let raw = payload
        .get(*cursor..end)
        .ok_or(MigrationError::InvalidLegacyWorkspaceMetadata(
            "truncated length",
        ))?;
    let len = u32::from_le_bytes(raw.try_into().unwrap()) as usize;
    *cursor = end;
    let end = cursor
        .checked_add(len)
        .ok_or(MigrationError::InvalidLegacyWorkspaceMetadata(
            "length overflow",
        ))?;
    let raw = payload
        .get(*cursor..end)
        .ok_or(MigrationError::InvalidLegacyWorkspaceMetadata(
            "truncated string",
        ))?;
    *cursor = end;
    String::from_utf8(raw.to_vec())
        .map_err(|_| MigrationError::InvalidLegacyWorkspaceMetadata("invalid UTF-8"))
}

fn legacy_workspace_uuid(id: &str) -> [u8; 16] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-legacy-workspace-owner-v1\0");
    hash.update((id.len() as u64).to_le_bytes());
    hash.update(id.as_bytes());
    let digest = hash.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    bytes
}

fn reject_invalid_paths(source: &Path, output: &Path) -> Result<(), MigrationError> {
    if source == output {
        return Err(MigrationError::SourceAndOutputMatch);
    }
    if output.exists() {
        let same = fs::canonicalize(source)
            .ok()
            .zip(fs::canonicalize(output).ok())
            .is_some_and(|(source, output)| source == output);
        return Err(if same {
            MigrationError::SourceAndOutputMatch
        } else {
            MigrationError::OutputExists
        });
    }
    Ok(())
}

pub(super) fn processing_epochs_for_migration(
    container: &mut Container,
    memories: &MemoryStore,
    current: Vec<(MemoryId, ProcessingLaneId, ProcessingEpochState)>,
) -> Result<Vec<(MemoryId, ProcessingLaneId, ProcessingEpochState)>, MigrationError> {
    let mut records = current;
    for (memory_id, legacy) in op(legacy_dream_cooldown_records(container))? {
        let updated_at_ns = memories
            .records()
            .iter()
            .filter(|record| record.id == memory_id)
            .max_by_key(|record| record.revision)
            .map(|record| record.updated_at_ns)
            .ok_or_else(|| {
                MigrationError::Operation(
                    "legacy Dream cooldown references a missing Memory".into(),
                )
            })?;
        records.push((
            memory_id,
            DREAM_PROCESSING_LANE,
            ProcessingEpochState {
                satisfied_through_epoch: legacy.epoch,
                cadence_version: DREAM_CADENCE_VERSION,
                checkpoint_at_ns: Some(legacy.processed_at_ns.unwrap_or(updated_at_ns)),
            },
        ));
    }
    op(merge_processing_epoch_records(records))
}

pub(super) fn require_same<T: Eq + fmt::Debug>(
    expected: T,
    actual: T,
    owner: &'static str,
) -> Result<(), MigrationError> {
    if expected == actual {
        Ok(())
    } else {
        Err(MigrationError::Operation(format!(
            "{owner} identity changed during repack: expected={expected:?} actual={actual:?}"
        )))
    }
}

pub(super) fn op<T, E: fmt::Display>(result: Result<T, E>) -> Result<T, MigrationError> {
    result.map_err(|error| MigrationError::Operation(error.to_string()))
}
