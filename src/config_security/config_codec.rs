use crate::ConfigError;
use reliquary_switchboard::{ConfigDocument, ConfigObject};
use std::collections::BTreeMap;

pub(crate) type RawConfigObject = ConfigObject;

pub(crate) fn encode_config(
    objects: &BTreeMap<String, RawConfigObject>,
) -> Result<Vec<u8>, ConfigError> {
    let mut document = ConfigDocument::new();
    for (key, object) in objects {
        document.insert(key.clone(), object.clone());
    }
    reliquary_switchboard::encode_config_document(&document).map_err(ConfigError::from)
}

pub(crate) fn decode_config(
    bytes: &[u8],
) -> Result<BTreeMap<String, RawConfigObject>, ConfigError> {
    let document =
        reliquary_switchboard::decode_config_document(bytes).map_err(ConfigError::from)?;
    Ok(document.objects().clone())
}
