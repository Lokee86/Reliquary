use crate::ContainerError;
use std::fmt;

#[derive(Debug)]
pub(crate) enum InteractionStreamError {
    Container(ContainerError),
    Invalid(String),
}

impl fmt::Display for InteractionStreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(error) => write!(f, "{error}"),
            Self::Invalid(message) => write!(f, "interaction stream error: {message}"),
        }
    }
}

impl std::error::Error for InteractionStreamError {}

impl From<ContainerError> for InteractionStreamError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}
