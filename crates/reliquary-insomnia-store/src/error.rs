use reliquary_container::ContainerError;
use std::fmt;

#[derive(Debug)]
pub enum InsomniaError {
    Container(ContainerError),
    CorruptRecord(&'static str),
    InvalidField(&'static str),
    MissingFormat,
    ConflictingFormat,
    MissingEpisode,
    ConflictingEpisode,
    MissingWork,
    InvalidLease,
    LeaseExpired,
    InvalidTransition,
}
impl fmt::Display for InsomniaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "insomnia error: {self:?}")
    }
}
impl std::error::Error for InsomniaError {}
impl From<ContainerError> for InsomniaError {
    fn from(v: ContainerError) -> Self {
        Self::Container(v)
    }
}
