//! Disposable receipt lookup during a single validated reopen scan.
//! No lifetime ID table is retained in RAM or promoted to owner authority.
//! The directory is fresh for each scan and removed when its last staged view drops.
use super::{JournalEntry, decode, encode, for_each_event, use_fields};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Debug)]
pub(super) struct RebuildReceipts {
    directory: PathBuf,
}
impl RebuildReceipts {
    pub(super) fn new() -> Self {
        Self {
            directory: std::env::temp_dir().join(format!(
                "reliquary-freshness-rebuild-{}",
                uuid::Uuid::new_v4()
            )),
        }
    }
    fn path(&self, kind: &str, id: &str) -> PathBuf {
        self.directory
            .join(format!("{kind}-{:x}", Sha256::digest(id.as_bytes())))
    }
    pub(super) fn find(&self, kind: &str, id: &str) -> Result<Option<JournalEntry>, String> {
        let bytes = match std::fs::read(self.path(kind, id)) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        let (_, entry) = decode(&bytes)?;
        let matches = match kind {
            "event" => super::event_in(&entry, id).is_some(),
            "use" => use_fields(&entry).is_some_and(|(key, _, _, _)| key == id),
            _ => false,
        };
        if !matches {
            return Err("Freshness rebuild receipt key mismatch".into());
        }
        Ok(Some(entry))
    }
    pub(super) fn remember(&self, entry: &JournalEntry) -> Result<(), String> {
        std::fs::create_dir_all(&self.directory).map_err(|e| e.to_string())?;
        for_each_event(entry, &mut |event| {
            let JournalEntry::Event { event_id, .. } = event else {
                unreachable!()
            };
            std::fs::write(self.path("event", event_id), encode([0; 16], event))
                .map_err(|e| e.to_string())
        })?;
        if let Some((id, _, _, _)) = use_fields(entry) {
            std::fs::write(self.path("use", id), encode([0; 16], entry))
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
impl Drop for RebuildReceipts {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
