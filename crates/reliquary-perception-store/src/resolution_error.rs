use reliquary_container::ContainerError;
#[derive(Debug)]
pub enum ResolutionError {
    Container(ContainerError),
    CorruptRecord(&'static str),
    InvalidField(&'static str),
    FieldTooLarge,
    InvalidVersion,
    VersionExhausted,
    RevisionConflict,
    RoutingMetadataConflict,
}
impl From<ContainerError> for ResolutionError {
    fn from(e: ContainerError) -> Self {
        Self::Container(e)
    }
}
impl std::fmt::Display for ResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "resolution error: {self:?}")
    }
}
impl std::error::Error for ResolutionError {}
