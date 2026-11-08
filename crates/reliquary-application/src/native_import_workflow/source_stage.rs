//! Authenticated composition over the existing epoch-scoped staging workflow.

use super::*;
use crate::native_import_workflow::{NativeImportBatch, stage_native_import_with_admission};
use reliquary_owner_session::{
    OwnerSessionError, RelSession, RelWriter,
    model::ImportReceipt,
    records::{ImportEpochStatus, import_id},
};
use std::{cell::Cell, time::SystemTime};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeImportState {
    Staging,
    Staged,
    Settled,
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NativeImportStatus {
    pub import_id: String,
    pub source_key: String,
    pub source_format: String,
    pub state: NativeImportState,
    pub receipt_digest: Option<[u8; 32]>,
    pub started_at_ns: i64,
    pub completed_at_ns: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NativeImportStageResult {
    pub import_id: String,
    pub source_sha256: [u8; 32],
    pub receipt_digest: [u8; 32],
    pub episode_ids: Vec<[u8; 32]>,
    pub coverage: NativeImportCoverage,
    pub parent_mappings: Vec<NativeImportParentMapping>,
    pub already_staged: bool,
    pub already_completed: bool,
}

#[derive(Debug)]
pub enum NativeImportStageError {
    Conflict,
    Revoked,
    Owner(OwnerSessionError),
}

impl fmt::Display for NativeImportStageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => {
                f.write_str("native import conflicts with the mounted owner's source or open epoch")
            }
            Self::Revoked => f.write_str("credential has been revoked"),
            Self::Owner(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl std::error::Error for NativeImportStageError {}

pub fn native_import_status(session: &RelSession, id: &str) -> Option<NativeImportStatus> {
    let (start, completed_at_ns) = match session.import_status(id)? {
        ImportEpochStatus::Open(start) => (start, None),
        ImportEpochStatus::Completed { start, complete } => (start, Some(complete.completed_at_ns)),
    };
    let receipt_digest = decode_key(&start.import_key)
        .and_then(|key| session.native_import_receipt(&key))
        .map(|receipt| receipt.receipt_digest);
    let state = if completed_at_ns.is_some() {
        NativeImportState::Completed
    } else if session.native_import_settlement(id).is_some() {
        NativeImportState::Settled
    } else if receipt_digest.is_some() {
        NativeImportState::Staged
    } else {
        NativeImportState::Staging
    };
    Some(NativeImportStatus {
        import_id: start.import_id,
        source_key: start.import_key,
        source_format: start.source_format,
        state,
        receipt_digest,
        started_at_ns: start.started_at_ns,
        completed_at_ns,
    })
}

impl NativeImportPlan {
    /// The owner-local ID can be retained before staging to inspect interrupted work.
    pub fn import_id(&self, owner: [u8; 16]) -> String {
        import_id(owner, &hex(&self.import_key))
    }

    /// The Gateway supplies the authenticated Instance principal and retains
    /// the mounted writer lock for the duration of staging.
    pub fn stage(
        self,
        writer: &mut RelWriter,
        principal: &str,
        credential_active: impl Fn() -> bool,
    ) -> Result<NativeImportStageResult, NativeImportStageError> {
        let owner = writer.owner_id();
        let id = self.import_id(owner);
        let session = writer.read_session();
        let (source_namespace, source_format) = self.source.identity(principal);
        let copied = matches!(&self.source, NativeSource::HermesCopy { .. });
        if copied
            && self.conversations.iter().any(|row| {
                session
                    .conversation_tombstone(&row.conversation_id)
                    .is_some()
            })
        {
            return Err(NativeImportStageError::Conflict);
        }
        let already_staged = session.native_import_receipt(&self.import_key).is_some();
        let status = session.import_status(&id);
        let mut turns = self.turns;
        let anchored = if let NativeSource::HermesCopy {
            scope,
            predecessor: Some(predecessor),
        } = &self.source
        {
            hermes::attach_predecessor(
                session,
                scope,
                &source_namespace,
                predecessor,
                status.is_some(),
                &mut turns,
            )?;
            true
        } else {
            false
        };
        if let Some(status) = status {
            let start = match status {
                ImportEpochStatus::Open(start) | ImportEpochStatus::Completed { start, .. } => {
                    start
                }
            };
            if start.source_format != source_format || start.source_namespace != source_namespace {
                return Err(NativeImportStageError::Conflict);
            }
        } else {
            let ids: BTreeSet<_> = self
                .conversations
                .iter()
                .map(|row| row.conversation_id.as_str())
                .collect();
            if session.open_native_import_id().is_some()
                || ids
                    .iter()
                    .any(|id| session.conversation_tombstone(id).is_some())
                || (!anchored
                    && session
                        .accepted_turn_sources()
                        .iter()
                        .any(|turn| ids.contains(turn.conversation_id.as_str())))
            {
                return Err(NativeImportStageError::Conflict);
            }
        }

        for (turn, _) in &mut turns {
            if !copied && turn.role == "user" {
                turn.principal_id = Some(principal.to_owned());
            }
        }
        let started_at_ns = SystemTime::UNIX_EPOCH
            .elapsed()
            .ok()
            .and_then(|duration| duration.as_nanos().try_into().ok())
            .ok_or_else(|| {
                NativeImportStageError::Owner(OwnerSessionError::Record(
                    "system time is outside the import clock range".into(),
                ))
            })?;
        let revoked = Cell::new(false);
        let staged = stage_native_import_with_admission(
            writer,
            NativeImportBatch {
                source: ImportReceipt {
                    schema_version: 1,
                    owner_uuid: owner,
                    import_key: self.import_key,
                    source_namespace,
                    source_format: source_format.into(),
                    source_digest: self.import_key,
                    receipt_digest: [0; 32],
                    conversations: self.conversations,
                },
                turns,
                episodes: self.episodes,
                started_at_ns,
            },
            || {
                if credential_active() {
                    Ok(())
                } else {
                    revoked.set(true);
                    Err(OwnerSessionError::Record(
                        "credential has been revoked".into(),
                    ))
                }
            },
        )
        .map_err(|error| {
            if revoked.get() {
                NativeImportStageError::Revoked
            } else {
                NativeImportStageError::Owner(error)
            }
        })?;
        Ok(NativeImportStageResult {
            import_id: staged.import_id,
            source_sha256: self.source_sha256,
            receipt_digest: staged.receipt.receipt_digest,
            episode_ids: staged.episodes.into_iter().map(|id| id.0).collect(),
            coverage: self.coverage,
            parent_mappings: self.parent_mappings,
            already_staged,
            already_completed: staged.already_completed,
        })
    }
}

pub(super) fn principal_namespace(namespace: &str, principal: &str) -> String {
    let principal_key = Sha256::digest(principal.as_bytes());
    format!("{namespace}/principal-{principal_key:x}")
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_key(key: &str) -> Option<[u8; 32]> {
    if key.len() != 64 || !key.is_ascii() {
        return None;
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&key[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}
