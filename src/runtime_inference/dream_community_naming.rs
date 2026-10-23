use crate::dream_community_naming_context::{build_payload, parse_name};
use crate::dream_community_naming_schema::{
    DREAM_COMMUNITY_NAMING_SYSTEM_PROMPT, dream_community_naming_schema,
};
use crate::{CommunitySemanticName, DreamCommunityNamingError, GeneralEndpoint, Memory};

const DREAM_COMMUNITY_NAMING_RETRY_LIMIT: usize = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DreamCommunityNamingResult {
    pub attempted: usize,
    pub named: Vec<CommunitySemanticName>,
    pub remaining: usize,
}

pub struct DreamCommunityNamer<E> {
    endpoint: E,
    system_prompt: String,
}

impl<E: GeneralEndpoint> DreamCommunityNamer<E> {
    pub fn new(endpoint: E) -> Self {
        Self {
            endpoint,
            system_prompt: DREAM_COMMUNITY_NAMING_SYSTEM_PROMPT.to_owned(),
        }
    }

    pub fn with_system_prompt(endpoint: E, system_prompt: impl Into<String>) -> Self {
        Self {
            endpoint,
            system_prompt: system_prompt.into(),
        }
    }

    pub fn model(&self) -> &str {
        self.endpoint.model()
    }

    pub(crate) fn generate_name(
        &self,
        memories: &[Memory],
    ) -> Result<String, DreamCommunityNamingError> {
        let payload = build_payload(memories)?;
        for attempt in 0..=DREAM_COMMUNITY_NAMING_RETRY_LIMIT {
            match self.endpoint.complete_json(
                &self.system_prompt,
                &payload,
                "dream_community_name",
                &dream_community_naming_schema(),
            ) {
                Ok(output) => match parse_name(&output) {
                    Ok(name) => return Ok(name),
                    Err(error) if attempt == DREAM_COMMUNITY_NAMING_RETRY_LIMIT => {
                        return Err(error);
                    }
                    Err(_) => continue,
                },
                Err(error) if error.is_backpressure() => {
                    return Err(DreamCommunityNamingError::Inference(error));
                }
                Err(error) if attempt == DREAM_COMMUNITY_NAMING_RETRY_LIMIT => {
                    return Err(DreamCommunityNamingError::Inference(error));
                }
                Err(_) => continue,
            }
        }
        unreachable!("Dream community naming retry loop always returns")
    }
}
