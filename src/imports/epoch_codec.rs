use super::{ImportComplete, ImportError, ImportStart};
pub(crate) use reliquary_import_store::codec::ImportEpochRecord;
pub(crate) fn encode_start(value: &ImportStart) -> Result<Vec<u8>, ImportError> {
    Ok(reliquary_import_store::codec::encode_start(value)?)
}
pub(crate) fn encode_complete(value: &ImportComplete) -> Result<Vec<u8>, ImportError> {
    Ok(reliquary_import_store::codec::encode_complete(value)?)
}
pub(crate) fn decode(value: &[u8]) -> Result<Option<ImportEpochRecord>, ImportError> {
    Ok(reliquary_import_store::codec::decode(value)?)
}
