//! Replay of the owner-bound processing plan within a native import epoch.
use super::{ImportEpochStore, decode, encode};
use reliquary_container::Container;
use serde::{Deserialize, Serialize};

pub(crate) const MAGIC: &[u8; 8] = b"RELIPLN1";
const MAX_FRAME_BYTES: usize = 2048;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImportProcessingBinding {
    pub policy: [u8; 32],
    pub destination: Option<[u8; 16]>,
    pub profile: Option<[u8; 32]>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    owner: [u8; 16],
    pub(super) import_id: String,
    binding: ImportProcessingBinding,
}

pub(crate) fn processing_marker_id(payload: &[u8]) -> Result<String, String> {
    Ok(decode_record(payload)?.import_id)
}

fn decode_record(payload: &[u8]) -> Result<Record, String> {
    if payload.len() > MAX_FRAME_BYTES {
        return Err("import processing plan exceeds its size bound".into());
    }
    decode(payload, MAGIC)
}

impl ImportEpochStore {
    pub(crate) fn processing_binding(&self, id: &str) -> Option<ImportProcessingBinding> {
        self.processing.get(id).copied()
    }

    pub(crate) fn append_processing_binding(
        &mut self,
        container: &mut Container,
        import_id: &str,
        binding: ImportProcessingBinding,
    ) -> Result<(), String> {
        self.check_owner(container)?;
        let record = Record {
            owner: container.owner_id(),
            import_id: import_id.to_owned(),
            binding,
        };
        self.validate_processing_binding(&record)?;
        let bytes = encode(MAGIC, &record)?;
        container
            .append(&bytes)
            .map_err(|error| error.to_string())?;
        container.sync().map_err(|error| error.to_string())?;
        self.processing.insert(record.import_id, record.binding);
        Ok(())
    }

    pub(super) fn ingest_processing_binding(&mut self, payload: &[u8]) -> Result<(), String> {
        let record = decode_record(payload)?;
        self.validate_processing_binding(&record)?;
        self.processing.insert(record.import_id, record.binding);
        Ok(())
    }

    fn validate_processing_binding(&self, record: &Record) -> Result<(), String> {
        let binding = record.binding;
        if !self.open_epoch().is_some_and(|start| {
            start.format_version == 2
                && start.import_id == record.import_id
                && start.owner_uuid == record.owner
        }) || self.settlement(&record.import_id).is_some()
            || binding.destination == Some(record.owner)
            || binding.policy == [0; 32]
            || binding.profile == Some([0; 32])
        {
            return Err("invalid owner-scoped import processing plan".into());
        }
        if let Some(previous) = self.processing_binding(&record.import_id)
            && (previous == binding
                || previous.policy != binding.policy
                || previous.destination.is_some() && previous.destination != binding.destination
                || previous.profile.is_some() && previous.destination != binding.destination
                || previous.profile.is_some() && previous.profile != binding.profile)
        {
            return Err("conflicting import processing plan transition".into());
        }
        Ok(())
    }
}
