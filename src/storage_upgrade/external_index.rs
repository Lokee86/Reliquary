//! Only current derived index state survives; CVAXMEM1 proof is untouched.
use super::StorageUpgradeError;
use crate::{Container, Cva, FileKind};

pub(super) fn prepare(source: &Container) -> Result<Vec<Vec<u8>>, StorageUpgradeError> {
    if source
        .identity()
        .is_none_or(|id| id.file_kind != FileKind::Reliquary)
    {
        return Ok(Vec::new());
    }
    let rel = Cva::open_v1_read_only(source.path())?;
    let mut payloads = Vec::new();
    if rel.external_index.damaged {
        payloads.push(crate::external_source::store::discard_payload());
    }
    for state in rel.external_index.sources.values() {
        payloads.push(
            crate::external_source::store::encode(state)
                .map_err(|error| StorageUpgradeError::Migration(error.to_string()))?,
        );
    }
    Ok(payloads)
}
