use crate::{Credential, CredentialError, CredentialId, CredentialsConfig, SecretString};
use base64::Engine;
use reqwest::StatusCode;
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use serde_json::{Value, json};
use std::fmt;
use std::thread;
use std::time::{Duration, Instant};

#[path = "openai_codex_device_auth/refresh.rs"]
mod refresh;

pub const OPENAI_CODEX_AUTH_ISSUER: &str = "https://auth.openai.com";
pub const OPENAI_CODEX_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const OPENAI_CODEX_DEVICE_LOGIN_TIMEOUT_SECS: u64 = 15 * 60;
const REQUEST_TIMEOUT_SECS: u64 = 30;

#[derive(Clone)]
pub struct OpenAiCodexDeviceAuth {
    client: Client,
    issuer: String,
    client_id: String,
    login_timeout: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenAiCodexDeviceCode {
    pub verification_url: String,
    pub user_code: String,
    device_auth_id: String,
    interval_secs: u64,
}

#[derive(Debug)]
pub enum OpenAiCodexDeviceAuthError {
    Http(String),
    InvalidResponse(&'static str),
    UnexpectedStatus(u16),
    TimedOut,
    Credential(CredentialError),
}

impl fmt::Display for OpenAiCodexDeviceAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(message) => write!(f, "Codex device-auth HTTP failure: {message}"),
            Self::InvalidResponse(message) => {
                write!(f, "invalid Codex device-auth response: {message}")
            }
            Self::UnexpectedStatus(status) => write!(f, "Codex device-auth returned HTTP {status}"),
            Self::TimedOut => write!(f, "Codex device login timed out"),
            Self::Credential(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for OpenAiCodexDeviceAuthError {}

impl From<CredentialError> for OpenAiCodexDeviceAuthError {
    fn from(value: CredentialError) -> Self {
        Self::Credential(value)
    }
}

impl OpenAiCodexDeviceAuth {
    pub fn new() -> Result<Self, OpenAiCodexDeviceAuthError> {
        Self::with_issuer_and_client_id(
            OPENAI_CODEX_AUTH_ISSUER,
            OPENAI_CODEX_OAUTH_CLIENT_ID,
            Duration::from_secs(OPENAI_CODEX_DEVICE_LOGIN_TIMEOUT_SECS),
        )
    }

    pub fn request_device_code(&self) -> Result<OpenAiCodexDeviceCode, OpenAiCodexDeviceAuthError> {
        let url = format!("{}/api/accounts/deviceauth/usercode", self.issuer);
        let payload = serde_json::to_vec(&json!({"client_id": self.client_id}))
            .map_err(|error| OpenAiCodexDeviceAuthError::Http(error.to_string()))?;
        let response = self
            .client
            .post(url)
            .header(CONTENT_TYPE, "application/json")
            .body(payload)
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
            .map_err(|_| OpenAiCodexDeviceAuthError::InvalidResponse("user-code JSON"))?;
        let device_auth_id = required_string(&value, "device_auth_id")?;
        let user_code = value
            .get("user_code")
            .or_else(|| value.get("usercode"))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or(OpenAiCodexDeviceAuthError::InvalidResponse("user_code"))?
            .to_owned();
        let interval_secs = parse_interval(&value).max(1);
        Ok(OpenAiCodexDeviceCode {
            verification_url: format!("{}/codex/device", self.issuer),
            user_code,
            device_auth_id,
            interval_secs,
        })
    }

    pub fn complete_device_code(
        &self,
        code: OpenAiCodexDeviceCode,
        credentials: &mut CredentialsConfig,
        credential_id: CredentialId,
    ) -> Result<(), OpenAiCodexDeviceAuthError> {
        let authorization = self.poll_for_authorization(&code)?;
        let credential = self.exchange_authorization_code(&authorization)?;
        credentials.insert(credential_id, credential);
        Ok(())
    }

    fn poll_for_authorization(
        &self,
        code: &OpenAiCodexDeviceCode,
    ) -> Result<AuthorizationCode, OpenAiCodexDeviceAuthError> {
        let url = format!("{}/api/accounts/deviceauth/token", self.issuer);
        let started = Instant::now();
        loop {
            if started.elapsed() >= self.login_timeout {
                return Err(OpenAiCodexDeviceAuthError::TimedOut);
            }
            let payload = serde_json::to_vec(&json!({
                "device_auth_id": code.device_auth_id,
                "user_code": code.user_code,
            }))
            .map_err(|error| OpenAiCodexDeviceAuthError::Http(error.to_string()))?;
            let response = self
                .client
                .post(&url)
                .header(CONTENT_TYPE, "application/json")
                .body(payload)
                .send()
                .map_err(|error| OpenAiCodexDeviceAuthError::Http(error.to_string()))?;
            let status = response.status();
            if status.is_success() {
                let value = response.json::<Value>().map_err(|_| {
                    OpenAiCodexDeviceAuthError::InvalidResponse("authorization JSON")
                })?;
                required_string(&value, "code_challenge")?;
                return Ok(AuthorizationCode {
                    authorization_code: required_string(&value, "authorization_code")?,
                    code_verifier: required_string(&value, "code_verifier")?,
                });
            }
            if !matches!(status, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND) {
                return Err(OpenAiCodexDeviceAuthError::UnexpectedStatus(
                    status.as_u16(),
                ));
            }
            let remaining = self.login_timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(OpenAiCodexDeviceAuthError::TimedOut);
            }
            thread::sleep(Duration::from_secs(code.interval_secs).min(remaining));
        }
    }

    fn exchange_authorization_code(
        &self,
        authorization: &AuthorizationCode,
    ) -> Result<Credential, OpenAiCodexDeviceAuthError> {
        let url = format!("{}/oauth/token", self.issuer);
        let redirect_uri = format!("{}/deviceauth/callback", self.issuer);
        let response = self
            .client
            .post(url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", authorization.authorization_code.as_str()),
                ("redirect_uri", redirect_uri.as_str()),
                ("client_id", self.client_id.as_str()),
                ("code_verifier", authorization.code_verifier.as_str()),
            ])
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
            .map_err(|_| OpenAiCodexDeviceAuthError::InvalidResponse("token JSON"))?;
        let id_token = required_string(&value, "id_token")?;
        let account_id = chatgpt_account_id(&id_token);
        Ok(Credential::ChatGptOAuth {
            id_token: SecretString::new(id_token)?,
            access_token: SecretString::new(required_string(&value, "access_token")?)?,
            refresh_token: SecretString::new(required_string(&value, "refresh_token")?)?,
            account_id,
        })
    }

