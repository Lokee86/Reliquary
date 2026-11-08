use super::{Object, Objects, read_string, take_array};
use crate::runtime_config::{
    DREAM, EMBEDDING, ENTITY, ENTITY_DECISION, GENERAL, GeneralRoute, INSOMNIA, RETRIEVAL,
    RuntimeConfig, SHARED, invalid,
};
use crate::transport::validated_http_url;
use crate::{
    ConfiguredRuntimeError, CredentialId, DecisionModelEndpoint, EmbeddingModelEndpoint,
    EmbeddingNormalization, GeneralModelEndpoint, ModelProvider,
};

const CODEX_URL: &str = "https://chatgpt.com/backend-api/codex/responses";

pub(super) fn decode(objects: &Objects<'_>) -> Result<RuntimeConfig, ConfiguredRuntimeError> {
    Ok(RuntimeConfig {
        general: selected(objects, "models.general", GENERAL, general)?,
        insomnia: selected(objects, "models.insomnia", INSOMNIA, general)?,
        dream: selected(objects, "models.dream", DREAM, general)?,
        entity_resolution: selected(objects, "models.entity_resolution", ENTITY, general)?,
        general_decision: selected(objects, "models.general_decision", SHARED, decision)?,
        entity_resolution_decision: selected(
            objects,
            "models.entity_resolution_decision",
            ENTITY_DECISION,
            decision,
        )?,
        memory_retrieval_decision: selected(
            objects,
            "models.memory_retrieval_decision",
            RETRIEVAL,
            decision,
        )?,
        embedding: selected(objects, "models.embedding", EMBEDDING, embedding)?,
        worker_lease_duration_ms: None,
    })
}

fn selected<T>(
    objects: &Objects<'_>,
    key: &str,
    route: &'static str,
    decode: impl FnOnce(&Object<'_>) -> Option<T>,
) -> Result<Option<T>, ConfiguredRuntimeError> {
    objects
        .get(key)
        .map(|object| decode(object).ok_or_else(|| invalid(route, "invalid protected route")))
        .transpose()
}

fn general(object: &Object<'_>) -> Option<GeneralRoute> {
    let mut bytes = object.payload;
    let &[provider, effort, 0, 0] = take_array(&mut bytes)? else {
        return None;
    };
    let effort = match (object.schema, object.flags, effort) {
        (2 | 3, 0, 0) => None,
        (3, 0, 1) => Some("none"),
        (3, 0, 2) => Some("minimal"),
        (3, 0, 3) => Some("low"),
        (3, 0, 4) => Some("medium"),
        (3, 0, 5) => Some("high"),
        (3, 0, 6) => Some("xhigh"),
        (3, 0, 7) => Some("max"),
        _ => return None,
    };
    let (provider, default_url) = match provider {
        1 => (ModelProvider::OpenAiCodex, Some(CODEX_URL)),
        2 => (ModelProvider::OpenAiReady, None),
        _ => return None,
    };
    let DecisionModelEndpoint {
        model,
        url,
        credential_id,
    } = route_fields(bytes, default_url)?;
    Some(GeneralRoute {
        endpoint: GeneralModelEndpoint {
            provider,
            model,
            url,
            credential_id,
        },
        reasoning_effort: effort,
    })
}

fn decision(object: &Object<'_>) -> Option<DecisionModelEndpoint> {
    if object.schema != 1 || object.flags != 0 {
        return None;
    }
    route_fields(object.payload, None)
}

fn embedding(object: &Object<'_>) -> Option<EmbeddingModelEndpoint> {
    if object.schema != 2 || object.flags != 0 {
        return None;
    }
    let mut bytes = object.payload;
    let &[2, normalization, 0, 0] = take_array(&mut bytes)? else {
        return None;
    };
    let normalization = match normalization {
        0 => EmbeddingNormalization::None,
        1 => EmbeddingNormalization::L2,
        _ => return None,
    };
    let dimensions = u32::from_le_bytes(*take_array(&mut bytes)?);
    if dimensions == 0 {
        return None;
    }
    let DecisionModelEndpoint {
        model,
        url,
        credential_id,
    } = route_fields(bytes, None)?;
    Some(EmbeddingModelEndpoint {
        provider: ModelProvider::OpenAiReady,
        model,
        url,
        credential_id,
        dimensions,
        normalization,
    })
}

fn route_fields(mut bytes: &[u8], codex_url: Option<&str>) -> Option<DecisionModelEndpoint> {
    let model = read_string(&mut bytes)?;
    let url = read_string(&mut bytes)?;
    let id = read_string(&mut bytes)?;
    if model.trim().is_empty() || !bytes.is_empty() {
        return None;
    }
    let url = match (codex_url, url) {
        (Some(default), "") => default,
        (Some(_), _) => return None,
        (None, url) => url,
    };
    validated_http_url(url).ok()?;
    Some(DecisionModelEndpoint {
        model: model.to_owned(),
        url: url.to_owned(),
        credential_id: CredentialId::new(id).ok()?,
    })
}
