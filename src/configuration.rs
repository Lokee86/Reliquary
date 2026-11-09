//! Root product settings composed with Switchboard's persisted configuration.
use reliquary_switchboard::{ConfigFile, ConfigObject};
use std::path::{Path, PathBuf};

pub use reliquary_archive::FragmentConfig;
pub use reliquary_retrieval::RetrievalConfig;
pub use reliquary_switchboard::{
    CodexDeviceCode, ConfiguredRuntimeError, CredentialError, CredentialId, CredentialKind,
    DecisionModelEndpoint, DeviceAuthError, EmbeddingModelEndpoint, EmbeddingNormalization,
    GeneralModelEndpoint, GeneralRoute, MasterKeyError, ModelProvider, ModelRoute, ModelSlot,
    SecretString,
};

const FRAGMENTS: &str = "archive.fragments";
const RETRIEVAL: &str = "retrieval.default";
const RUNTIME: &str = "runtime.default";

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct RuntimeSettings {
    pub dream_inference_concurrency: usize,
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        Self {
            dream_inference_concurrency: 12,
        }
    }
}

/// An editable CVCFG configuration.
///
/// Opening a legacy configuration may migrate its key to the OS credential service.
/// Validation does not contact providers.
#[derive(Debug)]
pub struct ReliquaryConfig {
    file: ConfigFile,
    fragments: FragmentConfig,
    retrieval: RetrievalConfig,
    runtime: RuntimeSettings,
}

impl ReliquaryConfig {
    /// Create the standard local/development configuration. Model routes are
    /// populated with the proven project defaults, but no credential objects or
    /// secrets are created. Users authenticate the referenced credential IDs or
    /// replace any route they prefer.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let mut config = Self::new_blank(path);
        config.install_standard_model_routes();
        config
    }

    /// Create only the non-model product defaults. This is intended for tests,
    /// fixtures and advanced callers that want to construct every model route
    /// explicitly rather than inheriting the standard profile.
    pub fn new_blank(path: impl Into<PathBuf>) -> Self {
        let mut config = Self {
            file: ConfigFile::new(path),
            fragments: FragmentConfig::default(),
            retrieval: RetrievalConfig::default(),
            runtime: RuntimeSettings::default(),
        };
        config
            .set_fragments(config.fragments)
            .expect("valid fragment defaults");
        config
            .set_retrieval(config.retrieval)
            .expect("valid retrieval defaults");
        config
            .set_runtime_settings(config.runtime)
            .expect("valid runtime defaults");
        config
    }

    fn install_standard_model_routes(&mut self) {
        let codex = CredentialId::new("codex").expect("valid default credential ID");
        let codex_alt = CredentialId::new("codex-alt").expect("valid default credential ID");
        let openrouter = CredentialId::new("embedding").expect("valid default credential ID");
        let general = |model: &str, credential_id: CredentialId| {
            ModelRoute::General(GeneralRoute {
                endpoint: GeneralModelEndpoint {
                    provider: ModelProvider::OpenAiCodex,
                    model: model.into(),
                    url: String::new(),
                    credential_id,
                },
                reasoning_effort: Some("low"),
            })
        };
        for (slot, route) in [
            (
                ModelSlot::General,
                general("gpt-5.6-luna", codex_alt.clone()),
            ),
            (ModelSlot::Insomnia, general("gpt-6-sol", codex.clone())),
            (
                ModelSlot::InsomniaMetadata,
                general("gpt-6-luna", codex.clone()),
            ),
            (
                ModelSlot::EntityExtraction,
                general("gpt-5.6-sol", codex_alt.clone()),
            ),
            (
                ModelSlot::EntityResolution,
                general("gpt-5.6-sol", codex_alt.clone()),
            ),
            (ModelSlot::Dream, general("gpt-5.6-luna", codex_alt)),
        ] {
            self.set_model(slot, &route)
                .expect("valid standard model route");
        }
        self.set_model(
            ModelSlot::EntityResolutionDecision,
            &ModelRoute::Decision(DecisionModelEndpoint {
                model: "typesafe/jev-1.13".into(),
                url: "https://openrouter.ai/api/alpha/decisions".into(),
                credential_id: openrouter.clone(),
            }),
        )
        .expect("valid standard decision route");
        self.set_model(
            ModelSlot::Embedding,
            &ModelRoute::Embedding(EmbeddingModelEndpoint {
                provider: ModelProvider::OpenAiReady,
                model: "qwen/qwen3-embedding-8b".into(),
                url: "https://openrouter.ai/api/v1/embeddings".into(),
                credential_id: openrouter,
                dimensions: 1024,
                normalization: EmbeddingNormalization::L2,
            }),
        )
        .expect("valid standard embedding route");
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, ConfiguredRuntimeError> {
        let file = ConfigFile::open(path.as_ref())?;
        let fragments = fields(&file, FRAGMENTS)?
            .map(|[turns, overlap]| fragments(turns, overlap))
            .transpose()?
            .unwrap_or_default();
        let retrieval = fields(&file, RETRIEVAL)?
            .map(|[candidates, results, lexical, semantic]| {
                retrieval(candidates, results, lexical, semantic)
            })
            .transpose()?
            .unwrap_or_default();
        let runtime = fields(&file, RUNTIME)?
            .map(|[concurrency]| runtime(concurrency))
            .transpose()?
            .unwrap_or_default();
        Ok(Self {
            file,
            fragments,
            retrieval,
            runtime,
        })
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }
    pub fn fragments(&self) -> FragmentConfig {
        self.fragments
    }
    pub fn retrieval(&self) -> RetrievalConfig {
        self.retrieval
    }
    pub fn runtime_settings(&self) -> RuntimeSettings {
        self.runtime
    }

    pub fn model(&self, slot: ModelSlot) -> Result<Option<ModelRoute>, ConfiguredRuntimeError> {
        self.file.model(slot)
    }

    pub fn set_model(
        &mut self,
        slot: ModelSlot,
        route: &ModelRoute,
    ) -> Result<(), ConfiguredRuntimeError> {
        self.file.set_model(slot, route)
    }

    pub fn clear_model(&mut self, slot: ModelSlot) {
        self.file.clear_model(slot);
    }

    pub fn credentials(&self) -> impl Iterator<Item = (&CredentialId, CredentialKind)> {
        self.file.credentials()
    }

    pub fn add_api_key(&mut self, id: CredentialId, secret: SecretString) {
        self.file.add_api_key(id, secret);
    }

    pub fn remove_credential(&mut self, id: &CredentialId) -> Result<(), ConfiguredRuntimeError> {
        self.file.remove_credential(id)
    }

    pub fn login_codex(
        &mut self,
        id: CredentialId,
        on_code: impl FnOnce(&CodexDeviceCode),
    ) -> Result<(), ConfiguredRuntimeError> {
        self.file.login_codex(id, on_code)
    }

    pub fn refresh_codex(&mut self, id: &CredentialId) -> Result<(), ConfiguredRuntimeError> {
        self.file.refresh_codex(id)
    }

    pub fn set_fragments(&mut self, value: FragmentConfig) -> Result<(), ConfiguredRuntimeError> {
        let fields = [value.turns as u64, value.overlap as u64];
        fragments(fields[0], fields[1])?;
        self.file.set_product_object(FRAGMENTS, object(fields))?;
        self.fragments = value;
        Ok(())
    }

    pub fn set_retrieval(&mut self, value: RetrievalConfig) -> Result<(), ConfiguredRuntimeError> {
        let [candidates, results, lexical, semantic] = [
            value.candidate_limit as u64,
            value.result_limit as u64,
            value.lexical_weight.to_bits(),
            value.semantic_weight.to_bits(),
        ];
        retrieval(candidates, results, lexical, semantic)?;
        self.file
            .set_product_object(RETRIEVAL, object([candidates, results, lexical, semantic]))?;
        self.retrieval = value;
        Ok(())
    }

    pub fn set_runtime_settings(
        &mut self,
        value: RuntimeSettings,
    ) -> Result<(), ConfiguredRuntimeError> {
        let concurrency = value.dream_inference_concurrency as u64;
        runtime(concurrency)?;
        self.file
            .set_product_object(RUNTIME, object([concurrency]))?;
        self.runtime = value;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), ConfiguredRuntimeError> {
        self.configured_runtime().map(|_| ())
    }

    pub fn configured_runtime(&self) -> Result<crate::ConfiguredRuntime, ConfiguredRuntimeError> {
        Ok(crate::ConfiguredRuntime::new(
            self.file.bind()?,
            self.retrieval,
        ))
    }

    pub fn save(&mut self) -> Result<(), ConfiguredRuntimeError> {
        self.file.save()
    }
}

