use crate::{
    CredentialId, DecisionModelEndpoint, EmbeddingModelEndpoint, EmbeddingNormalization,
    GeneralModelEndpoint, ModelConfigError, ModelProvider, ModelReasoningEffort,
};

pub const GENERAL_MODEL_KEY: &str = "models.general";
pub const INSOMNIA_MODEL_KEY: &str = "models.insomnia";
pub const INSOMNIA_METADATA_MODEL_KEY: &str = "models.insomnia_metadata";
pub const ENTITY_EXTRACTION_MODEL_KEY: &str = "models.entity_extraction";
pub const GENERAL_DECISION_MODEL_KEY: &str = "models.general_decision";
pub const ENTITY_RESOLUTION_DECISION_MODEL_KEY: &str = "models.entity_resolution_decision";
pub const MEMORY_RETRIEVAL_DECISION_MODEL_KEY: &str = "models.memory_retrieval_decision";
pub const ENTITY_RESOLUTION_MODEL_KEY: &str = "models.entity_resolution";
pub const DREAM_MODEL_KEY: &str = "models.dream";
pub const EMBEDDING_MODEL_KEY: &str = "models.embedding";
pub const GENERAL_MODEL_SCHEMA_V3: u16 = 3;
pub const DECISION_MODEL_SCHEMA_V1: u16 = 1;
pub const EMBEDDING_MODEL_SCHEMA_V2: u16 = 2;

