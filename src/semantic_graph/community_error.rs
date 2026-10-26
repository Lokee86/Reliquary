use crate::GraphError;
use reliquary_container::ContainerError;
use std::fmt;

#[derive(Debug)]
pub enum CommunityError {
    Container(ContainerError),
    Graph(GraphError),
    MissingOwnerIdentity,
    CorruptRecord(&'static str),
    InvalidSnapshot(&'static str),
    InvalidSemanticName(&'static str),
    GenerationOverflow,
    SizeOverflow,
    Leiden(String),
}

impl fmt::Display for CommunityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(error) => write!(f, "{error}"),
            Self::Graph(error) => write!(f, "{error}"),
            Self::MissingOwnerIdentity => {
                write!(f, "community snapshots require durable owner identity")
            }
            Self::CorruptRecord(field) => write!(f, "corrupt community record: {field}"),
            Self::InvalidSnapshot(reason) => write!(f, "invalid community snapshot: {reason}"),
            Self::InvalidSemanticName(reason) => {
                write!(f, "invalid community semantic name: {reason}")
            }
            Self::GenerationOverflow => write!(f, "community generation exhausted"),
            Self::SizeOverflow => write!(f, "community record size overflow"),
            Self::Leiden(error) => write!(f, "Leiden community detection failed: {error}"),
        }
    }
}

impl std::error::Error for CommunityError {}

impl From<ContainerError> for CommunityError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}

impl From<GraphError> for CommunityError {
    fn from(value: GraphError) -> Self {
        Self::Graph(value)
    }
}
