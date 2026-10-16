use crate::{
    ConfigError, CredentialId, DecisionModelEndpoint, EmbeddingModelEndpoint, GeneralModelEndpoint,
    ModelProvider, ModelReasoningEffort, VectorNormalization,
};

pub(crate) const GENERAL_MODEL_KEY: &str = "models.general";
pub(crate) const INSOMNIA_MODEL_KEY: &str = "models.insomnia";
pub(crate) const INSOMNIA_METADATA_MODEL_KEY: &str = "models.insomnia_metadata";
pub(crate) const ENTITY_EXTRACTION_MODEL_KEY: &str = "models.entity_extraction";
pub(crate) const ENTITY_RESOLUTION_DECISION_MODEL_KEY: &str = "models.entity_resolution_decision";
pub(crate) const MEMORY_RETRIEVAL_DECISION_MODEL_KEY: &str = "models.memory_retrieval_decision";
pub(crate) const ENTITY_RESOLUTION_MODEL_KEY: &str = "models.entity_resolution";
pub(crate) const CHRONOS_MODEL_KEY: &str = "models.chronos";
pub(crate) const DREAM_MODEL_KEY: &str = "models.dream";
pub(crate) const EMBEDDING_MODEL_KEY: &str = "models.embedding";
pub(crate) const GENERAL_MODEL_SCHEMA_V3: u16 = 3;
pub(crate) const DECISION_MODEL_SCHEMA_V1: u16 = 1;
pub(crate) const EMBEDDING_MODEL_SCHEMA_V2: u16 = 2;

pub(crate) fn encode_general(endpoint: &GeneralModelEndpoint) -> Result<Vec<u8>, ConfigError> {
    let mut bytes = Vec::new();
    bytes.push(endpoint.provider.tag());
    bytes.push(
        endpoint
            .reasoning_effort
            .map(ModelReasoningEffort::tag)
            .unwrap_or(0),
    );
    bytes.extend_from_slice(&[0; 2]);
    encode_string(&mut bytes, &endpoint.model)?;
    encode_string(&mut bytes, endpoint.url.as_deref().unwrap_or(""))?;
    encode_string(&mut bytes, endpoint.credential_id.as_str())?;
    Ok(bytes)
}

pub(crate) fn decode_general_v3(bytes: &[u8]) -> Result<GeneralModelEndpoint, ConfigError> {
    let header = bytes.get(..4).ok_or(ConfigError::InvalidModelSwitchboard)?;
    if header[2..4] != [0; 2] {
        return Err(ConfigError::InvalidModelSwitchboard);
    }
    let provider =
        ModelProvider::from_tag(header[0]).ok_or(ConfigError::InvalidModelSwitchboard)?;
    let reasoning_effort = if header[1] == 0 {
        None
    } else {
        Some(
            ModelReasoningEffort::from_tag(header[1])
                .ok_or(ConfigError::InvalidModelSwitchboard)?,
        )
    };
    decode_general_body(bytes, provider, reasoning_effort)
}

pub(crate) fn decode_general_v2(bytes: &[u8]) -> Result<GeneralModelEndpoint, ConfigError> {
    let header = bytes.get(..4).ok_or(ConfigError::InvalidModelSwitchboard)?;
    if header[1..4] != [0; 3] {
        return Err(ConfigError::InvalidModelSwitchboard);
    }
    let provider =
        ModelProvider::from_tag(header[0]).ok_or(ConfigError::InvalidModelSwitchboard)?;
    decode_general_body(bytes, provider, None)
}

fn decode_general_body(
    bytes: &[u8],
    provider: ModelProvider,
    reasoning_effort: Option<ModelReasoningEffort>,
) -> Result<GeneralModelEndpoint, ConfigError> {
    let mut cursor = 4;
    let model = decode_string(bytes, &mut cursor)?;
    let url = decode_string(bytes, &mut cursor)?;
    let credential_id = CredentialId::new(decode_string(bytes, &mut cursor)?)?;
    if cursor != bytes.len() {
        return Err(ConfigError::InvalidModelSwitchboard);
    }
    Ok(GeneralModelEndpoint {
        provider,
        model,
        url: (!url.is_empty()).then_some(url),
        credential_id,
        reasoning_effort,
    })
}

