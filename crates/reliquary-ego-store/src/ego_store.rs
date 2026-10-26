#[path = "ego_store_anchors.rs"]
mod anchors;
#[path = "ego_store_compact.rs"]
mod compact;
#[path = "ego_store_documents.rs"]
mod documents;
#[path = "ego_store_identities.rs"]
mod identities;
#[path = "ego_store_read.rs"]
mod read;
#[path = "ego_store_validation.rs"]
mod validation;

use crate::ego_codec::{EgoRecord, decode, record_ego_version};
use crate::{
    EgoAnchorId, EgoAnchorPriority, EgoError, EgoIdentityId, EgoPersonality, EgoWebSynthesis,
};
use reliquary_container::Container;
use std::collections::BTreeMap;

use validation::validate_record_revision;
pub use validation::validate_source_memory_version;

/// Stable identity used by the current single-default Identity convenience API.
pub const DEFAULT_IDENTITY_ID: EgoIdentityId = EgoIdentityId([0; 16]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EgoOwnerKind {
    Reliquary,
    Phylactery,
}

#[derive(Clone, Debug)]
struct IdentityState {
    ego_version: u64,
    revision: u64,
    deleted: bool,
    name: String,
    text: String,
}

#[derive(Clone, Debug)]
struct AnchorState {
    ego_version: u64,
    revision: u64,
    deleted: bool,
    priority: EgoAnchorPriority,
    text: String,
}

pub struct EgoStore {
    owner_kind: EgoOwnerKind,
    version: u64,
    identities: BTreeMap<EgoIdentityId, IdentityState>,
    identity_history: BTreeMap<(EgoIdentityId, u64), crate::EgoIdentity>,
    anchor_history: BTreeMap<(EgoAnchorId, u64), crate::EgoAnchor>,
    identity_deletions: BTreeMap<EgoIdentityId, u64>,
    anchor_deletions: BTreeMap<EgoAnchorId, u64>,
    active_identity_id: Option<EgoIdentityId>,
    personality: Option<EgoPersonality>,
    personality_ego_version: u64,
    anchors: BTreeMap<EgoAnchorId, AnchorState>,
    synthesis: Option<EgoWebSynthesis>,
    synthesis_ego_version: u64,
}

impl EgoStore {
    pub fn reliquary() -> Self {
        Self::new(EgoOwnerKind::Reliquary)
    }
    pub fn phylactery() -> Self {
        Self::new(EgoOwnerKind::Phylactery)
    }

    pub fn empty_replay(&self) -> Self {
        Self::new(self.owner_kind)
    }

    fn new(owner_kind: EgoOwnerKind) -> Self {
        Self {
            owner_kind,
            version: 0,
            identities: BTreeMap::new(),
            identity_history: BTreeMap::new(),
            anchor_history: BTreeMap::new(),
            identity_deletions: BTreeMap::new(),
            anchor_deletions: BTreeMap::new(),
            active_identity_id: None,
            personality: None,
            personality_ego_version: 0,
            anchors: BTreeMap::new(),
            synthesis: None,
            synthesis_ego_version: 0,
        }
    }

    pub fn ingest(&mut self, payload: &[u8]) -> Result<(), EgoError> {
        self.ingest_publication(payload).map(|_| ())
    }

    pub fn ingest_publication(&mut self, payload: &[u8]) -> Result<Option<EgoRecord>, EgoError> {
        // Entity merge bodies use CVAETM02 in the shared physical stream.
        // Bare bodies are unpublished and have no Ego authority.
        if payload.starts_with(b"CVAETM02") {
            return Ok(None);
        }
        let record = if payload.starts_with(b"CVAECR") {
            crate::ego_codec::compact::decode_creation(payload)?.record
        } else if payload.starts_with(b"CVAETM") {
            crate::ego_codec::compact::decode_terminal(payload)?.record
        } else if payload.starts_with(b"CVAEDL") {
            let delta = crate::ego_codec::compact::decode_delta(payload)?;
            let previous = self.delta_predecessor(&delta.changes)?;
            crate::ego_compact_apply::materialize(&previous, &delta)?
        } else if let Some(record) = decode(payload)? {
            record
        } else {
            return Ok(None);
        };
        self.validate_owner_record(&record)?;
        let expected_version = self.next_version()?;
        let actual_version = record_ego_version(&record);
        if actual_version != expected_version {
            return Err(EgoError::InvalidRecord(format!(
                "Ego version sequence expected {expected_version}, got {actual_version}"
            )));
        }
        self.apply(record.clone())?;
        self.version = actual_version;
        Ok(Some(record))
    }

    fn append_apply(
        &mut self,
        container: &mut Container,
        record: EgoRecord,
    ) -> Result<(), EgoError> {
        let payload = self.encode_compact_publication(&record)?;
        container.append(&payload)?;
        let version = record_ego_version(&record);
        self.apply(record)?;
        self.version = version;
        Ok(())
    }

    fn apply(&mut self, record: EgoRecord) -> Result<(), EgoError> {
        let ego_version = record_ego_version(&record);
        match record {
            EgoRecord::Identity {
                id,
                revision,
                deleted,
                activate,
                name,
                text,
                ..
            } => {
                self.apply_identity(ego_version, id, revision, deleted, activate, name, text)?;
                if deleted {
                    self.identity_deletions.insert(id, ego_version);
                }
            }
            EgoRecord::IdentityActive { id, .. } => {
                self.require_live_identity(id)?;
                self.active_identity_id = Some(id);
            }
            EgoRecord::Personality {
                revision,
                source_memory_version,
                text,
                ..
            } => {
                validate_record_revision(
                    self.personality.as_ref().map(|value| value.revision),
                    revision,
                )?;
                self.personality = Some(EgoPersonality {
                    revision,
                    text,
                    source_memory_version,
                });
                self.personality_ego_version = ego_version;
            }
            EgoRecord::Anchor {
                id,
                revision,
                deleted,
                priority,
                text,
                ..
            } => {
                validate_record_revision(
                    self.anchors.get(&id).map(|value| value.revision),
                    revision,
                )?;
                if self.anchors.get(&id).is_some_and(|state| state.deleted) {
                    return Err(crate::owner_lifecycle::LifecycleError::TerminalTombstone(
                        crate::owner_lifecycle::LifecycleTarget::EgoAnchor(id),
                    )
                    .into());
                }
                if deleted {
                    self.anchor_deletions.insert(id, ego_version);
                } else {
                    self.anchor_history.insert(
                        (id, revision),
                        crate::EgoAnchor {
                            id,
                            revision,
                            priority,
                            text: text.clone(),
                        },
                    );
                }
                self.anchors.insert(
                    id,
                    AnchorState {
                        ego_version,
                        revision,
                        deleted,
                        priority,
                        text,
                    },
                );
            }
            EgoRecord::Synthesis {
                revision,
                source_memory_version,
                text,
                ..
            } => {
                validate_record_revision(
                    self.synthesis.as_ref().map(|value| value.revision),
                    revision,
                )?;
                self.synthesis = Some(EgoWebSynthesis {
                    revision,
                    source_memory_version,
                    text,
                });
                self.synthesis_ego_version = ego_version;
            }
        }
        Ok(())
    }

    fn apply_identity(
        &mut self,
        ego_version: u64,
        id: EgoIdentityId,
        revision: u64,
        deleted: bool,
        activate: bool,
        name: String,
        text: String,
    ) -> Result<(), EgoError> {
        validate_record_revision(
            self.identities.get(&id).map(|value| value.revision),
            revision,
        )?;
        if self.identities.get(&id).is_some_and(|state| state.deleted) {
            return Err(crate::owner_lifecycle::LifecycleError::TerminalTombstone(
                crate::owner_lifecycle::LifecycleTarget::EgoIdentity(id),
            )
            .into());
        }
        if deleted && self.active_identity_id == Some(id) && self.live_identity_count() > 1 {
            return Err(EgoError::ActiveIdentityDeletion);
        }
        if deleted && activate {
            return Err(crate::owner_lifecycle::LifecycleError::TerminalTombstone(
                crate::owner_lifecycle::LifecycleTarget::EgoIdentity(id),
            )
            .into());
        }
        let live_after = self.live_identity_count()
            + usize::from(!deleted && !self.identities.contains_key(&id))
            - usize::from(deleted && self.identities.get(&id).is_some_and(|state| !state.deleted));
        let active_after = if activate {
            Some(id)
        } else if deleted && self.active_identity_id == Some(id) {
            None
        } else {
            self.active_identity_id
        };
        if active_after.is_none() && live_after > 0 {
            return Err(EgoError::InvalidRecord(
                "PHY contains identities but no active identity".into(),
            ));
        }
        if !deleted {
            self.identity_history.insert(
                (id, revision),
                crate::EgoIdentity {
                    id,
                    revision,
                    name: name.clone(),
                    text: text.clone(),
                },
            );
        }
        self.identities.insert(
            id,
            IdentityState {
                ego_version,
                revision,
                deleted,
                name,
                text,
            },
        );
        if deleted && self.active_identity_id == Some(id) {
            self.active_identity_id = None;
        }
        if activate {
            self.require_live_identity(id)?;
            self.active_identity_id = Some(id);
        }
        if self.active_identity_id.is_none() && self.live_identity_count() > 0 {
            return Err(EgoError::InvalidRecord(
                "PHY contains identities but no active identity".into(),
            ));
        }
        Ok(())
    }

    fn require_live_identity(&self, id: EgoIdentityId) -> Result<(), EgoError> {
        let state = self
            .identity_lifecycle(id)
            .ok_or(EgoError::NotFound("Identity"))?;
        Ok(state.require_active(crate::owner_lifecycle::LifecycleTarget::EgoIdentity(id))?)
    }

    fn live_identity_count(&self) -> usize {
        self.identities
            .values()
            .filter(|state| !state.deleted)
            .count()
    }

    fn validate_owner_record(&self, record: &EgoRecord) -> Result<(), EgoError> {
        if self.owner_kind == EgoOwnerKind::Reliquary
            && matches!(
                record,
                EgoRecord::Identity { .. }
                    | EgoRecord::IdentityActive { .. }
                    | EgoRecord::Personality { .. }
            )
        {
            return Err(EgoError::InvalidOwnerRecord(
                "REL containers cannot contain Identity or Personality",
            ));
        }
        Ok(())
    }

    fn require_phylactery(&self, message: &'static str) -> Result<(), EgoError> {
        if self.owner_kind != EgoOwnerKind::Phylactery {
            return Err(EgoError::InvalidOwnerRecord(message));
        }
        Ok(())
    }

    fn next_version(&self) -> Result<u64, EgoError> {
        self.version
            .checked_add(1)
            .ok_or_else(|| EgoError::InvalidRecord("Ego version exhausted".into()))
    }
}
