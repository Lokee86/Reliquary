use crate::{
    ArchiveError, ArchiveVectorError, CommunityError, CompatibilityProfileError, ContainerError,
    ConversationCompactionError, EchoError, EgoError, EntityError, GraphError, InsomniaError,
    MemoryError, MemoryVectorError, PackedVectorError, RelationshipError, VectorGenerationError,
};
use std::fmt;

#[derive(Debug)]
pub enum CvaError {
    Container(ContainerError),
    Archive(ArchiveError),
    Memories(MemoryError),
    Entities(EntityError),
    Relationships(RelationshipError),
    Graph(GraphError),
    Communities(CommunityError),
    Insomnia(InsomniaError),
    PackedVectors(PackedVectorError),
    MemoryVectors(MemoryVectorError),
    ArchiveVectors(ArchiveVectorError),
    CompatibilityProfiles(CompatibilityProfileError),
    VectorGenerations(VectorGenerationError),
    InteractionStream(String),
    ConversationCompactions(ConversationCompactionError),
    Echo(EchoError),
    Ego(EgoError),
    RelMetadata(String),
    ProjectHistory(String),
    ProjectFile(String),
    Freshness(String),
    Repack(String),
    SemanticGlobalVersionConflict(u64),
    InvalidContainerIdentity(&'static str),
}

impl fmt::Display for CvaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(error) => write!(f, "{error}"),
            Self::Archive(error) => write!(f, "{error}"),
            Self::Memories(error) => write!(f, "{error}"),
            Self::Entities(error) => write!(f, "{error}"),
            Self::Relationships(error) => write!(f, "{error}"),
            Self::Graph(error) => write!(f, "{error}"),
            Self::Communities(error) => write!(f, "{error}"),
            Self::Insomnia(error) => write!(f, "{error}"),
            Self::PackedVectors(error) => write!(f, "{error}"),
            Self::MemoryVectors(error) => write!(f, "{error}"),
            Self::ArchiveVectors(error) => write!(f, "{error}"),
            Self::CompatibilityProfiles(error) => write!(f, "{error}"),
            Self::VectorGenerations(error) => write!(f, "{error}"),
            Self::InteractionStream(error) => write!(f, "interaction stream error: {error}"),
            Self::ConversationCompactions(error) => write!(f, "{error}"),
            Self::Echo(error) => write!(f, "{error}"),
            Self::Ego(error) => write!(f, "{error}"),
            Self::RelMetadata(error) => write!(f, "REL metadata error: {error}"),
            Self::ProjectHistory(error) => write!(f, "project history error: {error}"),
            Self::ProjectFile(error) => write!(f, "project file error: {error}"),
            Self::Freshness(error) => write!(f, "Freshness error: {error}"),
            Self::Repack(error) => write!(f, "REL repack error: {error}"),
            Self::InvalidContainerIdentity(message) => {
                write!(f, "invalid container identity: {message}")
            }
            Self::SemanticGlobalVersionConflict(version) => {
                write!(
                    f,
                    "CVA global version {version} is claimed by multiple semantic mutations"
                )
            }
        }
    }
}

impl std::error::Error for CvaError {}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for CvaError {
            fn from(value: $source) -> Self {
                Self::$variant(value)
            }
        }
    };
}

from_error!(ContainerError, Container);
from_error!(ArchiveError, Archive);
from_error!(MemoryError, Memories);
from_error!(EntityError, Entities);
from_error!(RelationshipError, Relationships);
from_error!(GraphError, Graph);
from_error!(CommunityError, Communities);
from_error!(InsomniaError, Insomnia);
from_error!(PackedVectorError, PackedVectors);
from_error!(MemoryVectorError, MemoryVectors);
from_error!(ArchiveVectorError, ArchiveVectors);
from_error!(CompatibilityProfileError, CompatibilityProfiles);
from_error!(VectorGenerationError, VectorGenerations);
from_error!(ConversationCompactionError, ConversationCompactions);
from_error!(EchoError, Echo);
from_error!(EgoError, Ego);
