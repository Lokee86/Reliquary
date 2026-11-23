//! Admitted grouped publication metadata and exact carrier access.

use super::*;
use reliquary_container::ObjectRef;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::borrow::Cow;

#[derive(Serialize, Deserialize)]
pub(super) struct StoredGroupedCompletion {
    pub(super) episode_id: [u8; 32],
    pub(super) extractor_model: String,
    pub(super) memory_ids: Vec<[u8; 32]>,
    pub(super) external_memory_refs: Vec<crate::records::GroupedExternalMemoryRef>,
    pub(super) freshness_birth_memory_ids: Vec<[u8; 32]>,
    pub(super) memory_versions: Option<(u64, u64)>,
    backing: GroupedCompletionBacking,
}

#[derive(Serialize, Deserialize)]
enum GroupedCompletionBacking {
    Indexed {
        object: ObjectRef,
        digest: [u8; 32],
    },
    #[serde(skip)]
    Detached(Box<GroupedInsomniaCompletion>),
}

impl StoredGroupedCompletion {
    pub(super) fn from_admitted(
        completion: GroupedInsomniaCompletion,
        frame: Option<(ObjectRef, &[u8])>,
    ) -> Result<Self, MemoryError> {
        let mut versions = None;
        for bytes in &completion.memory_records {
            let version = reliquary_memory_core::decode_grouped_creation(bytes)?.memory_version;
            let (first, last) = versions.unwrap_or((version, version));
            versions = Some((first.min(version), last.max(version)));
        }
        let freshness_birth_memory_ids = if completion.freshness_birth_turn.is_some() {
            completion.freshness_birth_memory_ids.clone()
        } else {
            Vec::new()
        };
        Ok(Self {
            episode_id: completion.episode_id,
            extractor_model: completion.extractor_model.clone(),
            memory_ids: completion.memory_ids.clone(),
            external_memory_refs: completion.external_memory_refs.clone(),
            freshness_birth_memory_ids,
            memory_versions: versions,
            backing: match frame {
                Some((object, payload)) => GroupedCompletionBacking::Indexed {
                    object,
                    digest: Sha256::digest(payload).into(),
                },
                None => GroupedCompletionBacking::Detached(Box::new(completion)),
            },
        })
    }
}

impl RelSession {
    pub(super) fn load_grouped_completion<'a>(
        &'a self,
        key: (WorkEpisodeId, u32),
        completion: &'a StoredGroupedCompletion,
    ) -> Result<Cow<'a, GroupedInsomniaCompletion>, OwnerSessionError> {
        match &completion.backing {
            GroupedCompletionBacking::Detached(value) => Ok(Cow::Borrowed(value.as_ref())),
            GroupedCompletionBacking::Indexed { object, digest } => {
                let reader = self.grouped_completion_reader.as_ref().ok_or_else(|| {
                    OwnerSessionError::Record(
                        "grouped publication has no active owner lease".into(),
                    )
                })?;
                let payload = reader.read_frame(*object, *digest)?;
                let result = GroupedInsomniaCompletion::decode(&payload)
                    .map_err(OwnerSessionError::Record)
                    .and_then(|decoded| {
                        if (WorkEpisodeId(decoded.episode_id), decoded.attempt) != key {
                            return Err(OwnerSessionError::Record(
                                "indexed grouped publication identity differs".into(),
                            ));
                        }
                        Ok(decoded)
                    })
                    .inspect_err(|_| reader.fence_writes())?;
                Ok(Cow::Owned(result))
            }
        }
    }
}
