//! Current-document bases for Ego-owned delta replay. Sequence metadata comes
//! from accepted publications; it is never synthesized from another clock.
use super::EgoStore;
use crate::EgoError;
use crate::ego_codec::EgoRecord;
use crate::ego_compact_contract::{
    EgoChanges, EgoDelta, EgoDocumentKey, EgoTerminal, document_key, is_terminal, normalize,
    revision,
};

impl EgoStore {
    pub(crate) fn validate_storage_version(
        &self,
        version: crate::FormatVersion,
    ) -> Result<(), EgoError> {
        if version != (crate::FormatVersion { major: 2, minor: 0 }) && self.compact_seen {
            return Err(EgoError::InvalidRecord(
                "compact Ego publications require V2 storage".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn delta_predecessor(&self, changes: &EgoChanges) -> Result<EgoRecord, EgoError> {
        self.document_predecessor(changes.document_key())
    }

    pub(crate) fn encode_compact_publication(
        &self,
        record: &EgoRecord,
    ) -> Result<Vec<u8>, EgoError> {
        let record = normalize(record.clone());
        self.validate_owner_record(&record)?;
        if crate::ego_codec::record_ego_version(&record) != self.next_version()? {
            return Err(EgoError::InvalidRecord(
                "compact Ego writer sequence differs".into(),
            ));
        }
        if is_terminal(&record) {
            crate::ego_codec::compact::encode_terminal(&EgoTerminal::new(record)?)
        } else if revision(&record) == Some(1) || matches!(record, EgoRecord::IdentityActive { .. })
        {
            crate::ego_codec::compact::encode_initial(&record)
        } else {
            let previous = self.document_predecessor(document_key(&record).ok_or_else(missing)?)?;
            crate::ego_codec::compact::encode_delta(&EgoDelta::between(&previous, &record)?)
        }
    }

    fn document_predecessor(&self, key: EgoDocumentKey) -> Result<EgoRecord, EgoError> {
        Ok(match key {
            EgoDocumentKey::Identity(id) => {
                let state = self.identities.get(&id).ok_or_else(missing)?;
                EgoRecord::Identity {
                    ego_version: state.ego_version,
                    id,
                    revision: state.revision,
                    deleted: state.deleted,
                    activate: false,
                    name: state.name.clone(),
                    text: state.text.clone(),
                }
            }
            EgoDocumentKey::Anchor(id) => {
                let state = self.anchors.get(&id).ok_or_else(missing)?;
                EgoRecord::Anchor {
                    ego_version: state.ego_version,
                    id,
                    revision: state.revision,
                    deleted: state.deleted,
                    priority: state.priority,
                    text: state.text.clone(),
                }
            }
            EgoDocumentKey::Personality => {
                let state = self.personality.as_ref().ok_or_else(missing)?;
                EgoRecord::Personality {
                    ego_version: self.personality_ego_version,
                    revision: state.revision,
                    source_memory_version: state.source_memory_version,
                    text: state.text.clone(),
                }
            }
            EgoDocumentKey::Synthesis => {
                let state = self.synthesis.as_ref().ok_or_else(missing)?;
                EgoRecord::Synthesis {
                    ego_version: self.synthesis_ego_version,
                    revision: state.revision,
                    source_memory_version: state.source_memory_version,
                    text: state.text.clone(),
                }
            }
        })
    }
}

fn missing() -> EgoError {
    EgoError::InvalidRecord("compact Ego delta has no predecessor".into())
}
