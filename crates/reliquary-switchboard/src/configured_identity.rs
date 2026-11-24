//! Stable identity of the routes an owner processing job may invoke.
use super::model_config::{
    DREAM, EMBEDDING, ENTITY, GENERAL, INSOMNIA, INSOMNIA_METADATA, RuntimeConfig, SHARED,
};
use super::{
    CredentialsConfig, GeneralRoute, HostedInferenceIdentity, ModelProvider,
    OBSERVATION_DECISION_MODEL, ProcessingRouteIdentity,
};
use ring::digest::{SHA256, digest};
use serde_json::{Value, json};

pub(super) fn processing_routes(
    config: &RuntimeConfig,
    credentials: &CredentialsConfig,
) -> ProcessingRouteIdentity {
    let general = config.general.as_ref().map(|route| (GENERAL, route));
    let insomnia = config
        .insomnia
        .as_ref()
        .map(|route| (INSOMNIA, route))
        .or(general);
    let ownership = config
        .insomnia_metadata
        .as_ref()
        .map(|route| (INSOMNIA_METADATA, route))
        .or(insomnia);
    let dream = config
        .dream
        .as_ref()
        .map(|route| (DREAM, route))
        .or(general);
    let entity = config
        .entity_resolution
        .as_ref()
        .map(|route| (ENTITY, route));
    let general_identity = |selection: Option<(&str, &GeneralRoute)>| {
        selection.map(|(name, route)| {
            let endpoint = &route.endpoint;
            let account = credentials
                .auth(&endpoint.credential_id, endpoint.provider)
                .and_then(|(_, account)| account);
            json!([
                name,
                provider(endpoint.provider),
                endpoint.model,
                endpoint.url,
                endpoint.credential_id.0,
                account,
                route.reasoning_effort
            ])
        })
    };
    let embedding = config.embedding.as_ref().map(|endpoint| {
        json!([
            EMBEDDING,
            provider(endpoint.provider),
            endpoint.model,
            endpoint.url,
            endpoint.credential_id.0,
            endpoint.dimensions,
            match endpoint.normalization {
                super::EmbeddingNormalization::None => "none",
                super::EmbeddingNormalization::L2 => "l2",
            }
        ])
    });
    let dream = general_identity(dream);
    let identity_route = entity.map(|(_, route)| {
        json!([
            provider(route.endpoint.provider),
            route.endpoint.model,
            route.endpoint.url,
            route.reasoning_effort
        ])
    });
    let native = config
        .general_decision
        .as_ref()
        .filter(|route| route.model == OBSERVATION_DECISION_MODEL)
        .map(|route| json!([SHARED, route.model, route.url, route.credential_id.0]));
    ProcessingRouteIdentity {
        perception_embedding: hash(json!([
            "reliquary-perception-embedding-route-v1",
            embedding
        ])),
        perception: hash(json!([
            "reliquary-perception-routes-v1",
            general_identity(general),
            native,
            embedding
        ])),
        entity_identity: hash(json!([
            "reliquary-entity-identity-route-v1",
            identity_route
        ])),
        import: hash(json!([
            "reliquary-owner-processing-routes-v1",
            general_identity(insomnia),
            general_identity(ownership),
            dream,
            general_identity(entity),
            embedding,
        ])),
        live_dream: hash(json!(["reliquary-live-dream-routes-v1", dream, embedding])),
    }
}

pub(super) fn hosted(identity: &HostedInferenceIdentity) -> ProcessingRouteIdentity {
    let general = json!([
        "hermes-hosted-general-v1",
        identity.general_provider(),
        identity.general_model(),
    ]);
    let embedding = json!([
        "hermes-hosted-embedding-v1",
        identity.embedding_provider(),
        identity.embedding_model(),
        identity.embedding_dimensions(),
        identity.embedding_input_semantics().as_str(),
        match identity.embedding_normalization() {
            super::EmbeddingNormalization::None => "none",
            super::EmbeddingNormalization::L2 => "l2",
        }
    ]);
    ProcessingRouteIdentity {
        perception_embedding: hash(json!([
            "reliquary-perception-embedding-route-v1",
            embedding
        ])),
        perception: hash(json!([
            "reliquary-hermes-hosted-perception-v1",
            general,
            embedding
        ])),
        entity_identity: hash(json!(["reliquary-entity-identity-route-v1", general])),
        import: hash(json!([
            "reliquary-hermes-hosted-import-v1",
            general,
            embedding
        ])),
        live_dream: hash(json!([
            "reliquary-hermes-hosted-live-dream-v1",
            general,
            embedding
        ])),
    }
}

fn hash(canonical: Value) -> [u8; 32] {
    digest(&SHA256, canonical.to_string().as_bytes())
        .as_ref()
        .try_into()
        .expect("SHA-256 digest")
}

fn provider(provider: ModelProvider) -> &'static str {
    match provider {
        ModelProvider::OpenAiCodex => "openai_codex",
        ModelProvider::OpenAiReady => "openai_ready",
    }
}
