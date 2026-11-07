use reliquary_owner_session::{OwnerFailureKind, OwnerSessionError};
use reliquary_runtime_gateway::GatewayError;
use serde::Serialize;
use std::{fmt, io};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductErrorKind {
    WrongFormat,
    Admission,
    Unauthorized,
    Revoked,
    Unavailable,
    Unsupported,
    Stale,
    Conflict,
    WriterUnavailable,
    PublicationCollision,
    Io,
    Verification,
    InvalidInput,
}

#[derive(Debug)]
pub struct ProductError {
    kind: ProductErrorKind,
    message: String,
}

impl ProductError {
    pub fn new(kind: ProductErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    pub fn kind(&self) -> ProductErrorKind {
        self.kind
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::new(ProductErrorKind::InvalidInput, message)
    }
}

impl fmt::Display for ProductError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ProductError {}

fn owner_kind(error: &OwnerSessionError) -> ProductErrorKind {
    match error.failure_kind() {
        OwnerFailureKind::UnsupportedFormat | OwnerFailureKind::WrongFileKind => {
            ProductErrorKind::WrongFormat
        }
        OwnerFailureKind::Admission => ProductErrorKind::Admission,
        OwnerFailureKind::WriterConflict => ProductErrorKind::WriterUnavailable,
        OwnerFailureKind::Io => ProductErrorKind::Io,
        OwnerFailureKind::Corruption => ProductErrorKind::Verification,
    }
}

impl From<OwnerSessionError> for ProductError {
    fn from(error: OwnerSessionError) -> Self {
        Self::new(owner_kind(&error), error.to_string())
    }
}

impl From<GatewayError> for ProductError {
    fn from(error: GatewayError) -> Self {
        let kind = match &error {
            GatewayError::OwnerSession(inner) => owner_kind(inner),
            GatewayError::UnauthorizedRel { .. }
            | GatewayError::UnauthorizedPhy { .. }
            | GatewayError::UnauthorizedQueueSubmission { .. } => ProductErrorKind::Unauthorized,
            GatewayError::CredentialRevoked => ProductErrorKind::Revoked,
            GatewayError::WrongOwnerKind(_) => ProductErrorKind::WrongFormat,
            GatewayError::AdmissionOwnerMismatch { .. } => ProductErrorKind::Admission,
            GatewayError::DuplicateOwner(_) => ProductErrorKind::Conflict,
            GatewayError::WriterUnavailable => ProductErrorKind::WriterUnavailable,
            GatewayError::StaleRetrieval => ProductErrorKind::Stale,
            GatewayError::UnknownOwner(_) => ProductErrorKind::Unavailable,
            GatewayError::UnsupportedHostCapability(_)
            | GatewayError::IncompleteHostContract(_) => ProductErrorKind::Unsupported,
            GatewayError::InvalidPrincipal
            | GatewayError::InvalidCredential
            | GatewayError::DuplicateCredential
            | GatewayError::InvalidGenerationSubscription
            | GatewayError::InvalidHandoffReceipt
            | GatewayError::InvalidPreparedRetrieval => ProductErrorKind::InvalidInput,
            _ => ProductErrorKind::Verification,
        };
        Self::new(kind, error.to_string())
    }
}

impl From<io::Error> for ProductError {
    fn from(error: io::Error) -> Self {
        Self::new(ProductErrorKind::Io, error.to_string())
    }
}