pub fn encode_general(endpoint: &GeneralModelEndpoint) -> Result<Vec<u8>, ModelConfigError> {
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

pub fn decode_general_v3(bytes: &[u8]) -> Result<GeneralModelEndpoint, ModelConfigError> {
    let header = bytes
        .get(..4)
        .ok_or(ModelConfigError::InvalidModelSwitchboard)?;
    if header[2..4] != [0; 2] {
        return Err(ModelConfigError::InvalidModelSwitchboard);
    }
    let provider =
        ModelProvider::from_tag(header[0]).ok_or(ModelConfigError::InvalidModelSwitchboard)?;
    let reasoning_effort = if header[1] == 0 {
        None
    } else {
        Some(
            ModelReasoningEffort::from_tag(header[1])
                .ok_or(ModelConfigError::InvalidModelSwitchboard)?,
        )
    };
    decode_general_body(bytes, provider, reasoning_effort)
}

pub fn decode_general_v2(bytes: &[u8]) -> Result<GeneralModelEndpoint, ModelConfigError> {
    let header = bytes
        .get(..4)
        .ok_or(ModelConfigError::InvalidModelSwitchboard)?;
    if header[1..4] != [0; 3] {
        return Err(ModelConfigError::InvalidModelSwitchboard);
    }
    let provider =
        ModelProvider::from_tag(header[0]).ok_or(ModelConfigError::InvalidModelSwitchboard)?;
    decode_general_body(bytes, provider, None)
}

fn decode_general_body(
    bytes: &[u8],
    provider: ModelProvider,
    reasoning_effort: Option<ModelReasoningEffort>,
) -> Result<GeneralModelEndpoint, ModelConfigError> {
    let mut cursor = 4;
    let model = decode_string(bytes, &mut cursor)?;
    let url = decode_string(bytes, &mut cursor)?;
    let credential_id = CredentialId::new(decode_string(bytes, &mut cursor)?)?;
    if cursor != bytes.len() {
        return Err(ModelConfigError::InvalidModelSwitchboard);
    }
    Ok(GeneralModelEndpoint {
        provider,
        model,
        url: (!url.is_empty()).then_some(url),
        credential_id,
        reasoning_effort,
    })
}

pub fn encode_decision(endpoint: &DecisionModelEndpoint) -> Result<Vec<u8>, ModelConfigError> {
    let mut bytes = Vec::new();
    encode_string(&mut bytes, &endpoint.model)?;
    encode_string(&mut bytes, &endpoint.url)?;
    encode_string(&mut bytes, endpoint.credential_id.as_str())?;
    Ok(bytes)
}

pub fn decode_decision(bytes: &[u8]) -> Result<DecisionModelEndpoint, ModelConfigError> {
    let mut cursor = 0;
    let model = decode_string(bytes, &mut cursor)?;
    let url = decode_string(bytes, &mut cursor)?;
    let credential_id = CredentialId::new(decode_string(bytes, &mut cursor)?)?;
    if cursor != bytes.len() {
        return Err(ModelConfigError::InvalidModelSwitchboard);
    }
    Ok(DecisionModelEndpoint {
        model,
        url,
        credential_id,
    })
}

pub fn encode_embedding(endpoint: &EmbeddingModelEndpoint) -> Result<Vec<u8>, ModelConfigError> {
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

pub fn decode_embedding(bytes: &[u8]) -> Result<EmbeddingModelEndpoint, ModelConfigError> {
    let header = bytes
        .get(..8)
        .ok_or(ModelConfigError::InvalidModelSwitchboard)?;
    if header[2..4] != [0; 2] {
        return Err(ModelConfigError::InvalidModelSwitchboard);
    }
    let provider =
        ModelProvider::from_tag(header[0]).ok_or(ModelConfigError::InvalidModelSwitchboard)?;
    let normalization = EmbeddingNormalization::from_tag(header[1])
        .ok_or(ModelConfigError::InvalidModelSwitchboard)?;
    let dimensions = u32::from_le_bytes(header[4..8].try_into().unwrap());
    let mut cursor = 8;
    let model = decode_string(bytes, &mut cursor)?;
    let url = decode_string(bytes, &mut cursor)?;
    let credential_id = CredentialId::new(decode_string(bytes, &mut cursor)?)?;
    if cursor != bytes.len() {
        return Err(ModelConfigError::InvalidModelSwitchboard);
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

fn encode_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), ModelConfigError> {
    let len = u32::try_from(value.len()).map_err(|_| ModelConfigError::InvalidModelSwitchboard)?;
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn decode_string(bytes: &[u8], cursor: &mut usize) -> Result<String, ModelConfigError> {
    let length_end = cursor
        .checked_add(4)
        .ok_or(ModelConfigError::InvalidModelSwitchboard)?;
    let len_bytes = bytes
        .get(*cursor..length_end)
        .ok_or(ModelConfigError::InvalidModelSwitchboard)?;
    let len = u32::from_le_bytes(len_bytes.try_into().unwrap()) as usize;
    let value_end = length_end
        .checked_add(len)
        .ok_or(ModelConfigError::InvalidModelSwitchboard)?;
    let value = std::str::from_utf8(
        bytes
            .get(length_end..value_end)
            .ok_or(ModelConfigError::InvalidModelSwitchboard)?,
    )
    .map_err(|_| ModelConfigError::InvalidModelSwitchboard)?
    .to_owned();
    *cursor = value_end;
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_object_bytes_preserve_current_schemas_and_general_v2_read() {
        let id = CredentialId::new("k").unwrap();
        let general = GeneralModelEndpoint {
            provider: ModelProvider::OpenAiCodex,
            model: "m".into(),
            url: None,
            credential_id: id.clone(),
            reasoning_effort: None,
        };
        let general_bytes = vec![1, 0, 0, 0, 1, 0, 0, 0, b'm', 0, 0, 0, 0, 1, 0, 0, 0, b'k'];
        assert_eq!(encode_general(&general).unwrap(), general_bytes);
        assert_eq!(decode_general_v3(&general_bytes).unwrap(), general);
        assert_eq!(decode_general_v2(&general_bytes).unwrap(), general);
        let decision = DecisionModelEndpoint {
            model: "m".into(),
            url: "u".into(),
            credential_id: id.clone(),
        };
        let decision_bytes = vec![1, 0, 0, 0, b'm', 1, 0, 0, 0, b'u', 1, 0, 0, 0, b'k'];
        assert_eq!(encode_decision(&decision).unwrap(), decision_bytes);
        assert_eq!(decode_decision(&decision_bytes).unwrap(), decision);
        let embedding = EmbeddingModelEndpoint {
            provider: ModelProvider::OpenAiReady,
            model: "m".into(),
            url: Some("u".into()),
            credential_id: id,
            dimensions: 3,
            normalization: EmbeddingNormalization::L2,
        };
        let embedding_bytes = vec![
            2, 1, 0, 0, 3, 0, 0, 0, 1, 0, 0, 0, b'm', 1, 0, 0, 0, b'u', 1, 0, 0, 0, b'k',
        ];
        assert_eq!(encode_embedding(&embedding).unwrap(), embedding_bytes);
        assert_eq!(decode_embedding(&embedding_bytes).unwrap(), embedding);
    }
}
