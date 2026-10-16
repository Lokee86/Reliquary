use crate::config_args::{ConfigCommand, CredentialCommand, ModelCommand};
use crate::util::{credential_id, normalization, provider, read_secret, reasoning};
use anyhow::{Result, anyhow};
use reliquary_memory::{
    DecisionModelEndpoint, EmbeddingModelEndpoint, GeneralModelEndpoint, OpenAiCodexDeviceAuth,
    ReliquaryConfig,
};
use std::path::Path;

#[cfg(test)]
#[path = "config_route_tests.rs"]
mod route_tests;

pub fn run(path: &Path, command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Show => show(path),
        ConfigCommand::Verify => verify(path),
        ConfigCommand::Credential { command } => credential(path, command),
        ConfigCommand::Model { command } => model(path, command),
    }
}

fn show(path: &Path) -> Result<()> {
    let config = load(path)?;
    println!("config: {}", path.display());
    println!(
        "fragments: turns={} overlap={}",
        config.fragments.turns, config.fragments.overlap
    );
    println!(
        "retrieval: candidates={} results={} lexical={} semantic={}",
        config.retrieval.candidate_limit,
        config.retrieval.result_limit,
        config.retrieval.lexical_weight,
        config.retrieval.semantic_weight
    );
    show_general(config.models.general.as_ref());
    show_insomnia(config.models.insomnia.as_ref());
    show_insomnia_metadata(config.models.insomnia_metadata.as_ref());
    show_entity_extraction(config.models.entity_extraction.as_ref());
    show_entity_resolution_decision(config.models.entity_resolution_decision.as_ref());
    show_memory_retrieval_decision(config.models.memory_retrieval_decision.as_ref());
    show_entity_resolution(config.models.entity_resolution.as_ref());
    show_dream(config.models.dream.as_ref());
    show_embedding(config.models.embedding.as_ref());
    println!("credentials:");
    for id in config.credentials.ids() {
        let kind = config.credentials.get(id).map(|value| value.auth_kind());
        println!("  {}  {:?}", id.as_str(), kind.unwrap());
    }
    Ok(())
}

fn verify(path: &Path) -> Result<()> {
    let config = ReliquaryConfig::open(path)?;
    config.validate_runtime()?;
    println!("config ok: {}", path.display());
    Ok(())
}

fn credential(path: &Path, command: CredentialCommand) -> Result<()> {
    let mut config = load(path)?;
    match command {
        CredentialCommand::List => {
            for id in config.credentials.ids() {
                let credential = config.credentials.get(id).unwrap();
                println!("{}\t{:?}", id.as_str(), credential.auth_kind());
            }
            return Ok(());
        }
        CredentialCommand::AddApiKey { id, stdin } => {
            let id = credential_id(id)?;
            let secret = read_secret("API key: ", stdin)?;
            config.credentials.insert_api_key(id.clone(), secret)?;
            config.save()?;
            println!("saved API-key credential: {}", id.as_str());
        }
        CredentialCommand::LoginCodex { id } => {
            let id = credential_id(id)?;
            let auth = OpenAiCodexDeviceAuth::new()?;
            let code = auth.request_device_code()?;
            println!("Open this URL in your browser:");
            println!("  {}", code.verification_url);
            println!("Enter this one-time code:");
            println!("  {}", code.user_code);
            println!("Waiting for ChatGPT authorization...");
            auth.complete_device_code(code, &mut config.credentials, id.clone())?;
            config.save()?;
            println!("saved ChatGPT OAuth credential: {}", id.as_str());
        }
        CredentialCommand::RefreshCodex { id } => {
            let id = credential_id(id)?;
            OpenAiCodexDeviceAuth::new()?.refresh_credential(&mut config.credentials, &id)?;
            config.save()?;
            println!("refreshed ChatGPT OAuth credential: {}", id.as_str());
        }
        CredentialCommand::Remove { id } => {
            let id = credential_id(id)?;
            if config.credentials.remove(&id).is_none() {
                return Err(anyhow!("credential not found: {}", id.as_str()));
            }
            config.validate_runtime()?;
            config.save()?;
            println!("removed credential: {}", id.as_str());
        }
    }
    Ok(())
}

