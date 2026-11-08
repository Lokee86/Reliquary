use super::runtime_credentials::resolve_credential;
use super::transport::validated_http_url;
use super::*;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub(super) const GENERAL: &str = "general";
pub(super) const SHARED: &str = "decision";
pub(super) const INSOMNIA: &str = "insomnia";
pub(super) const DREAM: &str = "dream";
pub(super) const ENTITY: &str = "entity_resolution";
pub(super) const ENTITY_DECISION: &str = "entity_resolution_decision";
pub(super) const RETRIEVAL: &str = "memory_retrieval_decision";
pub(super) const EMBEDDING: &str = "embedding";
const WORKER: &str = "worker";
const MAX_CONFIG_BYTES: usize = 64 * 1024;

pub(super) struct GeneralRoute {
    pub endpoint: GeneralModelEndpoint,
    pub reasoning_effort: Option<&'static str>,
}

pub(super) struct RuntimeConfig {
    pub general: Option<GeneralRoute>,
    pub insomnia: Option<GeneralRoute>,
    pub dream: Option<GeneralRoute>,
    pub entity_resolution: Option<GeneralRoute>,
    pub general_decision: Option<DecisionModelEndpoint>,
    pub entity_resolution_decision: Option<DecisionModelEndpoint>,
    pub memory_retrieval_decision: Option<DecisionModelEndpoint>,
    pub embedding: Option<EmbeddingModelEndpoint>,
    pub worker_lease_duration_ms: Option<u32>,
}

