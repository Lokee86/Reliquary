use crate::openai_ready_embedding_response::decode_response;
use crate::{
    EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, ModelProvider, ModelRequestAuth,
    ModelSwitchboard, VectorNormalization,
};
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;
use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub const DEFAULT_REMOTE_EMBEDDING_BATCH_SIZE: usize = 16;
pub const DEFAULT_REMOTE_EMBEDDING_CONCURRENCY: usize = 16;
const REQUEST_TIMEOUT_SECS: u64 = 120;

#[derive(Clone)]
pub struct OpenAiReadyEmbeddingEndpoint {
    client: Client,
    url: String,
    model: String,
    dimensions: u32,
    normalization: VectorNormalization,
    auth: ModelRequestAuth,
    batch_size: usize,
    concurrency: usize,
}

impl OpenAiReadyEmbeddingEndpoint {
    pub fn from_switchboard(
        switchboard: &ModelSwitchboard,
    ) -> Result<Self, EmbeddingEndpointError> {
        let route = switchboard
            .embedding()
            .ok_or(EmbeddingEndpointError::InvalidConfiguration(
                "embedding route is not configured",
            ))?;
        if route.provider != ModelProvider::OpenAiReady {
            return Err(EmbeddingEndpointError::InvalidConfiguration(
                "embedding route is not OpenAI-ready",
            ));
        }
        let url = route
            .url
            .clone()
            .ok_or(EmbeddingEndpointError::InvalidConfiguration(
                "embedding URL is missing",
            ))?;
        let auth =
            switchboard
                .embedding_auth()
                .ok_or(EmbeddingEndpointError::InvalidConfiguration(
                    "embedding auth is missing",
                ))?;
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|error| EmbeddingEndpointError::Failure(error.to_string()))?;
        Ok(Self {
            client,
            url,
            model: route.model.clone(),
            dimensions: route.dimensions,
            normalization: route.normalization,
            auth,
            batch_size: DEFAULT_REMOTE_EMBEDDING_BATCH_SIZE,
            concurrency: DEFAULT_REMOTE_EMBEDDING_CONCURRENCY,
        })
    }

    pub fn with_batching(
        mut self,
        batch_size: usize,
        concurrency: usize,
    ) -> Result<Self, EmbeddingEndpointError> {
        if batch_size == 0 || concurrency == 0 {
            return Err(EmbeddingEndpointError::InvalidConfiguration(
                "batch size and concurrency must be non-zero",
            ));
        }
        self.batch_size = batch_size;
        self.concurrency = concurrency;
        Ok(self)
    }

    fn request_batch(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.request_batch_with_limit(mode, inputs, None)
    }

    fn request_batch_with_limit(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
        limit: Option<usize>,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        let body = json!({
            "model": self.model,
            "input": inputs,
            "dimensions": self.dimensions,
            "encoding_format": "float",
            "input_type": input_type(mode),
        });
        let payload = serde_json::to_vec(&body)
            .map_err(|error| EmbeddingEndpointError::Failure(error.to_string()))?;
        let bounded_client;
        let client = if limit.is_some() {
            bounded_client = Client::builder()
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
                .redirect(reqwest::redirect::Policy::none())
                .retry(reqwest::retry::never())
                .build()
                .map_err(|e| EmbeddingEndpointError::Failure(e.to_string()))?;
            &bounded_client
        } else {
            &self.client
        };
        let response = client
            .post(&self.url)
            .header(AUTHORIZATION, self.auth.authorization_header())
            .header(CONTENT_TYPE, "application/json")
            .body(payload)
            .send()
            .map_err(|error| EmbeddingEndpointError::Failure(error.to_string()))?;
        let status = response.status();
        let bytes = if let Some(limit) = limit {
            let cap = u64::try_from(limit)
                .ok()
                .and_then(|n| n.checked_add(1))
                .ok_or(EmbeddingEndpointError::InvalidConfiguration(
                    "response bound overflow",
                ))?;
            let mut bytes = Vec::new();
            response
                .take(cap)
                .read_to_end(&mut bytes)
                .map_err(|e| EmbeddingEndpointError::Failure(e.to_string()))?;
            if bytes.len() > limit {
                return Err(EmbeddingEndpointError::InvalidResponse(
                    "bounded response exceeded",
                ));
            }
            bytes
        } else {
            response
                .bytes()
                .map_err(|e| EmbeddingEndpointError::Failure(e.to_string()))?
                .to_vec()
        };
        if !status.is_success() {
            let message = String::from_utf8_lossy(&bytes);
            return Err(EmbeddingEndpointError::Failure(format!(
                "HTTP {status}: {}",
                truncate(&message, 1024)
            )));
        }
        decode_response(&bytes, inputs.len(), self.dimensions, self.normalization)
    }
}

impl EmbeddingEndpoint for OpenAiReadyEmbeddingEndpoint {
    fn supports_bounded_embedding(&self) -> bool {
        true
    }
    fn embed_bounded(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
        max_response_bytes: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        if inputs.is_empty() || inputs.len() > 16 || max_response_bytes == 0 {
            return Err(EmbeddingEndpointError::InvalidConfiguration(
                "bounded request population",
            ));
        }
        self.request_batch_with_limit(mode, inputs, Some(max_response_bytes))
    }
    fn dimensions(&self) -> u32 {
        self.dimensions
    }

    fn normalization(&self) -> VectorNormalization {
        self.normalization
    }

    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let batches = inputs.len().div_ceil(self.batch_size);
        let workers = self.concurrency.min(batches);
        let next = AtomicUsize::new(0);
        let (tx, rx) = mpsc::channel();
        thread::scope(|scope| {
            for _ in 0..workers {
                let tx = tx.clone();
                let next = &next;
                scope.spawn(move || {
                    loop {
                        let batch = next.fetch_add(1, Ordering::Relaxed);
                        if batch >= batches {
                            break;
                        }
                        let start = batch * self.batch_size;
                        let end = (start + self.batch_size).min(inputs.len());
                        if tx
                            .send((batch, self.request_batch(mode, &inputs[start..end])))
                            .is_err()
                        {
                            break;
                        }
                    }
                });
            }
            drop(tx);
            let mut ordered = vec![None; batches];
            let mut first_error = None;
            for _ in 0..batches {
                let (batch, result) = rx.recv().map_err(|_| {
                    EmbeddingEndpointError::Failure("embedding worker terminated".into())
                })?;
                match result {
                    Ok(vectors) => ordered[batch] = Some(vectors),
                    Err(error) if first_error.is_none() => first_error = Some(error),
                    Err(_) => {}
                }
            }
            if let Some(error) = first_error {
                return Err(error);
            }
            Ok(ordered.into_iter().flatten().flatten().collect())
        })
    }
}

fn input_type(mode: EmbeddingMode) -> &'static str {
    match mode {
        EmbeddingMode::Query => "search_query",
        EmbeddingMode::Document => "search_document",
    }
}

fn truncate(value: &str, max: usize) -> &str {
    value.get(..value.len().min(max)).unwrap_or(value)
}