fn model(path: &Path, command: ModelCommand) -> Result<()> {
    let mut config = load(path)?;
    match command {
        ModelCommand::SetGeneral {
            provider: p,
            model,
            credential,
            url,
            reasoning: r,
        } => {
            config.models.general = Some(GeneralModelEndpoint {
                provider: provider(p),
                model,
                url,
                credential_id: credential_id(credential)?,
                reasoning_effort: r.map(reasoning),
            });
            validate_runtime(&config)?;
        }
        ModelCommand::ClearGeneral => config.models.general = None,
        ModelCommand::SetInsomnia {
            provider: p,
            model,
            credential,
            url,
            reasoning: r,
        } => {
            config.models.insomnia = Some(GeneralModelEndpoint {
                provider: provider(p),
                model,
                url,
                credential_id: credential_id(credential)?,
                reasoning_effort: r.map(reasoning),
            });
            validate_runtime(&config)?;
        }
        ModelCommand::ClearInsomnia => config.models.insomnia = None,
        ModelCommand::SetInsomniaMetadata {
            provider: p,
            model,
            credential,
            url,
            reasoning: r,
        } => {
            config.models.insomnia_metadata = Some(GeneralModelEndpoint {
                provider: provider(p),
                model,
                url,
                credential_id: credential_id(credential)?,
                reasoning_effort: r.map(reasoning),
            });
            validate_runtime(&config)?;
        }
        ModelCommand::ClearInsomniaMetadata => config.models.insomnia_metadata = None,
        ModelCommand::SetEntityExtraction {
            provider: p,
            model,
            credential,
            url,
            reasoning: r,
        } => {
            config.models.entity_extraction = Some(GeneralModelEndpoint {
                provider: provider(p),
                model,
                url,
                credential_id: credential_id(credential)?,
                reasoning_effort: r.map(reasoning),
            });
            validate_runtime(&config)?;
        }
        ModelCommand::ClearEntityExtraction => config.models.entity_extraction = None,
        ModelCommand::SetEntityResolutionDecision {
            model,
            credential,
            url,
        } => {
            config.models.entity_resolution_decision = Some(DecisionModelEndpoint {
                model,
                url,
                credential_id: credential_id(credential)?,
            });
            validate_runtime(&config)?;
        }
        ModelCommand::ClearEntityResolutionDecision => {
            config.models.entity_resolution_decision = None
        }
        ModelCommand::SetMemoryRetrievalDecision {
            model,
            credential,
            url,
        } => {
            config.models.memory_retrieval_decision = Some(DecisionModelEndpoint {
                model,
                url,
                credential_id: credential_id(credential)?,
            });
            validate_runtime(&config)?;
        }
        ModelCommand::ClearMemoryRetrievalDecision => {
            config.models.memory_retrieval_decision = None
        }
        ModelCommand::SetEntityResolution {
            provider: p,
            model,
            credential,
            url,
            reasoning: r,
        } => {
            config.models.entity_resolution = Some(GeneralModelEndpoint {
                provider: provider(p),
                model,
                url,
                credential_id: credential_id(credential)?,
                reasoning_effort: r.map(reasoning),
            });
            validate_runtime(&config)?;
        }
        ModelCommand::ClearEntityResolution => config.models.entity_resolution = None,
        ModelCommand::SetDream {
            provider: p,
            model,
            credential,
            url,
            reasoning: r,
        } => {
            config.models.dream = Some(GeneralModelEndpoint {
                provider: provider(p),
                model,
                url,
                credential_id: credential_id(credential)?,
                reasoning_effort: r.map(reasoning),
            });
            validate_runtime(&config)?;
        }
        ModelCommand::ClearDream => config.models.dream = None,
        ModelCommand::SetEmbedding {
            provider: p,
            model,
            credential,
            url,
            dimensions,
            normalization: n,
        } => {
            config.models.embedding = Some(EmbeddingModelEndpoint {
                provider: provider(p),
                model,
                url: Some(url),
                credential_id: credential_id(credential)?,
                dimensions,
                normalization: normalization(n),
            });
            validate_runtime(&config)?;
        }
        ModelCommand::ClearEmbedding => config.models.embedding = None,
    }
    config.save()?;
    println!("saved model configuration: {}", path.display());
    Ok(())
}

fn load(path: &Path) -> Result<ReliquaryConfig> {
    if path.exists() {
        Ok(ReliquaryConfig::open(path)?)
    } else {
        Ok(ReliquaryConfig::new(path))
    }
}

fn validate_runtime(config: &ReliquaryConfig) -> Result<()> {
    config.validate_runtime()?;
    Ok(())
}

