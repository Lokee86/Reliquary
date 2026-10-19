//! Persisted physical-settlement compatibility. No new owner writer emits these formats.
use super::{FreshnessStore, HistoricalImportFreshnessSettlement};
use sha2::{Digest, Sha256};

pub(crate) fn decode(
    payload: &[u8],
) -> Result<Option<HistoricalImportFreshnessSettlement>, String> {
    if payload.starts_with(b"CVAFRS03") {
        return HistoricalImportFreshnessSettlement::decode_legacy_frame(payload).map(Some);
    }
    if payload.starts_with(b"CVAFRS04") || payload.starts_with(b"CVAFRS05") {
        return HistoricalImportFreshnessSettlement::decode(payload).map(Some);
    }
    Ok(None)
}

fn equivalent(
    old: &HistoricalImportFreshnessSettlement,
    new: &HistoricalImportFreshnessSettlement,
) -> bool {
    old.owner == new.owner
        && old.import_id == new.import_id
        && old.base_cut == new.base_cut
        && old.settlement_cut == new.settlement_cut
        && old.global_version == new.global_version
        && old.archive_version == new.archive_version
        && old.graph_version == new.graph_version
        && old.memory_graph_version == new.memory_graph_version
        && old.cohort == new.cohort
        && old.records == new.records
        && old.superseded == new.superseded
        && old.corpus_digest == new.corpus_digest
}

fn visit(
    path: &std::path::Path,
    mut visitor: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let mut scan = crate::Container::open_read_only_for_scan(path).map_err(|e| e.to_string())?;
    let mut failure = None;
    scan.visit_payloads::<crate::ContainerError>(|_, payload| {
        if failure.is_none() {
            failure = visitor(payload).err();
        }
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    failure.map_or(Ok(()), Err)
}

impl FreshnessStore {
    pub(super) fn compatibility_absorbs_event(
        &self,
        settlement: &HistoricalImportFreshnessSettlement,
        event_id: &str,
    ) -> Result<bool, String> {
        if settlement.epoch.is_some() {
            return self.event_accepted_through(event_id, settlement.input_bytes);
        }
        Ok(false)
    }

    /// Find a semantic successor before replaying obsolete physical evidence.
    /// Its complete identity/event validation still runs at V6 ingestion.
    pub(super) fn has_semantic_successor(&self, payload: &[u8]) -> Result<bool, String> {
        let Some(old) = decode(payload)? else {
            return Ok(false);
        };
        let Some(path) = &self.history_path else {
            return Ok(false);
        };
        let digest: [u8; 32] = Sha256::digest(payload).into();
        let mut found = false;
        visit(path, |bytes| {
            if bytes.starts_with(b"CVAFRS06") {
                let new = HistoricalImportFreshnessSettlement::decode(bytes)?;
                if new.semantic.as_ref().and_then(|s| s.predecessor_digest) == Some(digest) {
                    if !equivalent(&old, &new) {
                        return Err("semantic normalization changes accepted legacy state".into());
                    }
                    found = true;
                }
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?;
        Ok(found)
    }

    pub(super) fn validate_semantic_predecessor(
        &self,
        new: &HistoricalImportFreshnessSettlement,
    ) -> Result<(), String> {
        let Some(digest) = new.semantic.as_ref().and_then(|s| s.predecessor_digest) else {
            return Ok(());
        };
        let path = self
            .history_path
            .as_ref()
            .ok_or("normalization needs retained predecessor")?;
        let mut found = false;
        visit(path, |payload| {
            if Sha256::digest(payload).as_slice() == digest
                && let Some(old) = decode(payload)?
            {
                if !equivalent(&old, new) {
                    return Err("normalization predecessor differs from accepted state".into());
                }
                found = true;
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?;
        if !found {
            return Err("normalization predecessor digest is unavailable".into());
        }
        Ok(())
    }
}
