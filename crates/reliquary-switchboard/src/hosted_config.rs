//! Credential-free Switchboard routes for trusted host-owned transports.

use crate::{
    ConfigObject, ConfiguredRuntimeError, EmbeddingNormalization, ModelSlot,
    config_document::ConfigDocument, protected_config,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const HOSTED_SCHEMA: u16 = 0x4853;
const HOSTED_VERSION: u8 = 1;
const MAX_ENDPOINT_BYTES: usize = 2048;
const MAX_MODEL_BYTES: usize = 256;
const REASONING_EFFORTS: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];

/// Embedding compatibility selected by a host-owned Switchboard route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostedEmbeddingSettings {
    pub dimensions: u32,
    pub normalization: EmbeddingNormalization,
}

/// Non-secret route selection for a Hermes or other trusted host transport.
/// `endpoint` is a host-recognized provider scope or endpoint reference; it is
/// never used by Switchboard to make a network request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostedModelRoute {
    pub endpoint: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub embedding: Option<HostedEmbeddingSettings>,
}

impl HostedModelRoute {
    fn validate(&self, slot: ModelSlot) -> Result<(), ConfiguredRuntimeError> {
        let err = || hosted_invalid(slot, "invalid hosted model route");
        if self.endpoint.is_empty()
            || self.endpoint.len() > MAX_ENDPOINT_BYTES
            || self.endpoint.chars().any(char::is_control)
        {
            return Err(err());
        }
        if self.model.as_ref().is_some_and(|model| {
            model.trim().is_empty()
                || model.len() > MAX_MODEL_BYTES
                || model.chars().any(char::is_control)
        }) {
            return Err(err());
        }
        if let Some(effort) = &self.reasoning_effort {
            if slot.is_decision()
                || slot == ModelSlot::Embedding
                || !REASONING_EFFORTS.contains(&effort.as_str())
            {
                return Err(err());
            }
        }
        match (slot, &self.embedding) {
            (ModelSlot::Embedding, Some(embedding)) if embedding.dimensions > 0 => {}
            (ModelSlot::Embedding, _) => return Err(err()),
            (_, Some(_)) => return Err(err()),
            (_, None) => {}
        }
        Ok(())
    }

    fn encode(&self, slot: ModelSlot) -> Result<ConfigObject, ConfiguredRuntimeError> {
        self.validate(slot)?;
        let payload = serde_json::to_vec(&HostedRouteRecord::from_route(self))
            .map_err(|_| hosted_invalid(slot, "invalid hosted model route"))?;
        Ok(ConfigObject {
            schema: HOSTED_SCHEMA,
            flags: 0,
            payload,
        })
    }