fn show_general(value: Option<&GeneralModelEndpoint>) {
    match value {
        Some(endpoint) => println!(
            "general: provider={:?} model={} reasoning={} url={} credential={}",
            endpoint.provider,
            endpoint.model,
            endpoint
                .reasoning_effort
                .map(|value| value.as_str())
                .unwrap_or("<unset>"),
            endpoint.url.as_deref().unwrap_or("<provider-owned>"),
            endpoint.credential_id.as_str()
        ),
        None => println!("general: <unset>"),
    }
}

fn show_insomnia(value: Option<&GeneralModelEndpoint>) {
    match value {
        Some(endpoint) => println!(
            "insomnia: provider={:?} model={} reasoning={} url={} credential={}",
            endpoint.provider,
            endpoint.model,
            endpoint
                .reasoning_effort
                .map(|value| value.as_str())
                .unwrap_or("<unset>"),
            endpoint.url.as_deref().unwrap_or("<provider-owned>"),
            endpoint.credential_id.as_str()
        ),
        None => println!("insomnia: <unset; falls back to general>"),
    }
}

fn show_insomnia_metadata(value: Option<&GeneralModelEndpoint>) {
    match value {
        Some(endpoint) => println!(
            "insomnia_metadata: provider={:?} model={} reasoning={} url={} credential={}",
            endpoint.provider,
            endpoint.model,
            endpoint
                .reasoning_effort
                .map(|value| value.as_str())
                .unwrap_or("<unset>"),
            endpoint.url.as_deref().unwrap_or("<provider-owned>"),
            endpoint.credential_id.as_str()
        ),
        None => println!("insomnia_metadata: <unset>"),
    }
}

fn show_entity_extraction(value: Option<&GeneralModelEndpoint>) {
    match value {
        Some(endpoint) => println!(
            "entity_extraction: provider={:?} model={} reasoning={} url={} credential={}",
            endpoint.provider,
            endpoint.model,
            endpoint
                .reasoning_effort
                .map(|value| value.as_str())
                .unwrap_or("<unset>"),
            endpoint.url.as_deref().unwrap_or("<provider-owned>"),
            endpoint.credential_id.as_str()
        ),
        None => println!("entity_extraction: <unset>"),
    }
}

fn show_entity_resolution_decision(value: Option<&DecisionModelEndpoint>) {
    match value {
        Some(endpoint) => println!(
            "entity_resolution_decision: model={} url={} credential={}",
            endpoint.model,
            endpoint.url,
            endpoint.credential_id.as_str()
        ),
        None => println!("entity_resolution_decision: <unset>"),
    }
}

fn show_memory_retrieval_decision(value: Option<&DecisionModelEndpoint>) {
    match value {
        Some(endpoint) => println!(
            "memory_retrieval_decision: model={} url={} credential={}",
            endpoint.model,
            endpoint.url,
            endpoint.credential_id.as_str()
        ),
        None => println!("memory_retrieval_decision: <unset>"),
    }
}

fn show_entity_resolution(value: Option<&GeneralModelEndpoint>) {
    match value {
        Some(endpoint) => println!(
            "entity_resolution_fallback: provider={:?} model={} reasoning={} url={} credential={}",
            endpoint.provider,
            endpoint.model,
            endpoint
                .reasoning_effort
                .map(|value| value.as_str())
                .unwrap_or("<unset>"),
            endpoint.url.as_deref().unwrap_or("<provider-owned>"),
            endpoint.credential_id.as_str()
        ),
        None => println!("entity_resolution_fallback: <unset>"),
    }
}

fn show_dream(value: Option<&GeneralModelEndpoint>) {
    match value {
        Some(endpoint) => println!(
            "dream: provider={:?} model={} reasoning={} url={} credential={}",
            endpoint.provider,
            endpoint.model,
            endpoint
                .reasoning_effort
                .map(|value| value.as_str())
                .unwrap_or("<unset>"),
            endpoint.url.as_deref().unwrap_or("<provider-owned>"),
            endpoint.credential_id.as_str()
        ),
        None => println!("dream: <unset; falls back to general>"),
    }
}

fn show_embedding(value: Option<&EmbeddingModelEndpoint>) {
    match value {
        Some(endpoint) => println!(
            "embedding: provider={:?} model={} url={} credential={} dimensions={} normalization={:?}",
            endpoint.provider,
            endpoint.model,
            endpoint.url.as_deref().unwrap_or("<unset>"),
            endpoint.credential_id.as_str(),
            endpoint.dimensions,
            endpoint.normalization
        ),
        None => println!("embedding: <unset>"),
    }
}