impl RuntimeConfig {
    pub fn load(path: &Path) -> Result<Self, ConfiguredRuntimeError> {
        let mut bytes = Vec::new();
        File::open(path)
            .and_then(|file| {
                file.take((MAX_CONFIG_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
            })
            .map_err(|error| ConfiguredRuntimeError::Io(error.kind()))?;
        if bytes.len() > MAX_CONFIG_BYTES {
            return Err(invalid("configuration", "file exceeds 64 KiB"));
        }
        let document: Value = serde_json::from_slice(&bytes)
            .map_err(|_| invalid("configuration", "expected a JSON object"))?;
        let object = document
            .as_object()
            .ok_or_else(|| invalid("configuration", "expected a JSON object"))?;
        if object.keys().any(|key| {
            ![
                GENERAL,
                INSOMNIA,
                DREAM,
                ENTITY,
                SHARED,
                ENTITY_DECISION,
                RETRIEVAL,
                EMBEDDING,
                WORKER,
            ]
            .contains(&key.as_str())
        }) {
            return Err(invalid("configuration", "unknown route"));
        }
        let general = |name| parse_general_route(object, name);
        let decision =
            |name| parse_route(object, name, &[]).map(|entry| entry.map(|(route, _)| route));
        let embedding = parse_route(object, EMBEDDING, &["dimensions", "normalization"])?
            .map(|(route, fields)| {
                let dimensions = fields
                    .get("dimensions")
                    .and_then(Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .filter(|value| *value > 0)
                    .ok_or_else(|| invalid(EMBEDDING, "dimensions must be a positive u32"))?;
                let normalization = match fields.get("normalization").map(Value::as_str) {
                    None | Some(Some("none")) => EmbeddingNormalization::None,
                    Some(Some("l2")) => EmbeddingNormalization::L2,
                    _ => return Err(invalid(EMBEDDING, "unsupported normalization")),
                };
                Ok(EmbeddingModelEndpoint {
                    provider: ModelProvider::OpenAiReady,
                    model: route.model,
                    url: route.url,
                    credential_id: route.credential_id,
                    dimensions,
                    normalization,
                })
            })
            .transpose()?;
        Ok(Self {
            general: general(GENERAL)?,
            insomnia: general(INSOMNIA)?,
            dream: general(DREAM)?,
            entity_resolution: general(ENTITY)?,
            general_decision: decision(SHARED)?,
            entity_resolution_decision: decision(ENTITY_DECISION)?,
            memory_retrieval_decision: decision(RETRIEVAL)?,
            embedding,
            worker_lease_duration_ms: worker_lease_duration(object)?,
        })
    }

    pub fn credentials(
        &self,
        mut lookup: impl FnMut(&CredentialId) -> Result<CredentialMaterial, &'static str>,
    ) -> Result<CredentialsConfig, ConfiguredRuntimeError> {
        let mut credentials = CredentialsConfig::default();
        let mut resolve = |route, id: &CredentialId, provider| {
            resolve_credential(&mut credentials, &mut lookup, id, provider)
                .map_err(|reason| ConfiguredRuntimeError::Credential { route, reason })
        };
        for (name, route) in [
            (GENERAL, &self.general),
            (INSOMNIA, &self.insomnia),
            (DREAM, &self.dream),
            (ENTITY, &self.entity_resolution),
        ] {
            if let Some(GeneralRoute { endpoint, .. }) = route {
                resolve(name, &endpoint.credential_id, endpoint.provider)?;
            }
        }
        for (name, route) in [
            (SHARED, &self.general_decision),
            (ENTITY_DECISION, &self.entity_resolution_decision),
            (RETRIEVAL, &self.memory_retrieval_decision),
        ] {
            if let Some(route) = route {
                resolve(name, &route.credential_id, ModelProvider::OpenAiReady)?;
            }
        }
        if let Some(route) = &self.embedding {
            resolve(EMBEDDING, &route.credential_id, route.provider)?;
        }
        Ok(credentials)
    }
}

fn parse_general_route(
    object: &Map<String, Value>,
    name: &'static str,
) -> Result<Option<GeneralRoute>, ConfiguredRuntimeError> {
    parse_route(object, name, &["provider"])?
        .map(|(route, fields)| {
            let provider = match fields.get("provider").map(Value::as_str) {
                None | Some(Some("openai_ready")) => ModelProvider::OpenAiReady,
                Some(Some("openai_codex")) => ModelProvider::OpenAiCodex,
                _ => return Err(invalid(name, "unsupported provider")),
            };
            Ok(GeneralRoute {
                endpoint: GeneralModelEndpoint {
                    provider,
                    model: route.model,
                    url: route.url,
                    credential_id: route.credential_id,
                },
                reasoning_effort: None,
            })
        })
        .transpose()
}

fn worker_lease_duration(
    object: &Map<String, Value>,
) -> Result<Option<u32>, ConfiguredRuntimeError> {
    let Some(worker) = object.get(WORKER) else {
        return Ok(None);
    };
    let worker = worker
        .as_object()
        .ok_or_else(|| invalid(WORKER, "expected a worker policy object"))?;
    if worker.keys().any(|key| key != "lease_duration_ms") {
        return Err(invalid(WORKER, "unknown worker policy field"));
    }
    worker
        .get("lease_duration_ms")
        .map(|value| {
            let millis = value
                .as_u64()
                .filter(|millis| (30_000..=600_000).contains(millis))
                .ok_or_else(|| {
                    invalid(WORKER, "lease_duration_ms must be between 30000 and 600000")
                })?;
            Ok(millis as u32)
        })
        .transpose()
}

fn parse_route<'a>(
    object: &'a Map<String, Value>,
    name: &'static str,
    extra_fields: &[&str],
) -> Result<Option<(DecisionModelEndpoint, &'a Map<String, Value>)>, ConfiguredRuntimeError> {
    let Some(route) = object.get(name) else {
        return Ok(None);
    };
    let fields = route
        .as_object()
        .ok_or_else(|| invalid(name, "expected a route object"))?;
    if fields.keys().any(|key| {
        let key = key.as_str();
        !["model", "url", "credential_id"].contains(&key) && !extra_fields.contains(&key)
    }) {
        return Err(invalid(name, "unknown route field"));
    }
    let model = required(fields, "model").ok_or_else(|| invalid(name, "model is required"))?;
    let url = required(fields, "url").ok_or_else(|| invalid(name, "URL is required"))?;
    validated_http_url(url).map_err(|reason| invalid(name, reason))?;
    let id = required(fields, "credential_id")
        .ok_or_else(|| invalid(name, "credential_id is required"))?;
    let credential_id =
        CredentialId::new(id).map_err(|_| invalid(name, "invalid credential ID"))?;
    Ok(Some((
        DecisionModelEndpoint {
            model: model.to_owned(),
            url: url.to_owned(),
            credential_id,
        },
        fields,
    )))
}

fn required<'a>(fields: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    fields
        .get(key)?
        .as_str()
        .filter(|value| !value.trim().is_empty())
}

pub(super) fn invalid(route: &'static str, reason: &'static str) -> ConfiguredRuntimeError {
    ConfiguredRuntimeError::Invalid { route, reason }
}
