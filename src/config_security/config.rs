use crate::config_codec::{RawConfigObject, decode_config, encode_config};
use crate::config_credentials::{insert_credentials, load_credentials, master_key_path};
use crate::config_io::{read_config_file, replace_config_file};
use crate::config_object::{
    FRAGMENT_KEY, OBJECT_FLAGS_NONE, OBJECT_SCHEMA_V1, RETRIEVAL_KEY, RUNTIME_KEY,
    decode_fragments, decode_retrieval, decode_runtime, encode_fragments, encode_retrieval,
    encode_runtime, validate_fragments, validate_retrieval, validate_runtime,
};
use crate::model_switchboard::validate_switchboard;
use crate::model_switchboard_codec::{
    CHRONOS_MODEL_KEY, DECISION_MODEL_SCHEMA_V1, DREAM_MODEL_KEY, EMBEDDING_MODEL_KEY,
    EMBEDDING_MODEL_SCHEMA_V2, ENTITY_EXTRACTION_MODEL_KEY, ENTITY_RESOLUTION_DECISION_MODEL_KEY,
    ENTITY_RESOLUTION_MODEL_KEY, GENERAL_MODEL_KEY, GENERAL_MODEL_SCHEMA_V3,
    INSOMNIA_METADATA_MODEL_KEY, INSOMNIA_MODEL_KEY, MEMORY_RETRIEVAL_DECISION_MODEL_KEY,
    decode_decision, decode_embedding, decode_general_v2, decode_general_v3, encode_decision,
    encode_embedding, encode_general,
};
use crate::{
    ConfigError, CredentialsConfig, FragmentConfig, JsonMasterKeyStore, MasterKey, MasterKeyError,
    MasterKeyStore, ModelSwitchboardConfig, RetrievalConfig, RuntimeConfig,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct ReliquaryConfig {
    path: PathBuf,
    pub fragments: FragmentConfig,
    pub retrieval: RetrievalConfig,
    pub runtime: RuntimeConfig,
    pub models: ModelSwitchboardConfig,
    pub credentials: CredentialsConfig,
    extra_objects: BTreeMap<String, RawConfigObject>,
}

impl ReliquaryConfig {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            fragments: FragmentConfig::default(),
            retrieval: RetrievalConfig::default(),
            runtime: RuntimeConfig::default(),
            models: ModelSwitchboardConfig::default(),
            credentials: CredentialsConfig::default(),
            extra_objects: BTreeMap::new(),
        }
    }

    pub fn open(path: impl Into<PathBuf>) -> Result<Self, ConfigError> {
        let path = path.into();
        let bytes = read_config_file(&path)?;
        let mut objects = decode_config(&bytes)?;
        let fragments = match objects.remove(FRAGMENT_KEY) {
            Some(object) => decode_known_fragments(object)?,
            None => FragmentConfig::default(),
        };
        let retrieval = match objects.remove(RETRIEVAL_KEY) {
            Some(object) => decode_known_retrieval(object)?,
            None => RetrievalConfig::default(),
        };
        let runtime = match objects.remove(RUNTIME_KEY) {
            Some(object) => decode_known_runtime(object)?,
            None => RuntimeConfig::default(),
        };
        let general = match objects.remove(GENERAL_MODEL_KEY) {
            Some(object) => Some(decode_known_general(object)?),
            None => None,
        };
        let insomnia = match objects.remove(INSOMNIA_MODEL_KEY) {
            Some(object) => Some(decode_known_general(object)?),
            None => None,
        };
        let insomnia_metadata = match objects.remove(INSOMNIA_METADATA_MODEL_KEY) {
            Some(object) => Some(decode_known_general(object)?),
            None => None,
        };
        let entity_extraction = match objects.remove(ENTITY_EXTRACTION_MODEL_KEY) {
            Some(object) => Some(decode_known_general(object)?),
            None => None,
        };
        let entity_resolution_decision = match objects.remove(ENTITY_RESOLUTION_DECISION_MODEL_KEY)
        {
            Some(object) => Some(decode_known_decision(object)?),
            None => None,
        };
        let memory_retrieval_decision = match objects.remove(MEMORY_RETRIEVAL_DECISION_MODEL_KEY) {
            Some(object) => Some(decode_known_decision(object)?),
            None => None,
        };
        let entity_resolution = match objects.remove(ENTITY_RESOLUTION_MODEL_KEY) {
            Some(object) => Some(decode_known_general(object)?),
            None => None,
        };
        let chronos = match objects.remove(CHRONOS_MODEL_KEY) {
            Some(object) => Some(decode_known_general(object)?),
            None => None,
        };
        let dream = match objects.remove(DREAM_MODEL_KEY) {
            Some(object) => Some(decode_known_general(object)?),
            None => None,
        };
        let embedding = match objects.remove(EMBEDDING_MODEL_KEY) {
            Some(object) => Some(decode_known_embedding(object)?),
            None => None,
        };
        let models = ModelSwitchboardConfig {
            general,
            insomnia,
            insomnia_metadata,
            entity_extraction,
            entity_resolution_decision,
            memory_retrieval_decision,
            entity_resolution,
            chronos,
            dream,
            embedding,
        };
        validate_switchboard(&models)?;
        let credentials = load_credentials(&path, &mut objects)?;
        Ok(Self {
            path,
            fragments,
            retrieval,
            runtime,
            models,
            credentials,
            extra_objects: objects,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load_or_create_master_key(&self) -> Result<MasterKey, MasterKeyError> {
        JsonMasterKeyStore::new(master_key_path(&self.path)).load_or_create()
    }

    pub fn validate_runtime(&self) -> Result<(), ConfigError> {
        crate::ModelSwitchboard::new(self.models.clone(), self.credentials.clone())?;
        Ok(())
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        validate_fragments(self.fragments)?;
        validate_retrieval(self.retrieval)?;
        validate_runtime(self.runtime)?;
        validate_switchboard(&self.models)?;
        let mut objects = self.extra_objects.clone();
        objects.insert(
            FRAGMENT_KEY.to_owned(),
            RawConfigObject {
                schema: OBJECT_SCHEMA_V1,
                flags: OBJECT_FLAGS_NONE,
                payload: encode_fragments(self.fragments),
            },
        );
        objects.insert(
            RETRIEVAL_KEY.to_owned(),
            RawConfigObject {
                schema: OBJECT_SCHEMA_V1,
                flags: OBJECT_FLAGS_NONE,
                payload: encode_retrieval(self.retrieval),
            },
        );
        objects.insert(
            RUNTIME_KEY.to_owned(),
            RawConfigObject {
                schema: OBJECT_SCHEMA_V1,
                flags: OBJECT_FLAGS_NONE,
                payload: encode_runtime(self.runtime),
            },
        );
        if let Some(endpoint) = &self.models.general {
            objects.insert(
                GENERAL_MODEL_KEY.to_owned(),
                general_model_object(encode_general(endpoint)?),
            );
        }
        if let Some(endpoint) = &self.models.insomnia {
            objects.insert(
                INSOMNIA_MODEL_KEY.to_owned(),
                general_model_object(encode_general(endpoint)?),
            );
        }
        if let Some(endpoint) = &self.models.insomnia_metadata {
            objects.insert(
                INSOMNIA_METADATA_MODEL_KEY.to_owned(),
                general_model_object(encode_general(endpoint)?),
            );
        }
        if let Some(endpoint) = &self.models.entity_extraction {
            objects.insert(
                ENTITY_EXTRACTION_MODEL_KEY.to_owned(),
                general_model_object(encode_general(endpoint)?),
            );
        }
        if let Some(endpoint) = &self.models.entity_resolution_decision {
            objects.insert(
                ENTITY_RESOLUTION_DECISION_MODEL_KEY.to_owned(),
                decision_model_object(encode_decision(endpoint)?),
            );
        }
        if let Some(endpoint) = &self.models.memory_retrieval_decision {
            objects.insert(
                MEMORY_RETRIEVAL_DECISION_MODEL_KEY.to_owned(),
                decision_model_object(encode_decision(endpoint)?),
            );
        }
        if let Some(endpoint) = &self.models.entity_resolution {
            objects.insert(
                ENTITY_RESOLUTION_MODEL_KEY.to_owned(),
                general_model_object(encode_general(endpoint)?),
            );
        }
        if let Some(endpoint) = &self.models.chronos {
            objects.insert(
                CHRONOS_MODEL_KEY.to_owned(),
                general_model_object(encode_general(endpoint)?),
            );
        }
        if let Some(endpoint) = &self.models.dream {
            objects.insert(
                DREAM_MODEL_KEY.to_owned(),
                general_model_object(encode_general(endpoint)?),
            );
        }
        if let Some(endpoint) = &self.models.embedding {
            objects.insert(
                EMBEDDING_MODEL_KEY.to_owned(),
                embedding_model_object(encode_embedding(endpoint)?),
            );
        }
        insert_credentials(&self.path, &self.credentials, &mut objects)?;
        let bytes = encode_config(&objects)?;
        replace_config_file(&self.path, &bytes)
    }
}

fn decode_known_fragments(object: RawConfigObject) -> Result<FragmentConfig, ConfigError> {
    validate_known_object(&object)?;
    decode_fragments(&object.payload)
}

fn decode_known_retrieval(object: RawConfigObject) -> Result<RetrievalConfig, ConfigError> {
    validate_known_object(&object)?;
    decode_retrieval(&object.payload)
}

fn decode_known_runtime(object: RawConfigObject) -> Result<RuntimeConfig, ConfigError> {
    validate_known_object(&object)?;
    decode_runtime(&object.payload)
}

fn decode_known_general(
    object: RawConfigObject,
) -> Result<crate::GeneralModelEndpoint, ConfigError> {
    if object.flags != OBJECT_FLAGS_NONE {
        return Err(ConfigError::InvalidObject);
    }
    match object.schema {
        2 => decode_general_v2(&object.payload),
        GENERAL_MODEL_SCHEMA_V3 => decode_general_v3(&object.payload),
        _ => Err(ConfigError::InvalidObject),
    }
}

fn decode_known_decision(
    object: RawConfigObject,
) -> Result<crate::DecisionModelEndpoint, ConfigError> {
    if object.schema != DECISION_MODEL_SCHEMA_V1 || object.flags != OBJECT_FLAGS_NONE {
        return Err(ConfigError::InvalidObject);
    }
    decode_decision(&object.payload)
}

fn decode_known_embedding(
    object: RawConfigObject,
) -> Result<crate::EmbeddingModelEndpoint, ConfigError> {
    if object.schema != EMBEDDING_MODEL_SCHEMA_V2 || object.flags != OBJECT_FLAGS_NONE {
        return Err(ConfigError::InvalidObject);
    }
    decode_embedding(&object.payload)
}

fn general_model_object(payload: Vec<u8>) -> RawConfigObject {
    RawConfigObject {
        schema: GENERAL_MODEL_SCHEMA_V3,
        flags: OBJECT_FLAGS_NONE,
        payload,
    }
}

fn decision_model_object(payload: Vec<u8>) -> RawConfigObject {
    RawConfigObject {
        schema: DECISION_MODEL_SCHEMA_V1,
        flags: OBJECT_FLAGS_NONE,
        payload,
    }
}

fn embedding_model_object(payload: Vec<u8>) -> RawConfigObject {
    RawConfigObject {
        schema: EMBEDDING_MODEL_SCHEMA_V2,
        flags: OBJECT_FLAGS_NONE,
        payload,
    }
}

fn validate_known_object(object: &RawConfigObject) -> Result<(), ConfigError> {
    if object.schema != OBJECT_SCHEMA_V1 || object.flags != OBJECT_FLAGS_NONE {
        return Err(ConfigError::InvalidObject);
    }
    Ok(())
}
