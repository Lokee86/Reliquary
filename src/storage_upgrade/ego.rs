//! Ego-owned V1 publication translation; no semantic sequence is allocated.
use super::StorageUpgradeError;
use crate::{Container, FileKind, ObjectRef};
use std::collections::BTreeMap;

pub(super) fn prepare(
    source: &Container,
    memory_head: u64,
) -> Result<BTreeMap<ObjectRef, Vec<u8>>, StorageUpgradeError> {
    let mut ego = match source.identity().map(|identity| identity.file_kind) {
        Some(FileKind::Reliquary) => crate::ego_store::EgoStore::reliquary(),
        Some(FileKind::Phylactery) => crate::ego_store::EgoStore::phylactery(),
        _ => return Err(StorageUpgradeError::MissingOwnerIdentity),
    };
    let mut encoded = BTreeMap::new();
    let scanned = Container::open_scanned_version(
        source.path(),
        Some(crate::FormatVersion { major: 1, minor: 0 }),
        |object, payload, _| -> Result<(), StorageUpgradeError> {
            if crate::ego_codec::compact::is_compact_record(payload)? {
                return Err(StorageUpgradeError::Migration(
                    "V1 input contains compact Ego publication".into(),
                ));
            }
            if let Some(record) = crate::ego_codec::decode(payload)? {
                let compact = ego.encode_compact_publication(&record)?;
                ego.ingest(payload)?;
                encoded.insert(object, compact);
            }
            Ok(())
        },
    )?;
    ego.validate_storage_version(scanned.version())?;
    ego.validate_memory_version(memory_head)?;
    Ok(encoded)
}
