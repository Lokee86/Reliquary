use crate::{
    ArchiveError, CompatibilityProfileError, CvaError, CvaReconcileConflict, EntityError,
    GraphError, InsomniaError, MemoryError, RelationshipError,
};
use reliquary_container::ContainerError;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum CvaReconcileError {
    Cva(CvaError),
    Container(ContainerError),
    Archive(ArchiveError),
    Memories(MemoryError),
    Entities(EntityError),
    Relationships(RelationshipError),
    Graph(GraphError),
    Insomnia(InsomniaError),
    CompatibilityProfile(CompatibilityProfileError),
    Io(io::Error),
    MissingOwnerId(&'static str),
    OwnerMismatch {
        left: String,
        right: String,
    },
    HistoryGenerationConflict {
        left: crate::HistoryGenerationId,
        right: crate::HistoryGenerationId,
    },
    OutputExists,
    UnsupportedSemanticOwner(&'static str),
    InvalidInsomniaCompletion(&'static str),
    InvalidMemoryVersionRecord,
    InvalidGraphVersionRecord,
    MissingCompletionMemory,
    Conflict(CvaReconcileConflict),
    PromotionPathsMustDiffer,
    CanonicalChangedDuringPromotion,
    PromotionFinalizationFailed(String),
    PromotionRecoveryFailed {
        failure: String,
        recovery: String,
    },
}

impl fmt::Display for CvaReconcileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cva(error) => write!(f, "{error}"),
            Self::Container(error) => write!(f, "{error}"),
            Self::Archive(error) => write!(f, "{error}"),
            Self::Memories(error) => write!(f, "{error}"),
            Self::Entities(error) => write!(f, "{error}"),
            Self::Relationships(error) => write!(f, "{error}"),
            Self::Graph(error) => write!(f, "{error}"),
            Self::Insomnia(error) => write!(f, "{error}"),
            Self::CompatibilityProfile(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::MissingOwnerId(side) => {
                write!(f, "{side} Reliquary has no durable owner ID")
            }
            Self::OwnerMismatch { left, right } => {
                write!(f, "owner mismatch: left={left} right={right}")
            }
            Self::HistoryGenerationConflict { left, right } => {
                write!(f, "history generation conflict: left={left} right={right}")
            }
            Self::OutputExists => write!(f, "reconciliation output already exists"),
            Self::UnsupportedSemanticOwner(owner) => {
                write!(f, "reconciliation does not yet support divergent {owner}")
            }
            Self::InvalidInsomniaCompletion(message) => {
                write!(f, "invalid Insomnia completion: {message}")
            }
            Self::InvalidMemoryVersionRecord => {
                write!(f, "memory version points at a non-memory record")
            }
            Self::InvalidGraphVersionRecord => {
                write!(f, "graph version points at a non-graph mutation record")
            }
            Self::MissingCompletionMemory => {
                write!(f, "Insomnia completion references a missing Memory")
            }
            Self::Conflict(conflict) => {
                write!(
                    f,
                    "CVA reconciliation conflict ({}): {conflict:?}",
                    conflict.kind()
                )
            }
            Self::PromotionPathsMustDiffer => {
                write!(f, "canonical and conflicted CVA paths must differ")
            }
            Self::CanonicalChangedDuringPromotion => {
                write!(
                    f,
                    "canonical CVA changed while reconciliation was being prepared"
                )
            }
            Self::PromotionFinalizationFailed(failure) => {
                write!(
                    f,
                    "promoted CVA failed finalization and the canonical copy was restored: {failure}"
                )
            }
            Self::PromotionRecoveryFailed { failure, recovery } => {
                write!(
                    f,
                    "promoted CVA failed finalization ({failure}) and canonical recovery also failed ({recovery})"
                )
            }
        }
    }
}

impl std::error::Error for CvaReconcileError {}

impl CvaReconcileError {
    pub fn conflict(&self) -> Option<&CvaReconcileConflict> {
        match self {
            Self::Conflict(conflict) => Some(conflict),
            _ => None,
        }
    }
}

impl From<CvaError> for CvaReconcileError {
    fn from(value: CvaError) -> Self {
        Self::Cva(value)
    }
}

impl From<ContainerError> for CvaReconcileError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}

impl From<ArchiveError> for CvaReconcileError {
    fn from(value: ArchiveError) -> Self {
        Self::Archive(value)
    }
}

impl From<MemoryError> for CvaReconcileError {
    fn from(value: MemoryError) -> Self {
        Self::Memories(value)
    }
}

impl From<EntityError> for CvaReconcileError {
    fn from(value: EntityError) -> Self {
        Self::Entities(value)
    }
}

impl From<RelationshipError> for CvaReconcileError {
    fn from(value: RelationshipError) -> Self {
        Self::Relationships(value)
    }
}
impl From<reliquary_perception_store::RelationshipError> for CvaReconcileError {
    fn from(value: reliquary_perception_store::RelationshipError) -> Self {
        Self::Relationships(value.into())
    }
}

impl From<GraphError> for CvaReconcileError {
    fn from(value: GraphError) -> Self {
        Self::Graph(value)
    }
}

impl From<InsomniaError> for CvaReconcileError {
    fn from(value: InsomniaError) -> Self {
        Self::Insomnia(value)
    }
}

impl From<CompatibilityProfileError> for CvaReconcileError {
    fn from(value: CompatibilityProfileError) -> Self {
        Self::CompatibilityProfile(value)
    }
}

impl From<io::Error> for CvaReconcileError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<reliquary_perception_store::ResolutionError> for CvaReconcileError {
    fn from(e: reliquary_perception_store::ResolutionError) -> Self {
        crate::entity_resolution_boundary::memory_error(e).into()
    }
}