    fn decode(object: &ConfigObject, slot: ModelSlot) -> Result<Self, ConfiguredRuntimeError> {
        let err = || hosted_invalid(slot, "invalid hosted model route");
        if object.schema != HOSTED_SCHEMA {
            return Err(hosted_invalid(slot, "model object is not a hosted route"));
        }
        if object.flags != 0 {
            return Err(err());
        }
        let record: HostedRouteRecord =
            serde_json::from_slice(&object.payload).map_err(|_| err())?;
        if record.version != HOSTED_VERSION {
            return Err(err());
        }
        let route = record.into_route()?;
        route.validate(slot)?;
        Ok(route)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostedRouteRecord {
    version: u8,
    endpoint: String,
    model: Option<String>,
    reasoning_effort: Option<String>,
    embedding: Option<HostedEmbeddingRecord>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostedEmbeddingRecord {
    dimensions: u32,
    normalization: u8,
}

impl HostedRouteRecord {
    fn from_route(route: &HostedModelRoute) -> Self {
        Self {
            version: HOSTED_VERSION,
            endpoint: route.endpoint.clone(),
            model: route.model.clone(),
            reasoning_effort: route.reasoning_effort.clone(),
            embedding: route
                .embedding
                .as_ref()
                .map(|settings| HostedEmbeddingRecord {
                    dimensions: settings.dimensions,
                    normalization: match settings.normalization {
                        EmbeddingNormalization::None => 0,
                        EmbeddingNormalization::L2 => 1,
                    },
                }),
        }
    }

    fn into_route(self) -> Result<HostedModelRoute, ConfiguredRuntimeError> {
        let embedding = match self.embedding {
            Some(settings) => Some(HostedEmbeddingSettings {
                dimensions: settings.dimensions,
                normalization: match settings.normalization {
                    0 => EmbeddingNormalization::None,
                    1 => EmbeddingNormalization::L2,
                    _ => {
                        return Err(hosted_invalid(
                            ModelSlot::Embedding,
                            "invalid hosted embedding normalization",
                        ));
                    }
                },
            }),
            None => None,
        };
        Ok(HostedModelRoute {
            endpoint: self.endpoint,
            model: self.model,
            reasoning_effort: self.reasoning_effort,
            embedding,
        })
    }
}

fn hosted_invalid(slot: ModelSlot, reason: &'static str) -> ConfiguredRuntimeError {
    ConfiguredRuntimeError::Invalid {
        route: slot.label(),
        reason,
    }
}

/// Credential-free model selections stored in Switchboard's CVCFG document.
/// Opening and saving this view never decrypts, creates, or rewrites credential
/// objects. Unknown CVCFG objects are preserved during atomic route updates.
pub struct HostedSwitchboardConfig {
    path: PathBuf,
    resolved_path: Option<PathBuf>,
    models: BTreeMap<ModelSlot, HostedModelRoute>,
    pending: BTreeMap<ModelSlot, Option<HostedModelRoute>>,
}

impl HostedSwitchboardConfig {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            resolved_path: None,
            models: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, ConfiguredRuntimeError> {
        let path = path.as_ref();
        let resolved = path
            .canonicalize()
            .map_err(|error| ConfiguredRuntimeError::Io(error.kind()))?;
        let document = ConfigDocument::read(&resolved)?;
        if document.0.keys().any(|key| key.starts_with("credential.")) {
            return Err(ConfiguredRuntimeError::Invalid {
                route: "hosted configuration",
                reason: "credential objects are not supported",
            });
        }
        let models = models_from_document(&document)?;
        Ok(Self {
            path: path.to_owned(),
            resolved_path: Some(resolved),
            models,
            pending: BTreeMap::new(),
        })
    }

    pub fn model(&self, slot: ModelSlot) -> Option<&HostedModelRoute> {
        self.models.get(&slot)
    }

    pub fn set_model(
        &mut self,
        slot: ModelSlot,
        route: Option<HostedModelRoute>,
    ) -> Result<(), ConfiguredRuntimeError> {
        if let Some(route) = &route {
            route.validate(slot)?;
            self.models.insert(slot, route.clone());
        } else {
            self.models.remove(&slot);
        }
        self.pending.insert(slot, route);
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), ConfiguredRuntimeError> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let path = match &self.resolved_path {
            Some(path) => path.clone(),
            None => protected_config::config_destination(&self.path)?,
        };
        let _lock = protected_config::config_file_lock(&path)
            .map_err(|error| ConfiguredRuntimeError::Write(error.kind()))?;
        let replace = path.exists();
        let mut document = if replace {
            ConfigDocument::read(&path)?
        } else {
            ConfigDocument::default()
        };
        if document.0.keys().any(|key| key.starts_with("credential.")) {
            return Err(ConfiguredRuntimeError::Invalid {
                route: "hosted configuration",
                reason: "credential objects are not supported",
            });
        }
        for slot in ModelSlot::ALL {
            if let Some(object) = document.0.get(slot.object_key()) {
                HostedModelRoute::decode(object, slot)?;
            }
        }
        for (slot, route) in &self.pending {
            match route {
                Some(route) => {
                    document
                        .0
                        .insert(slot.object_key().into(), route.encode(*slot)?);
                }
                None => {
                    document.0.remove(slot.object_key());
                }
            }
        }
        let models = models_from_document(&document)?;
        document.save(&path, replace)?;
        self.models = models;
        self.resolved_path = Some(path);
        self.pending.clear();
        Ok(())
    }
}

fn models_from_document(
    document: &ConfigDocument,
) -> Result<BTreeMap<ModelSlot, HostedModelRoute>, ConfiguredRuntimeError> {
    let mut models = BTreeMap::new();
    for slot in ModelSlot::ALL {
        let Some(object) = document.0.get(slot.object_key()) else {
            continue;
        };
        let route = HostedModelRoute::decode(object, slot)?;
        route.validate(slot)?;
        models.insert(slot, route);
    }
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_document::ConfigObject;

    fn route(endpoint: &str, model: Option<&str>) -> HostedModelRoute {
        HostedModelRoute {
            endpoint: endpoint.into(),
            model: model.map(str::to_owned),
            reasoning_effort: None,
            embedding: None,
        }
    }

    #[test]
    fn hosted_switchboard_config_roundtrips_routes_and_preserves_unknown_objects() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosted.cfg");
        let mut original = HostedSwitchboardConfig::new(&path);
        original
            .set_model(
                ModelSlot::General,
                Some(route("hermes:openai", Some("chat-v3"))),
            )
            .unwrap();
        original.save().unwrap();

        let mut document = ConfigDocument::read(&path).unwrap();
        let unknown = ConfigObject {
            schema: 9,
            flags: 0,
            payload: b"preserve me".to_vec(),
        };
        document.0.insert("product.unknown".into(), unknown.clone());
        document.save(&path, true).unwrap();

        let mut opened = HostedSwitchboardConfig::open(&path).unwrap();
        assert_eq!(
            opened.model(ModelSlot::General),
            Some(&route("hermes:openai", Some("chat-v3")))
        );
        opened
            .set_model(
                ModelSlot::Embedding,
                Some(HostedModelRoute {
                    endpoint: "hermes:embedding".into(),
                    model: None,
                    reasoning_effort: None,
                    embedding: Some(HostedEmbeddingSettings {
                        dimensions: 1536,
                        normalization: EmbeddingNormalization::L2,
                    }),
                }),
            )
            .unwrap();
        opened.save().unwrap();

        let persisted = ConfigDocument::read(&path).unwrap();
        assert!(
            persisted
                .0
                .get("product.unknown")
                .is_some_and(|object| object == &unknown)
        );
        let model_object = persisted.0.get(ModelSlot::General.object_key()).unwrap();
        assert_eq!(model_object.schema, HOSTED_SCHEMA);
        let reopened = HostedSwitchboardConfig::open(&path).unwrap();
        assert_eq!(
            reopened.model(ModelSlot::General),
            Some(&route("hermes:openai", Some("chat-v3")))
        );
        assert_eq!(
            reopened
                .model(ModelSlot::Embedding)
                .unwrap()
                .embedding
                .as_ref()
                .unwrap()
                .dimensions,
            1536
        );
    }

    #[test]
    fn hosted_switchboard_config_rejects_standalone_routes_and_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy.cfg");
        let mut document = ConfigDocument::default();
        document.0.insert(
            "credential.LOST_KEY".into(),
            ConfigObject {
                schema: 88,
                flags: 0,
                payload: b"not decryptable by design".to_vec(),
            },
        );
        document.0.insert(
            ModelSlot::General.object_key().into(),
            ConfigObject {
                schema: 3,
                flags: 0,
                payload: b"standalone route record".to_vec(),
            },
        );
        document.save(&path, false).unwrap();

        let error = HostedSwitchboardConfig::open(&path).err().unwrap();
        assert!(
            error
                .to_string()
                .contains("credential objects are not supported")
        );

        document.0.remove("credential.LOST_KEY");
        document.save(&path, true).unwrap();
        let error = HostedSwitchboardConfig::open(&path).err().unwrap();
        assert!(
            error
                .to_string()
                .contains("model object is not a hosted route")
        );
    }

    #[test]
    fn hosted_routes_reject_slot_incompatible_settings_and_out_of_bounds_fields() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = HostedSwitchboardConfig::new(dir.path().join("routes.cfg"));
        let mut wrong_slot = route("provider", Some("model"));
        wrong_slot.embedding = Some(HostedEmbeddingSettings {
            dimensions: 3,
            normalization: EmbeddingNormalization::None,
        });
        assert!(
            config
                .set_model(ModelSlot::General, Some(wrong_slot))
                .is_err()
        );

        let mut invalid_embedding = route("provider", None);
        invalid_embedding.embedding = Some(HostedEmbeddingSettings {
            dimensions: 0,
            normalization: EmbeddingNormalization::None,
        });
        assert!(
            config
                .set_model(ModelSlot::Embedding, Some(invalid_embedding))
                .is_err()
        );

        let mut invalid_decision = route("provider", Some("model"));
        invalid_decision.reasoning_effort = Some("high".into());
        assert!(
            config
                .set_model(ModelSlot::GeneralDecision, Some(invalid_decision))
                .is_err()
        );
    }
}
