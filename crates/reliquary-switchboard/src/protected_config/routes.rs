use crate::config_document::{
    ConfigDocument, ConfigObject as Object, read_string, take_array, write_string,
};
use crate::model_config::{GeneralRoute, ModelRoute, ModelSlot, RuntimeConfig, invalid};
use crate::transport::validated_http_url;
use crate::{
    ConfiguredRuntimeError, CredentialId, DecisionModelEndpoint, EmbeddingModelEndpoint,
    EmbeddingNormalization, GeneralModelEndpoint, ModelProvider,
};

pub(crate) const CODEX_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const EFFORTS: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];

pub(super) fn decode(document: &ConfigDocument) -> Result<RuntimeConfig, ConfiguredRuntimeError> {
    use ModelSlot::*;
    let mut config = RuntimeConfig::default();
    for slot in ModelSlot::ALL {
        let Some(route) = get(document, slot)? else {
            continue;
        };
        match (slot, route) {
            (General, ModelRoute::General(route)) => config.general = Some(route),
            (Insomnia, ModelRoute::General(route)) => config.insomnia = Some(route),
            (InsomniaMetadata, ModelRoute::General(route)) => {
                config.insomnia_metadata = Some(route)
            }
            (EntityExtraction, ModelRoute::General(route)) => {
                config.entity_extraction = Some(route)
            }
            (EntityResolution, ModelRoute::General(route)) => {
                config.entity_resolution = Some(route)
            }
            (Dream, ModelRoute::General(route)) => config.dream = Some(route),
            (GeneralDecision, ModelRoute::Decision(route)) => config.general_decision = Some(route),
            (EntityResolutionDecision, ModelRoute::Decision(route)) => {
                config.entity_resolution_decision = Some(route)
            }
            (MemoryRetrievalDecision, ModelRoute::Decision(route)) => {
                config.memory_retrieval_decision = Some(route)
            }
            (Embedding, ModelRoute::Embedding(route)) => config.embedding = Some(route),
            _ => unreachable!("route codec selects the slot's type"),
        }
    }
    Ok(config)
}

pub(super) fn get(
    document: &ConfigDocument,
    slot: ModelSlot,
) -> Result<Option<ModelRoute>, ConfiguredRuntimeError> {
    document
        .0
        .get(slot.object_key())
        .map(|object| {
            decode_route(slot, object).ok_or_else(|| invalid(slot.label(), "invalid model route"))
        })
        .transpose()
}

fn decode_route(slot: ModelSlot, object: &Object) -> Option<ModelRoute> {
    if slot.is_decision() {
        decision(object).map(ModelRoute::Decision)
    } else if slot == ModelSlot::Embedding {
        embedding(object).map(ModelRoute::Embedding)
    } else {
        general(object).map(ModelRoute::General)
    }
}

pub(super) fn encode(
    slot: ModelSlot,
    route: &ModelRoute,
) -> Result<Object, ConfiguredRuntimeError> {
    let mut payload = Vec::new();
    let err = || invalid(slot.label(), "invalid model route");
    let (schema, model, url, id) = match route {
        ModelRoute::General(route) if !slot.is_decision() && slot != ModelSlot::Embedding => {
            let endpoint = &route.endpoint;
            let (provider, url) = match endpoint.provider {
                ModelProvider::OpenAiCodex
                    if endpoint.url.is_empty() || endpoint.url == CODEX_URL =>
                {
                    (1, "")
                }
                ModelProvider::OpenAiCodex => return Err(err()),
                ModelProvider::OpenAiReady => (2, endpoint.url.as_str()),
            };
            let effort = route
                .reasoning_effort
                .map(|effort| {
                    EFFORTS
                        .iter()
                        .position(|value| *value == effort)
                        .map(|index| index as u8 + 1)
                        .ok_or_else(err)
                })
                .transpose()?
                .unwrap_or(0);
            payload.extend_from_slice(&[provider, effort, 0, 0]);
            (3, &endpoint.model, url, &endpoint.credential_id)
        }
        ModelRoute::Decision(route) if slot.is_decision() => {
            (1, &route.model, route.url.as_str(), &route.credential_id)
        }
        ModelRoute::Embedding(route)
            if slot == ModelSlot::Embedding && route.provider == ModelProvider::OpenAiReady =>
        {
            let normalization = match route.normalization {
                EmbeddingNormalization::None => 0,
                EmbeddingNormalization::L2 => 1,
            };
            payload.extend_from_slice(&[2, normalization, 0, 0]);
            payload.extend_from_slice(&route.dimensions.to_le_bytes());
            (2, &route.model, route.url.as_str(), &route.credential_id)
        }
        _ => return Err(err()),
    };
    for value in [model.as_str(), url, id.as_str()] {
        write_string(&mut payload, value).ok_or_else(err)?;
    }
    let object = Object {
        schema,
        flags: 0,
        payload,
    };
    decode_route(slot, &object).ok_or_else(err)?;
    Ok(object)
}

fn general(object: &Object) -> Option<GeneralRoute> {
    let mut bytes = object.payload.as_slice();
    let &[provider, effort, 0, 0] = take_array(&mut bytes)? else {
        return None;
    };
    let effort = match (object.schema, object.flags, effort) {
        (2 | 3, 0, 0) => None,
        (3, 0, 1..=7) => Some(EFFORTS[usize::from(effort - 1)]),
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

fn decision(object: &Object) -> Option<DecisionModelEndpoint> {
    if object.schema != 1 || object.flags != 0 {
        return None;
    }
    route_fields(&object.payload, None)
}

fn embedding(object: &Object) -> Option<EmbeddingModelEndpoint> {
    if object.schema != 2 || object.flags != 0 {
        return None;
    }
    let mut bytes = object.payload.as_slice();
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
