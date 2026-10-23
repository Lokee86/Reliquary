use crate::model_config::ModelProvider;
use crate::{Credential, CredentialId, CredentialsConfig, SecretString};
use std::collections::BTreeMap;

#[derive(Clone, Eq, PartialEq)]
pub struct ModelRequestAuth {
    authorization: SecretString,
    pub chatgpt_account_id: Option<String>,
}

impl std::fmt::Debug for ModelRequestAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelRequestAuth")
            .field("authorization", &"[REDACTED]")
            .field("chatgpt_account_id", &self.chatgpt_account_id)
            .finish()
    }
}

impl ModelRequestAuth {
    pub fn authorization_header(&self) -> &str {
        self.authorization.expose()
    }

    pub fn apply_to(&self, headers: &mut BTreeMap<String, String>) {
        headers.insert("Authorization".into(), self.authorization.expose().into());
        if let Some(account_id) = &self.chatgpt_account_id {
            headers.insert("ChatGPT-Account-ID".into(), account_id.clone());
        }
    }
}

pub(crate) fn resolve_api_key_auth(
    id: &CredentialId,
    credentials: &CredentialsConfig,
) -> ModelRequestAuth {
    let credential = credentials.get(id).expect("validated credential");
    match credential {
        Credential::ApiKey { api_key } => ModelRequestAuth {
            authorization: bearer(api_key.expose()),
            chatgpt_account_id: None,
        },
        _ => unreachable!("validated API-key credential kind"),
    }
}

pub(crate) fn resolve_auth(
    provider: ModelProvider,
    id: &CredentialId,
    credentials: &CredentialsConfig,
) -> ModelRequestAuth {
    let credential = credentials.get(id).expect("validated credential");
    match (provider, credential) {
        (ModelProvider::OpenAiReady, Credential::ApiKey { api_key }) => ModelRequestAuth {
            authorization: bearer(api_key.expose()),
            chatgpt_account_id: None,
        },
        (
            ModelProvider::OpenAiCodex,
            Credential::ChatGptOAuth {
                access_token,
                account_id,
                ..
            },
        ) => ModelRequestAuth {
            authorization: bearer(access_token.expose()),
            chatgpt_account_id: account_id.clone(),
        },
        _ => unreachable!("validated credential kind"),
    }
}

fn bearer(token: &str) -> SecretString {
    SecretString::new(format!("Bearer {token}")).expect("non-empty bearer token")
}
