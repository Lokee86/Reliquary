//! Explicit Codex device authorization. Runtime requests never refresh credentials.
use crate::{CodexRefresh, Credential, SecretString};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{
    StatusCode,
    blocking::{Client, Response},
};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    io::Read,
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

const ISSUER: &str = "https://auth.openai.com";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(15 * 60);

pub struct CodexDeviceCode {
    pub verification_url: String,
    pub user_code: String,
    device_auth_id: SecretString,
    interval: Duration,
}

impl fmt::Debug for CodexDeviceCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CodexDeviceCode([REDACTED])")
    }
}

pub(super) struct DeviceAuth {
    client: Client,
    issuer: String,
    login_timeout: Duration,
}

#[derive(Debug, Eq, PartialEq)]
pub enum DeviceAuthError {
    RequestFailed,
    Status(u16),
    InvalidResponse,
    TimedOut,
}

impl fmt::Display for DeviceAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestFailed => f.write_str("Codex authorization request failed"),
            Self::Status(status) => write!(f, "Codex authorization returned HTTP {status}"),
            Self::InvalidResponse => f.write_str("invalid Codex authorization response"),
            Self::TimedOut => f.write_str("Codex device authorization timed out"),
        }
    }
}

impl std::error::Error for DeviceAuthError {}

impl DeviceAuth {
    pub fn new() -> Result<Self, DeviceAuthError> {
        Self::with_issuer(ISSUER, LOGIN_TIMEOUT)
    }

    fn with_issuer(issuer: &str, login_timeout: Duration) -> Result<Self, DeviceAuthError> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| DeviceAuthError::RequestFailed)?;
        Ok(Self {
            client,
            issuer: issuer.into(),
            login_timeout,
        })
    }

    pub fn login(
        &self,
        on_code: impl FnOnce(&CodexDeviceCode),
    ) -> Result<Credential, DeviceAuthError> {
        let code = self.request_code()?;
        on_code(&code);
        let authorization = self.poll(&code)?;
        let redirect_uri = format!("{}/deviceauth/callback", self.issuer);
        let response = self
            .client
            .post(format!("{}/oauth/token", self.issuer))
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", authorization.authorization_code.0.as_str()),
                ("redirect_uri", &redirect_uri),
                ("client_id", CLIENT_ID),
                ("code_verifier", authorization.code_verifier.0.as_str()),
            ])
            .send()
            .map_err(|_| DeviceAuthError::RequestFailed)?;
        let tokens: Tokens = decode(response)?;
        let id_token = tokens.id_token.ok_or(DeviceAuthError::InvalidResponse)?;
        let account_id = account_id(&id_token.0).ok_or(DeviceAuthError::InvalidResponse)?;
        Ok(Credential::CodexOAuth {
            access_token: tokens.access_token,
            account_id,
            refresh: Some(CodexRefresh {
                id_token,
                refresh_token: tokens.refresh_token,
            }),
        })
    }

    pub fn refresh(&self, credential: &Credential) -> Result<Credential, DeviceAuthError> {
        let Credential::CodexOAuth {
            refresh: Some(saved),
            account_id,
            ..
        } = credential
        else {
            return Err(DeviceAuthError::InvalidResponse);
        };
        #[derive(Serialize)]
        struct Request<'a> {
            client_id: &'a str,
            grant_type: &'a str,
            refresh_token: &'a str,
        }
        let response = self
            .client
            .post(format!("{}/oauth/token", self.issuer))
            .json(&Request {
                client_id: CLIENT_ID,
                grant_type: "refresh_token",
                refresh_token: &saved.refresh_token.0,
            })
            .send()
            .map_err(|_| DeviceAuthError::RequestFailed)?;
        let tokens: Tokens = decode(response)?;
        Ok(Credential::CodexOAuth {
            access_token: tokens.access_token,
            account_id: account_id.clone(),
            refresh: Some(CodexRefresh {
                id_token: saved.id_token.clone(),
                refresh_token: tokens.refresh_token,
            }),
        })
    }

    fn request_code(&self) -> Result<CodexDeviceCode, DeviceAuthError> {
        let response = self
            .client
            .post(format!("{}/api/accounts/deviceauth/usercode", self.issuer))
            .json(&serde_json::json!({ "client_id": CLIENT_ID }))
            .send()
            .map_err(|_| DeviceAuthError::RequestFailed)?;
        #[derive(Deserialize)]
        struct Code {
            device_auth_id: SecretString,
            #[serde(alias = "usercode")]
            user_code: SecretString,
            #[serde(default)]
            interval: serde_json::Value,
        }
        let code: Code = decode(response)?;
        let interval = code
            .interval
            .as_u64()
            .or_else(|| code.interval.as_str()?.trim().parse().ok())
            .unwrap_or(1)
            .max(1);
        Ok(CodexDeviceCode {
            verification_url: format!("{}/codex/device", self.issuer),
            user_code: code.user_code.0.clone(),
            device_auth_id: code.device_auth_id,
            interval: Duration::from_secs(interval),
        })
    }

    fn poll(&self, code: &CodexDeviceCode) -> Result<Authorization, DeviceAuthError> {
        #[derive(Serialize)]
        struct Request<'a> {
            device_auth_id: &'a str,
            user_code: &'a str,
        }
        let started = Instant::now();
        loop {
            let remaining = self.login_timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(DeviceAuthError::TimedOut);
            }
            let response = self
                .client
                .post(format!("{}/api/accounts/deviceauth/token", self.issuer))
                .timeout(remaining.min(REQUEST_TIMEOUT))
                .json(&Request {
                    device_auth_id: &code.device_auth_id.0,
                    user_code: &code.user_code,
                })
                .send()
                .map_err(|_| DeviceAuthError::RequestFailed)?;
            if !matches!(
                response.status(),
                StatusCode::FORBIDDEN | StatusCode::NOT_FOUND
            ) {
                return decode(response);
            }
            thread::sleep(
                code.interval
                    .min(self.login_timeout.saturating_sub(started.elapsed())),
            );
        }
    }
}

#[derive(Deserialize)]
struct Authorization {
    authorization_code: SecretString,
    code_verifier: SecretString,
}

#[derive(Deserialize)]
struct Tokens {
    id_token: Option<SecretString>,
    access_token: SecretString,
    refresh_token: SecretString,
}

impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl Drop for CodexDeviceCode {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.user_code);
    }
}

fn decode<T: serde::de::DeserializeOwned>(response: Response) -> Result<T, DeviceAuthError> {
    if !response.status().is_success() {
        return Err(DeviceAuthError::Status(response.status().as_u16()));
    }
    const LIMIT: usize = 128 * 1024;
    let mut bytes = Zeroizing::new(Vec::with_capacity(LIMIT + 1));
    response
        .take((LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| DeviceAuthError::RequestFailed)?;
    if bytes.len() > LIMIT {
        return Err(DeviceAuthError::InvalidResponse);
    }
    serde_json::from_slice(&bytes).map_err(|_| DeviceAuthError::InvalidResponse)
}

fn account_id(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = Zeroizing::new(URL_SAFE_NO_PAD.decode(payload).ok()?);
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    claims
        .pointer("/https:~1~1api.openai.com~1auth/chatgpt_account_id")?
        .as_str()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests;
