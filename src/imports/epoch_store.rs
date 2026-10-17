use super::epoch_codec::{ImportEpochRecord, decode, encode_complete, encode_start};
use super::{ImportComplete, ImportEpochStatus, ImportError, ImportStart};
use crate::Container;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct ImportEpochStore {
    starts: BTreeMap<String, ImportStart>,
    completions: BTreeMap<String, ImportComplete>,
    open: Option<String>,
}

impl ImportEpochStore {
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), ImportError> {
        let Some(record) = decode(payload)? else {
            return Ok(());
        };
        match record {
            ImportEpochRecord::Start(start) => {
                self.validate_start(&start)?;
                if self.starts.contains_key(&start.import_id) {
                    return Err(ImportError::Epoch("duplicate import start"));
                }
                if self.open.is_some() {
                    return Err(ImportError::Epoch("overlapping import epochs"));
                }
                self.open = Some(start.import_id.clone());
                self.starts.insert(start.import_id.clone(), start);
            }
            ImportEpochRecord::Complete(complete) => {
                self.validate_complete(&complete)?;
                self.completions
                    .insert(complete.import_id.clone(), complete);
                self.open = None;
            }
        }
        Ok(())
    }

    pub(crate) fn append_start(
        &mut self,
        container: &mut Container,
        start: ImportStart,
    ) -> Result<(), ImportError> {
        if self.open.is_some() {
            return Err(ImportError::Epoch("another import epoch is open"));
        }
        if self.starts.contains_key(&start.import_id) {
            return Err(ImportError::Epoch("import start already exists"));
        }
        self.validate_start(&start)?;
        if container.owner_uuid() != Some(start.owner_uuid) {
            return Err(ImportError::Epoch("import epoch owner mismatch"));
        }
        container.append(&encode_start(&start)?)?;
        container.set_write_admission_closed(true);
        self.open = Some(start.import_id.clone());
        self.starts.insert(start.import_id.clone(), start);
        Ok(())
    }

    pub(crate) fn append_complete(
        &mut self,
        container: &mut Container,
        complete: ImportComplete,
    ) -> Result<(), ImportError> {
        if let Some(existing) = self.completions.get(&complete.import_id) {
            return if existing == &complete {
                Ok(())
            } else {
                Err(ImportError::Epoch("conflicting import completion"))
            };
        }
        self.validate_complete(&complete)?;
        container.append(&encode_complete(&complete)?)?;
        self.completions
            .insert(complete.import_id.clone(), complete);
        self.open = None;
        container.set_write_admission_closed(false);
        Ok(())
    }

    pub(crate) fn open(&self) -> Option<&ImportStart> {
        self.open.as_ref().and_then(|id| self.starts.get(id))
    }

    pub(crate) fn status(&self, import_id: &str) -> Option<ImportEpochStatus> {
        let start = self.starts.get(import_id)?.clone();
        Some(match self.completions.get(import_id) {
            Some(complete) => ImportEpochStatus::Completed {
                start,
                complete: complete.clone(),
            },
            None => ImportEpochStatus::Open(start),
        })
    }

    pub(crate) fn statuses(&self) -> Vec<ImportEpochStatus> {
        self.starts
            .keys()
            .filter_map(|id| self.status(id))
            .collect()
    }

    pub(crate) fn validate_receipts(
        &self,
        receipts: &BTreeMap<String, super::ImportReceipt>,
    ) -> Result<(), ImportError> {
        for (id, complete) in &self.completions {
            let start = &self.starts[id];
            let receipt = receipts
                .get(&start.import_key)
                .ok_or(ImportError::Epoch("completed import lacks receipt"))?;
            if super::prepare::digest(receipt)? != complete.receipt_digest {
                return Err(ImportError::Epoch("import receipt digest mismatch"));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_owner(&self, owner_uuid: Option<[u8; 16]>) -> Result<(), ImportError> {
        for start in self.starts.values() {
            if Some(start.owner_uuid) != owner_uuid {
                return Err(ImportError::Epoch("import epoch owner mismatch"));
            }
        }
        Ok(())
    }

    fn validate_start(&self, start: &ImportStart) -> Result<(), ImportError> {
        if start.import_id != import_id(start.owner_uuid, &start.import_key) {
            return Err(ImportError::Epoch("invalid deterministic import ID"));
        }
        if !is_canonical_import_key(&start.import_key)
            || !valid_source_text(&start.source_namespace)
            || !valid_source_text(&start.source_format)
        {
            return Err(ImportError::Epoch("incomplete import start identity"));
        }
        Ok(())
    }

    fn validate_complete(&self, complete: &ImportComplete) -> Result<(), ImportError> {
        let start = self
            .starts
            .get(&complete.import_id)
            .ok_or(ImportError::Epoch("completion without start"))?;
        if self.open.as_deref() != Some(&complete.import_id)
            || start.owner_uuid != complete.owner_uuid
        {
            return Err(ImportError::Epoch("invalid import completion ordering"));
        }
        if self.completions.contains_key(&complete.import_id) {
            return Err(ImportError::Epoch("duplicate import completion"));
        }
        if complete.start_digest != super::prepare::digest(start)? {
            return Err(ImportError::Epoch("import start digest mismatch"));
        }
        if complete.receipt_digest.is_empty()
            || complete.historical_freshness_settlement_digest.is_empty()
        {
            return Err(ImportError::Epoch(
                "import completion evidence is incomplete",
            ));
        }
        if !watermarks_monotone(start.pre, complete.post)
            || complete.completed_at_ns < start.started_at_ns
        {
            return Err(ImportError::Epoch("import completion cut is not monotone"));
        }
        Ok(())
    }
}

pub(crate) fn import_id(owner_uuid: [u8; 16], import_key: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest([owner_uuid.as_slice(), import_key.as_bytes()].concat())
    )
}

fn is_canonical_import_key(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_source_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && !value.contains('\0')
}

#[cfg(test)]
#[path = "epoch_store_tests.rs"]
mod tests;

fn watermarks_monotone(pre: super::ImportWatermarks, post: super::ImportWatermarks) -> bool {
    post.global_version >= pre.global_version
        && post.archive_version >= pre.archive_version
        && post.memory_version >= pre.memory_version
        && post.entity_version >= pre.entity_version
        && post.relationship_version >= pre.relationship_version
        && post.graph_version >= pre.graph_version
        && post.memory_graph_version >= pre.memory_graph_version
        && post.vector_version >= pre.vector_version
}