    fn with_issuer_and_client_id(
        issuer: &str,
        client_id: &str,
        login_timeout: Duration,
    ) -> Result<Self, OpenAiCodexDeviceAuthError> {
        let issuer = issuer.trim_end_matches('/');
        if issuer.is_empty() || client_id.trim().is_empty() || login_timeout.is_zero() {
            return Err(OpenAiCodexDeviceAuthError::InvalidResponse(
                "device-auth configuration",
            ));
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|error| OpenAiCodexDeviceAuthError::Http(error.to_string()))?;
        Ok(Self {
            client,
            issuer: issuer.to_owned(),
            client_id: client_id.to_owned(),
            login_timeout,
        })
    }
}

struct AuthorizationCode {
    authorization_code: String,
    code_verifier: String,
}

fn required_string(value: &Value, key: &'static str) -> Result<String, OpenAiCodexDeviceAuthError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or(OpenAiCodexDeviceAuthError::InvalidResponse(key))
}

fn parse_interval(value: &Value) -> u64 {
    value
        .get("interval")
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|value| value.trim().parse().ok()))
        })
        .unwrap_or(1)
}

fn chatgpt_account_id(id_token: &str) -> Option<String> {
    let payload = id_token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    value
        .pointer("/https:~1~1api.openai.com~1auth/chatgpt_account_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
pub(crate) fn test_device_auth(
    issuer: &str,
    client_id: &str,
    login_timeout: Duration,
) -> Result<OpenAiCodexDeviceAuth, OpenAiCodexDeviceAuthError> {
    OpenAiCodexDeviceAuth::with_issuer_and_client_id(issuer, client_id, login_timeout)
}