pub(crate) fn encode_decision(endpoint: &DecisionModelEndpoint) -> Result<Vec<u8>, ConfigError> {
    let mut bytes = Vec::new();
    encode_string(&mut bytes, &endpoint.model)?;
    encode_string(&mut bytes, &endpoint.url)?;
    encode_string(&mut bytes, endpoint.credential_id.as_str())?;
    Ok(bytes)
}

pub(crate) fn decode_decision(bytes: &[u8]) -> Result<DecisionModelEndpoint, ConfigError> {
    let mut cursor = 0;
    let model = decode_string(bytes, &mut cursor)?;
    let url = decode_string(bytes, &mut cursor)?;
    let credential_id = CredentialId::new(decode_string(bytes, &mut cursor)?)?;
    if cursor != bytes.len() {
        return Err(ConfigError::InvalidModelSwitchboard);
    }
    Ok(DecisionModelEndpoint {
        model,
        url,
        credential_id,
    })
}

pub(crate) fn encode_embedding(endpoint: &EmbeddingModelEndpoint) -> Result<Vec<u8>, ConfigError> {
    let mut bytes = Vec::new();
    bytes.push(endpoint.provider.tag());
    bytes.push(endpoint.normalization.tag());
    bytes.extend_from_slice(&[0; 2]);
    bytes.extend_from_slice(&endpoint.dimensions.to_le_bytes());
    encode_string(&mut bytes, &endpoint.model)?;
    encode_string(&mut bytes, endpoint.url.as_deref().unwrap_or(""))?;
    encode_string(&mut bytes, endpoint.credential_id.as_str())?;
    Ok(bytes)
}

pub(crate) fn decode_embedding(bytes: &[u8]) -> Result<EmbeddingModelEndpoint, ConfigError> {
    let header = bytes.get(..8).ok_or(ConfigError::InvalidModelSwitchboard)?;
    if header[2..4] != [0; 2] {
        return Err(ConfigError::InvalidModelSwitchboard);
    }
    let provider =
        ModelProvider::from_tag(header[0]).ok_or(ConfigError::InvalidModelSwitchboard)?;
    let normalization =
        VectorNormalization::from_tag(header[1]).ok_or(ConfigError::InvalidModelSwitchboard)?;
    let dimensions = u32::from_le_bytes(header[4..8].try_into().unwrap());
    let mut cursor = 8;
    let model = decode_string(bytes, &mut cursor)?;
    let url = decode_string(bytes, &mut cursor)?;
    let credential_id = CredentialId::new(decode_string(bytes, &mut cursor)?)?;
    if cursor != bytes.len() {
        return Err(ConfigError::InvalidModelSwitchboard);
    }
    Ok(EmbeddingModelEndpoint {
        provider,
        model,
        url: (!url.is_empty()).then_some(url),
        credential_id,
        dimensions,
        normalization,
    })
}

fn encode_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), ConfigError> {
    let len = u32::try_from(value.len()).map_err(|_| ConfigError::InvalidModelSwitchboard)?;
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn decode_string(bytes: &[u8], cursor: &mut usize) -> Result<String, ConfigError> {
    let length_end = cursor
        .checked_add(4)
        .ok_or(ConfigError::InvalidModelSwitchboard)?;
    let len_bytes = bytes
        .get(*cursor..length_end)
        .ok_or(ConfigError::InvalidModelSwitchboard)?;
    let len = u32::from_le_bytes(len_bytes.try_into().unwrap()) as usize;
    let value_end = length_end
        .checked_add(len)
        .ok_or(ConfigError::InvalidModelSwitchboard)?;
    let value = std::str::from_utf8(
        bytes
            .get(length_end..value_end)
            .ok_or(ConfigError::InvalidModelSwitchboard)?,
    )
    .map_err(|_| ConfigError::InvalidModelSwitchboard)?
    .to_owned();
    *cursor = value_end;
    Ok(value)
}
