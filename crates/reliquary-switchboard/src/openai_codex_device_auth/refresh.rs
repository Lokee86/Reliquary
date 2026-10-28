use super::{OpenAiCodexDeviceAuth, OpenAiCodexDeviceAuthError, required_string};
use crate::{Credential, CredentialError, CredentialId, CredentialsConfig};
use serde_json::{Value, json};

impl OpenAiCodexDeviceAuth {
    pub fn refresh_credential(
        &self,
        credentials: &mut CredentialsConfig,
        credential_id: &CredentialId,
    ) -> Result<(), OpenAiCodexDeviceAuthError> {
        let credential = credentials
            .get(credential_id)
            .cloned()
            .ok_or_else(|| CredentialError::MissingCredential(credential_id.as_str().into()))?;
        let Credential::ChatGptOAuth {
            id_token,
            refresh_token,
            account_id,
            ..
        } = credential
        else {
            return Err(CredentialError::WrongAuthKind(credential_id.as_str().into()).into());
        };
        let response = self
            .client
            .post(format!("{}/oauth/token", self.issuer))
            .json(&json!({
                "client_id": self.client_id,
                "grant_type": "refresh_token",
                "refresh_token": refresh_token.expose(),
            }))
            .send()
            .map_err(|error| OpenAiCodexDeviceAuthError::Http(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(OpenAiCodexDeviceAuthError::UnexpectedStatus(
                status.as_u16(),
            ));
        }
        let value = response
            .json::<Value>()
            .map_err(|_| OpenAiCodexDeviceAuthError::InvalidResponse("refresh token JSON"))?;
        credentials.insert_chatgpt_oauth(
            credential_id.clone(),
            id_token.expose(),
            required_string(&value, "access_token")?,
            required_string(&value, "refresh_token")?,
            account_id,
        )?;
        Ok(())
    }
}
