use super::*;
use crate::operational::{config_error, config_path};
use reliquary_memory::{
    ConfiguredRuntimeError, CredentialId, DecisionModelEndpoint, EmbeddingModelEndpoint,
    EmbeddingNormalization, GeneralModelEndpoint, GeneralRoute, ModelProvider, ModelRoute,
    ModelSlot, ReliquaryConfig, SecretString,
};
use std::io::{self, BufRead, BufReader, Read};
use zeroize::Zeroizing;

pub(crate) fn run(args: &[String]) -> Result<Value, ProductError> {
    match args.first().map(String::as_str) {
        Some("show") => {
            validate_options(args, 1, &[("--config", false)])?;
            let config = open_or_default(args)?;
            let mut routes = serde_json::Map::new();
            for slot in ModelSlot::ALL {
                let route = config.model(slot).map_err(config_error)?;
                routes.insert(
                    slot.object_key().to_owned(),
                    route.map_or(Value::Null, route_report),
                );
            }
            let retrieval = config.retrieval();
            Ok(json!({
                "operation":"config_show", "path":config.path(), "routes":routes,
                "fragments":config.fragments(), "runtime":config.runtime_settings(),
                "retrieval": {
                    "candidate_limit":retrieval.candidate_limit, "result_limit":retrieval.result_limit,
                    "lexical_weight":retrieval.lexical_weight, "semantic_weight":retrieval.semantic_weight,
                },
                "credentials":inventory(&config),
            }))
        }
        Some("verify") => {
            validate_options(
                args,
                1,
                &[("--config", false), ("--online", false), ("--route", false)],
            )?;
            let online = args.iter().any(|arg| arg == "--online");
            let route = option(args, "--route");
            if route.is_some() && !online {
                return Err(invalid("--route requires --online"));
            }
            let runtime = operational::runtime(config_path(args))?;
            let readiness = match (online, route) {
                (true, Some(route)) => runtime.online_probe_route(route).map_err(config_error)?,
                (true, None) => runtime.online_probe().map_err(config_error)?,
                (false, _) => runtime.readiness(),
            };
            Ok(
                json!({"operation":"config_verify","verified":true,"online":online,"readiness":readiness}),
            )
        }
        Some("credential") => credential(&args[1..]),
        Some("model") => model(&args[1..]),
        _ => Err(invalid(
            "config requires show, verify, credential, or model",
        )),
    }
}

fn open(args: &[String]) -> Result<ReliquaryConfig, ProductError> {
    ReliquaryConfig::open(config_path(args)).map_err(config_error)
}

fn open_or_default(args: &[String]) -> Result<ReliquaryConfig, ProductError> {
    let path = config_path(args);
    match ReliquaryConfig::open(path) {
        Err(ConfiguredRuntimeError::Io(io::ErrorKind::NotFound)) => Ok(ReliquaryConfig::new(path)),
        result => result.map_err(config_error),
    }
}

fn inventory(config: &ReliquaryConfig) -> Vec<Value> {
    config
        .credentials()
        .map(|(id, kind)| json!({"id":id.as_str(),"auth_kind":format!("{kind:?}")}))
        .collect()
}

fn route_report(route: ModelRoute) -> Value {
    match route {
        ModelRoute::General(route) => json!({
            "provider":format!("{:?}", route.endpoint.provider), "model":route.endpoint.model,
            "credential_id":route.endpoint.credential_id.as_str(), "reasoning_effort":route.reasoning_effort,
            "url":(route.endpoint.provider != ModelProvider::OpenAiCodex).then(|| redacted_url(&route.endpoint.url)),
        }),
        ModelRoute::Decision(route) => json!({
            "provider":"OpenAiReady", "model":route.model, "credential_id":route.credential_id.as_str(),
            "url":redacted_url(&route.url),
        }),
        ModelRoute::Embedding(route) => json!({
            "provider":format!("{:?}", route.provider), "model":route.model,
            "credential_id":route.credential_id.as_str(), "dimensions":route.dimensions,
            "normalization":format!("{:?}", route.normalization),
            "url":redacted_url(&route.url),
        }),
    }
}

fn redacted_url(url: &str) -> &str {
    url.split_once('?').map_or(url, |(base, _)| base)
}

fn credential(args: &[String]) -> Result<Value, ProductError> {
    let Some(operation) = args.first().map(String::as_str) else {
        return Err(invalid("missing credential operation"));
    };
    if operation == "list" {
        validate_options(args, 1, &[("--config", false)])?;
        return Ok(
            json!({"operation":"credential_list","credentials":inventory(&open_or_default(args)?)}),
        );
    }
    let allow_create = matches!(operation, "add-api-key" | "login-codex");
    let options: &[(&str, bool)] = match operation {
        "add-api-key" => &[("--config", false), ("--stdin", false)],
        "login-codex" | "refresh-codex" | "remove" => &[("--config", false)],
        _ => return Err(invalid("unsupported credential operation")),
    };
    validate_options(args, 2, options)
        .map_err(|_| invalid("invalid credential command arguments"))?;
    let id = credential_id(&args[1])?;
    let mut config = if allow_create {
        open_or_default(args)?
    } else {
        open(args)?
    };
    match operation {
        "add-api-key" => config.add_api_key(
            id.clone(),
            read_key(args.iter().any(|arg| arg == "--stdin"))?,
        ),
        "login-codex" => config
            .login_codex(id.clone(), |code| {
                eprintln!(
                    "Open {} and enter {}",
                    code.verification_url, code.user_code
                );
            })
            .map_err(config_error)?,
        "refresh-codex" => config.refresh_codex(&id).map_err(config_error)?,
        "remove" => config.remove_credential(&id).map_err(config_error)?,
        _ => unreachable!(),
    }
    config.save().map_err(config_error)?;
    Ok(json!({"operation":operation,"credential_id":id.as_str(),"saved":true}))
}

