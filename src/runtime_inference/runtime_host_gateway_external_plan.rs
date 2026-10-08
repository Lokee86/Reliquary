use super::{owner_operations::FrozenPreparedMemory, publication_codec};
use crate::external_source::processing::{PreparedExternalMemory, PreparedExternalProcessing};
use crate::external_source::store::SourceState;
use crate::external_source::{ExternalMemoryEvidence, ExternalSourceError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FrozenMemory {
    memory: FrozenPreparedMemory,
    evidence: ExternalMemoryEvidence,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExternalWindowPlan {
    window: [u8; 32],
    principal: String,
    phy: Option<String>,
    project: Vec<FrozenMemory>,
    user: Vec<FrozenMemory>,
    rejected: usize,
}
const MAX_BYTES: usize = 16 * 1024 * 1024;
pub(super) fn path(root: &Path, owner: &str, window: [u8; 32]) -> PathBuf {
    let id: String = window.iter().map(|b| format!("{b:02x}")).collect();
    root.join("external-processing")
        .join(owner)
        .join(format!("{id}.rxp"))
}
fn freeze_memory(m: &PreparedExternalMemory) -> Result<FrozenMemory, ExternalSourceError> {
    let memory = crate::insomnia::PreparedMemory {
        draft: m.draft.clone(),
        temporal: crate::insomnia::temporal::assess_draft(&m.draft),
        temporal_inference: m.temporal.as_ref().map(|t| t.inference.clone()),
        routing_metadata: m.routing.clone(),
    };
    Ok(FrozenMemory {
        memory: publication_codec::freeze_memory(&memory, None)
            .map_err(|_| ExternalSourceError::Storage)?,
        evidence: m.evidence.clone(),
    })
}
fn thaw_memory(
    m: FrozenMemory,
    window: [u8; 32],
) -> Result<PreparedExternalMemory, ExternalSourceError> {
    m.evidence.validate()?;
    if m.evidence.processing_window != window {
        return Err(ExternalSourceError::Storage);
    }
    let (memory, source) =
        publication_codec::thaw_memory(&m.memory).map_err(|_| ExternalSourceError::Storage)?;
    if source.is_some()
        || memory.draft.source_episode_id.is_some()
        || memory.draft.source_node_id.is_some()
        || memory.draft.content_source_node_id.is_some()
        || memory.draft.grounding_source_node_id.is_some()
        || memory.draft.content_source_conversation_id.is_some()
        || memory.draft.grounding_source_conversation_id.is_some()
    {
        return Err(ExternalSourceError::Storage);
    }
    let temporal = memory.bound_temporal_inference();
    Ok(PreparedExternalMemory {
        draft: memory.draft,
        temporal,
        routing: memory.routing_metadata,
        evidence: m.evidence,
    })
}
impl ExternalWindowPlan {
    pub(super) fn freeze(
        value: &PreparedExternalProcessing,
        principal: &str,
        phy: Option<&str>,
    ) -> Result<Self, ExternalSourceError> {
        Ok(Self {
            window: value.window,
            principal: principal.into(),
            phy: phy.map(str::to_owned),
            project: value
                .project
                .iter()
                .map(freeze_memory)
                .collect::<Result<_, _>>()?,
            user: value
                .user
                .iter()
                .map(freeze_memory)
                .collect::<Result<_, _>>()?,
            rejected: value.rejected,
        })
    }
    pub(super) fn thaw(
        self,
        window: [u8; 32],
        principal: &str,
        phy: Option<&str>,
        state: SourceState,
    ) -> Result<PreparedExternalProcessing, ExternalSourceError> {
        if self.window != window || self.project.len() + self.user.len() > 64 {
            return Err(ExternalSourceError::Storage);
        }
        if !self.user.is_empty()
            && (self.principal != principal || self.phy.as_deref() != phy || phy.is_none())
        {
            return Err(ExternalSourceError::Unauthorized);
        }
        let project = self
            .project
            .into_iter()
            .map(|m| thaw_memory(m, window))
            .collect::<Result<_, _>>()?;
        let user = self
            .user
            .into_iter()
            .map(|m| thaw_memory(m, window))
            .collect::<Result<Vec<_>, _>>()?;
        if user.iter().any(|m| m.evidence.principal_id != principal) {
            return Err(ExternalSourceError::Unauthorized);
        }
        Ok(PreparedExternalProcessing {
            window,
            expected: state,
            project,
            user,
            rejected: self.rejected,
        })
    }
    pub(super) fn load(path: &Path) -> Result<Option<Self>, ExternalSourceError> {
        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(ExternalSourceError::Storage),
        };
        if metadata.len() > MAX_BYTES as u64 + 48 {
            return Err(ExternalSourceError::Bounds);
        }
        let bytes = fs::read(path).map_err(|_| ExternalSourceError::Storage)?;
        if bytes.len() < 48
            || &bytes[..8] != b"CVAXPLN1"
            || u64::from_le_bytes(bytes[8..16].try_into().unwrap()) != bytes.len() as u64 - 48
            || Sha256::digest(&bytes[..bytes.len() - 32]).as_slice() != &bytes[bytes.len() - 32..]
        {
            return Err(ExternalSourceError::Storage);
        }
        serde_json::from_slice(&bytes[16..bytes.len() - 32])
            .map(Some)
            .map_err(|_| ExternalSourceError::Storage)
    }
    pub(super) fn verify_receipts(
        &self,
        window: [u8; 32],
        principal: &str,
        phy: Option<&str>,
        project: &[crate::Memory],
        user: &[crate::Memory],
    ) -> Result<(), ExternalSourceError> {
        if self.window != window {
            return Err(ExternalSourceError::Storage);
        }
        let ids = |memories: &[FrozenMemory]| -> Result<Vec<crate::MemoryId>, ExternalSourceError> {
            memories
                .iter()
                .map(|m| {
                    let (memory, source) = publication_codec::thaw_memory(&m.memory)
                        .map_err(|_| ExternalSourceError::Storage)?;
                    if source.is_some() {
                        return Err(ExternalSourceError::Storage);
                    }
                    Ok(crate::memory_model::memory_id(&memory.draft.mutation_id))
                })
                .collect()
        };
        if ids(&self.project)? != project.iter().map(|m| m.id).collect::<Vec<_>>() {
            return Err(ExternalSourceError::Storage);
        }
        if self.principal == principal
            && self.phy.as_deref() == phy
            && ids(&self.user)? != user.iter().map(|m| m.id).collect::<Vec<_>>()
        {
            return Err(ExternalSourceError::Storage);
        }
        Ok(())
    }
    /// Called only under the source REL sequencer gate; first durable plan wins.
    pub(super) fn install(&self, path: &Path) -> Result<(), ExternalSourceError> {
        if path.exists() {
            return Ok(());
        }
        let json = serde_json::to_vec(self).map_err(|_| ExternalSourceError::Storage)?;
        if json.len() > MAX_BYTES {
            return Err(ExternalSourceError::Bounds);
        }
        let mut bytes = b"CVAXPLN1".to_vec();
        bytes.extend_from_slice(&(json.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&json);
        bytes.extend_from_slice(&Sha256::digest(&bytes));
        let parent = path.parent().ok_or(ExternalSourceError::Storage)?;
        fs::create_dir_all(parent).map_err(|_| ExternalSourceError::Storage)?;
        let mut total = 0u64;
        let mut count = 0usize;
        for entry in fs::read_dir(parent).map_err(|_| ExternalSourceError::Storage)? {
            let entry = entry.map_err(|_| ExternalSourceError::Storage)?;
            if entry.path().extension().and_then(|s| s.to_str()) != Some("rxp") {
                continue;
            }
            count += 1;
            total = total
                .checked_add(
                    entry
                        .metadata()
                        .map_err(|_| ExternalSourceError::Storage)?
                        .len(),
                )
                .ok_or(ExternalSourceError::Bounds)?;
        }
        if count >= 4096 || total + bytes.len() as u64 > 256 * 1024 * 1024 {
            return Err(ExternalSourceError::Bounds);
        }

        let temporary = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| ExternalSourceError::Storage)?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| ExternalSourceError::Storage)?;
        drop(file);
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            #[link(name = "Kernel32")]
            unsafe extern "system" {
                fn MoveFileExW(a: *const u16, b: *const u16, flags: u32) -> i32;
            }
            let a: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
            let b: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            if unsafe { MoveFileExW(a.as_ptr(), b.as_ptr(), 0x8) } == 0 {
                let _ = fs::remove_file(&temporary);
                return Err(ExternalSourceError::Storage);
            }
        }
        #[cfg(not(windows))]
        fs::rename(&temporary, path).map_err(|_| ExternalSourceError::Storage)?;
        #[cfg(unix)]
        std::fs::File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|_| ExternalSourceError::Storage)?;
        Ok(())
    }
}
