use crate::{
    CommunityError, CompatibilityProfileError, EgoError, EntityError, GraphError, MemoryError,
    MemoryVectorError, PackedVectorError, RelationshipError,
};
use reliquary_container::ContainerError;
use std::fmt;

#[derive(Debug)]
pub enum PhylacteryError {
    Container(ContainerError),
    Memories(MemoryError),
    Entities(EntityError),
    Relationships(RelationshipError),
    Graph(GraphError),
    Communities(CommunityError),
    PackedVectors(PackedVectorError),
    MemoryVectors(MemoryVectorError),
    CompatibilityProfiles(CompatibilityProfileError),
    Ego(EgoError),
    Profile(String),
    Repack(String),
    InvalidContainerIdentity(&'static str),
    SemanticGlobalVersionConflict(u64),
}

impl fmt::Display for PhylacteryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(error) => write!(f, "{error}"),
            Self::Memories(error) => write!(f, "{error}"),
            Self::Entities(error) => write!(f, "{error}"),
            Self::Relationships(error) => write!(f, "{error}"),
            Self::Graph(error) => write!(f, "{error}"),
            Self::Communities(error) => write!(f, "{error}"),
            Self::PackedVectors(error) => write!(f, "{error}"),
            Self::MemoryVectors(error) => write!(f, "{error}"),
            Self::CompatibilityProfiles(error) => write!(f, "{error}"),
            Self::Ego(error) => write!(f, "{error}"),
            Self::Profile(error) => write!(f, "Phylactery profile error: {error}"),
            Self::Repack(error) => write!(f, "PHY repack error: {error}"),
            Self::InvalidContainerIdentity(message) => {
                write!(f, "invalid container identity: {message}")
            }
            Self::SemanticGlobalVersionConflict(version) => {
                write!(
                    f,
                    "Phylactery global version {version} is claimed by multiple semantic mutations"
                )
            }
        }
    }
}

impl std::error::Error for PhylacteryError {}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for PhylacteryError {
            fn from(value: $source) -> Self {
                Self::$variant(value)
            }
        }
    };
}

from_error!(ContainerError, Container);
from_error!(MemoryError, Memories);
from_error!(EntityError, Entities);
from_error!(RelationshipError, Relationships);
impl From<reliquary_perception_store::RelationshipError> for PhylacteryError {
    fn from(value: reliquary_perception_store::RelationshipError) -> Self {
        Self::Relationships(value.into())
    }
}
from_error!(GraphError, Graph);
from_error!(CommunityError, Communities);
from_error!(PackedVectorError, PackedVectors);

impl From<reliquary_vector_store::PackedVectorError> for PhylacteryError {
    fn from(value: reliquary_vector_store::PackedVectorError) -> Self {
        Self::PackedVectors(value.into())
    }
}
from_error!(MemoryVectorError, MemoryVectors);
from_error!(CompatibilityProfileError, CompatibilityProfiles);
from_error!(EgoError, Ego);

impl From<reliquary_ego_store::EgoError> for PhylacteryError {
    fn from(value: reliquary_ego_store::EgoError) -> Self {
        Self::Ego(value.into())
    }
}

impl From<reliquary_perception_store::EntityError> for PhylacteryError {
    fn from(value: reliquary_perception_store::EntityError) -> Self {
        Self::Entities(value.into())
    }
}

impl From<reliquary_perception_store::ResolutionError> for PhylacteryError {
    fn from(e: reliquary_perception_store::ResolutionError) -> Self {
        crate::entity_resolution_boundary::memory_error(e).into()
    }
}