fn read_key(stdin: bool) -> Result<SecretString, ProductError> {
    const LIMIT: usize = 64 * 1024;
    let read_error = |_| ProductError::new(ProductErrorKind::Io, "could not read API key");
    let key = if stdin {
        let mut key = Zeroizing::new(String::with_capacity(LIMIT + 1));
        // Read one bounded line and erase the original buffer even on failure.
        BufReader::with_capacity(1, io::stdin().lock().take((LIMIT + 1) as u64))
            .read_line(&mut key)
            .map_err(read_error)?;
        if key.len() > LIMIT {
            return Err(invalid("API key exceeds size limit"));
        }
        SecretString::new(key.trim_end_matches(['\r', '\n']))
    } else {
        SecretString::new(rpassword::prompt_password("API key: ").map_err(read_error)?)
    };
    key.map_err(|error| invalid(error.to_string()))
}

fn credential_id(id: &str) -> Result<CredentialId, ProductError> {
    CredentialId::new(id).map_err(|error| invalid(error.to_string()))
}

fn model(args: &[String]) -> Result<Value, ProductError> {
    let operation = args
        .first()
        .ok_or_else(|| invalid("missing model operation"))?;
    let (action, name) = operation
        .split_once('-')
        .ok_or_else(|| invalid("expected set-ROUTE or clear-ROUTE"))?;
    let key = format!("models.{}", name.replace('-', "_"));
    let slot = ModelSlot::ALL
        .into_iter()
        .find(|slot| slot.object_key() == key)
        .ok_or_else(|| invalid("unknown model route"))?;
    let mut config = match action {
        "clear" => {
            validate_options(args, 1, &[("--config", false)])?;
            let mut config = open(args)?;
            config.clear_model(slot);
            config
        }
        "set" => {
            let route = model_route(slot, args)?;
            let mut config = open_or_default(args)?;
            config.set_model(slot, &route).map_err(config_error)?;
            config
        }
        _ => return Err(invalid("expected set-ROUTE or clear-ROUTE")),
    };
    config.save().map_err(config_error)?;
    Ok(json!({"operation":operation,"route":slot.object_key(),"saved":true}))
}

fn model_route(slot: ModelSlot, args: &[String]) -> Result<ModelRoute, ProductError> {
    let mut options = vec![
        ("--config", false),
        ("--model", false),
        ("--url", false),
        ("--credential", false),
    ];
    if !slot.is_decision() {
        options.push(("--provider", false));
        if slot == ModelSlot::Embedding {
            options.extend([("--dimensions", false), ("--normalization", false)]);
        } else {
            options.push(("--reasoning", false));
        }
    }
    validate_options(args, 1, &options)?;
    let model = required_option(args, "--model")?.to_owned();
    let credential_id = credential_id(required_option(args, "--credential")?)?;
    if slot.is_decision() {
        let url = required_option(args, "--url")?.to_owned();
        return Ok(ModelRoute::Decision(DecisionModelEndpoint {
            model,
            url,
            credential_id,
        }));
    }
    let provider = match option(args, "--provider") {
        Some("openai-codex") => ModelProvider::OpenAiCodex,
        Some("openai-ready") => ModelProvider::OpenAiReady,
        None if slot == ModelSlot::Embedding => ModelProvider::OpenAiReady,
        _ => return Err(invalid("--provider must be openai-codex or openai-ready")),
    };
    let url = option(args, "--url").unwrap_or_default().to_owned();
    if slot == ModelSlot::Embedding {
        let normalization = match option(args, "--normalization").unwrap_or("l2") {
            "l2" => EmbeddingNormalization::L2,
            "none" => EmbeddingNormalization::None,
            _ => return Err(invalid("--normalization must be l2 or none")),
        };
        let dimensions = number_option(args, "--dimensions")?
            .ok_or_else(|| invalid("--dimensions is required"))?;
        return Ok(ModelRoute::Embedding(EmbeddingModelEndpoint {
            provider,
            model,
            url,
            credential_id,
            dimensions,
            normalization,
        }));
    }
    let reasoning_effort = option(args, "--reasoning")
        .map(|value| {
            ["none", "minimal", "low", "medium", "high", "xhigh", "max"]
                .into_iter()
                .find(|effort| *effort == value)
                .ok_or_else(|| invalid("invalid --reasoning value"))
        })
        .transpose()?;
    Ok(ModelRoute::General(GeneralRoute {
        endpoint: GeneralModelEndpoint {
            provider,
            model,
            url,
            credential_id,
        },
        reasoning_effort,
    }))
}
