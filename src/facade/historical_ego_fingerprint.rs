//! Ego's accepted owner-local sequence; Ego records have no global ticket.
#[cfg(test)]
#[path = "historical_ego_compact_tests.rs"]
mod compact_tests;
use crate::ego_codec::{EgoRecord, record_ego_version};
use crate::ego_store::EgoStore;
use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::{
    Container, CvaError, EgoAnchor, EgoAnchorId, EgoAnchorPriority, EgoIdentity, EgoIdentityId,
    EgoPersonality, EgoWebSynthesis,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(crate) struct EgoHistory {
    owner: [u8; 16],
    records: Vec<EgoRecord>,
}

pub(crate) struct EgoCut {
    pub(crate) fingerprint: HistoricalFingerprint,
    pub(crate) identities: BTreeMap<EgoIdentityId, EgoIdentity>,
    pub(crate) active_identity: Option<EgoIdentityId>,
    pub(crate) anchors: BTreeMap<EgoAnchorId, EgoAnchor>,
    pub(crate) personality: Option<EgoPersonality>,
    pub(crate) synthesis: Option<EgoWebSynthesis>,
}

impl EgoHistory {
    pub(crate) fn read(container: &Container, store: &EgoStore) -> Result<Self, String> {
        let mut scan = container;
        let mut records = Vec::new();
        let mut replay = store.empty_replay();
        {
            let mut ingest_frame =
                |frame: crate::container::PhysicalFrame| -> Result<(), CvaError> {
                    let payload = frame.payload.as_slice();
                    {
                        if let Some(record) = replay.ingest_publication(payload)? {
                            if record_ego_version(&record) != records.len() as u64 + 1 {
                                return Err(crate::EgoError::InvalidRecord(
                                    "oracle Ego sequence differs".into(),
                                )
                                .into());
                            }
                            records.push(record);
                        }
                        Ok(())
                    }
                };
            let mut cursor = scan.physical_cursor().map_err(|e| e.to_string())?;
            (|| {
                while let Some(frame) = cursor.next_frame()? {
                    ingest_frame(frame)?;
                }
                Ok::<(), CvaError>(())
            })()
        }
        .map_err(|e| e.to_string())?;
        if records.len() as u64 != store.version() {
            return Err("Ego oracle publication count differs from accepted replay".into());
        }
        Ok(Self {
            owner: container
                .owner_uuid()
                .ok_or("Ego oracle needs owner UUID")?,
            records,
        })
    }

    pub(crate) fn at(&self, cut: u64) -> Result<EgoCut, String> {
        if cut > self.records.len() as u64 {
            return Err("Ego cut exceeds retained head".into());
        }
        let mut projection = OwnerProjection::new(self.owner, "ego", cut, None)?;
        let mut identities = BTreeMap::new();
        let mut active_identity = None;
        let mut anchors = BTreeMap::new();
        let mut personality = None;
        let mut synthesis = None;
        for record in self.records.iter().take(cut as usize) {
            let version = record_ego_version(record);
            projection.insert("publication", version.to_be_bytes(), record_value(record))?;
            match record {
                EgoRecord::Identity {
                    ego_version: _,
                    id,
                    revision,
                    deleted,
                    activate,
                    name,
                    text,
                } => {
                    if *deleted {
                        identities.remove(id);
                        if active_identity == Some(*id) {
                            active_identity = None;
                        }
                    } else {
                        identities.insert(
                            *id,
                            EgoIdentity {
                                id: *id,
                                revision: *revision,
                                name: name.clone(),
                                text: text.clone(),
                            },
                        );
                    }
                    if *activate {
                        active_identity = Some(*id);
                    }
                }
                EgoRecord::IdentityActive { ego_version: _, id } => {
                    active_identity = Some(*id);
                }
                EgoRecord::Personality {
                    ego_version: _,
                    revision,
                    source_memory_version,
                    text,
                } => {
                    personality = Some(EgoPersonality {
                        revision: *revision,
                        source_memory_version: *source_memory_version,
                        text: text.clone(),
                    });
                }
                EgoRecord::Anchor {
                    ego_version: _,
                    id,
                    revision,
                    deleted,
                    priority,
                    text,
                } => {
                    if *deleted {
                        anchors.remove(id);
                    } else {
                        anchors.insert(
                            *id,
                            EgoAnchor {
                                id: *id,
                                revision: *revision,
                                priority: *priority,
                                text: text.clone(),
                            },
                        );
                    }
                }
                EgoRecord::Synthesis {
                    ego_version: _,
                    revision,
                    source_memory_version,
                    text,
                } => {
                    synthesis = Some(EgoWebSynthesis {
                        revision: *revision,
                        source_memory_version: *source_memory_version,
                        text: text.clone(),
                    });
                }
            }
        }
        Ok(EgoCut {
            fingerprint: projection.fingerprint()?,
            identities,
            active_identity,
            anchors,
            personality,
            synthesis,
        })
    }
}

fn record_value(record: &EgoRecord) -> Value {
    match record {
        // The old wire carrier has exactly the default named identity's meaning.
        EgoRecord::Identity {
            ego_version,
            id,
            revision,
            deleted,
            activate,
            name,
            text,
        } => json!({
            "kind": "identity", "ego_version": ego_version, "id": id.0,
            "revision": revision, "deleted": deleted, "activate": activate, "name": name, "text": text
        }),
        EgoRecord::IdentityActive { ego_version, id } => {
            json!({"kind": "identity_active", "ego_version": ego_version, "id": id.0})
        }
        EgoRecord::Personality {
            ego_version,
            revision,
            source_memory_version,
            text,
        } => json!({
            "kind": "personality", "ego_version": ego_version, "revision": revision,
            "source_memory_version": source_memory_version, "text": text
        }),
        EgoRecord::Anchor {
            ego_version,
            id,
            revision,
            deleted,
            priority,
            text,
        } => json!({
            "kind": "anchor", "ego_version": ego_version, "id": id.0, "revision": revision,
            "deleted": deleted, "priority": match priority {EgoAnchorPriority::High => "high", EgoAnchorPriority::Normal => "normal", EgoAnchorPriority::Low => "low"}, "text": text
        }),
        EgoRecord::Synthesis {
            ego_version,
            revision,
            source_memory_version,
            text,
        } => json!({
            "kind": "synthesis", "ego_version": ego_version, "revision": revision,
            "source_memory_version": source_memory_version, "text": text
        }),
    }
}

#[path = "historical_ego_fingerprint_tests.rs"]
mod tests;