fn invalid(route: &'static str) -> ConfiguredRuntimeError {
    ConfiguredRuntimeError::Invalid {
        route,
        reason: "invalid product configuration object",
    }
}

fn fields<const N: usize>(
    file: &ConfigFile,
    name: &'static str,
) -> Result<Option<[u64; N]>, ConfiguredRuntimeError> {
    let Some(object) = file.object(name) else {
        return Ok(None);
    };
    if object.schema != 1 || object.flags != 0 || object.payload.len() != N * 8 {
        return Err(invalid(name));
    }
    let mut fields = [0; N];
    for (field, bytes) in fields.iter_mut().zip(object.payload.chunks_exact(8)) {
        *field = u64::from_le_bytes(bytes.try_into().unwrap());
    }
    Ok(Some(fields))
}

fn object(fields: impl IntoIterator<Item = u64>) -> ConfigObject {
    ConfigObject {
        schema: 1,
        flags: 0,
        payload: fields.into_iter().flat_map(u64::to_le_bytes).collect(),
    }
}

fn fragments(turns: u64, overlap: u64) -> Result<FragmentConfig, ConfiguredRuntimeError> {
    if turns == 0 || overlap >= turns {
        return Err(invalid(FRAGMENTS));
    }
    Ok(FragmentConfig {
        turns: usize::try_from(turns).map_err(|_| invalid(FRAGMENTS))?,
        overlap: usize::try_from(overlap).map_err(|_| invalid(FRAGMENTS))?,
    })
}

fn retrieval(
    candidates: u64,
    results: u64,
    lexical: u64,
    semantic: u64,
) -> Result<RetrievalConfig, ConfiguredRuntimeError> {
    let config = RetrievalConfig {
        candidate_limit: usize::try_from(candidates).map_err(|_| invalid(RETRIEVAL))?,
        result_limit: usize::try_from(results).map_err(|_| invalid(RETRIEVAL))?,
        lexical_weight: f64::from_bits(lexical),
        semantic_weight: f64::from_bits(semantic),
    };
    reliquary_retrieval::validate_hybrid_request("config", config)
        .map_err(|_| invalid(RETRIEVAL))?;
    Ok(config)
}

fn runtime(concurrency: u64) -> Result<RuntimeSettings, ConfiguredRuntimeError> {
    if concurrency == 0 {
        return Err(invalid(RUNTIME));
    }
    Ok(RuntimeSettings {
        dream_inference_concurrency: usize::try_from(concurrency).map_err(|_| invalid(RUNTIME))?,
    })
}
