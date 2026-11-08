use super::{CredentialId, CredentialsConfig, ModelProvider};
use std::fmt;

/// Secret material supplied by a host for one explicitly selected reference.
#[derive(Clone, Eq, PartialEq)]
pub enum CredentialMaterial {
    ApiKey(String),
    CodexOAuth {
        access_token: String,
        account_id: String,
    },
}

impl fmt::Debug for CredentialMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey(_) => f.write_str("ApiKey([REDACTED])"),
            Self::CodexOAuth { .. } => f.write_str("CodexOAuth([REDACTED])"),
        }
    }
}

/// Looks up a credential reference supplied by the configured runtime.
///
/// The runtime discards resolver error text, which may contain private data.
pub trait CredentialResolver {
    fn resolve(&self, id: &CredentialId) -> Result<CredentialMaterial, String>;
}

/// Reads the explicitly referenced API-key environment variable. For a
/// `codex.PREFIX` reference, reads `PREFIX_ACCESS_TOKEN` and `PREFIX_ACCOUNT_ID`.
#[derive(Clone, Copy, Debug, Default)]
pub struct EnvironmentCredentialResolver;

impl CredentialResolver for EnvironmentCredentialResolver {
    fn resolve(&self, id: &CredentialId) -> Result<CredentialMaterial, String> {
        resolve_environment(id, |name| std::env::var(name).ok())
    }
}

fn resolve_environment(
    id: &CredentialId,
    mut read: impl FnMut(&str) -> Option<String>,
) -> Result<CredentialMaterial, String> {
    let mut read = |name: &str| {
        read(name).ok_or_else(|| "referenced environment variable is unavailable".to_owned())
    };
    if let Some(prefix) = id.0.strip_prefix("codex.") {
        if prefix.is_empty() {
            return Err("Codex environment prefix is empty".into());
        }
        Ok(CredentialMaterial::CodexOAuth {
            access_token: read(&format!("{prefix}_ACCESS_TOKEN"))?,
            account_id: read(&format!("{prefix}_ACCOUNT_ID"))?,
        })
    } else {
        Ok(CredentialMaterial::ApiKey(read(&id.0)?))
    }
}

pub(super) fn resolve_credential(
    credentials: &mut CredentialsConfig,
    resolver: &dyn CredentialResolver,
    id: &CredentialId,
    provider: ModelProvider,
) -> Result<(), &'static str> {
    if !credentials.entries.contains_key(id) {
        let material = resolver
            .resolve(id)
            .map_err(|_| "credential lookup failed")?;
        match (material, provider) {
            (CredentialMaterial::ApiKey(secret), ModelProvider::OpenAiReady) => {
                if !valid_header_token(&secret) {
                    return Err("invalid API-key material");
                }
                credentials.insert_api_key(id.clone(), secret)
            }
            (
                CredentialMaterial::CodexOAuth {
                    access_token,
                    account_id,
                },
                ModelProvider::OpenAiCodex,
            ) => {
                if !valid_header_token(&access_token) || !valid_header_token(&account_id) {
                    return Err("invalid Codex OAuth material");
                }
                credentials.insert_codex_oauth(id.clone(), access_token, account_id)
            }
            _ => return Err("credential has the wrong provider type"),
        }
        .map_err(|_| "invalid credential material")?;
    }
    credentials
        .auth(id, provider)
        .map(|_| ())
        .ok_or("credential has the wrong provider type")
}

fn valid_header_token(value: &str) -> bool {
    !value.is_empty() && value.len() <= 16 * 1024 && value.bytes().all(|b| b.is_ascii_graphic())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_resolver_reads_only_explicit_references_and_redacts_debug() {
        let mut names = Vec::new();
        let material = resolve_environment(&CredentialId::new("codex.TEST").unwrap(), |name| {
            names.push(name.to_owned());
            Some("private-test-value".into())
        })
        .unwrap();
        assert_eq!(names, ["TEST_ACCESS_TOKEN", "TEST_ACCOUNT_ID"]);
        assert_eq!(format!("{material:?}"), "CodexOAuth([REDACTED])");

        let material = resolve_environment(&CredentialId::new("TEST_KEY").unwrap(), |name| {
            assert_eq!(name, "TEST_KEY");
            Some("private-test-value".into())
        })
        .unwrap();
        assert_eq!(format!("{material:?}"), "ApiKey([REDACTED])");
        assert!(resolve_environment(&CredentialId::new("ABSENT").unwrap(), |_| None).is_err());
    }
}
