use crate::ConfigError;
use std::path::Path;

pub(crate) fn read_config_file(path: &Path) -> Result<Vec<u8>, ConfigError> {
    let document = reliquary_switchboard::read_config_document(path).map_err(ConfigError::from)?;
    reliquary_switchboard::encode_config_document(&document).map_err(ConfigError::from)
}

pub(crate) fn replace_config_file(path: &Path, bytes: &[u8]) -> Result<(), ConfigError> {
    let document =
        reliquary_switchboard::decode_config_document(bytes).map_err(ConfigError::from)?;
    reliquary_switchboard::replace_config_document(path, &document).map_err(ConfigError::from)
}
