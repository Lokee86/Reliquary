/// Default concurrency shared by runtime configuration and Dream execution.
pub const DEFAULT_DREAM_INFERENCE_CONCURRENCY: usize = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    pub dream_inference_concurrency: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            dream_inference_concurrency: DEFAULT_DREAM_INFERENCE_CONCURRENCY,
        }
    }
}
