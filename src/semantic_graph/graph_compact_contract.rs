//! Graph-owned wire coordinates; GraphStore resolves stable identities at publication.
use crate::{GraphRelationOrigin, SemanticGraphRelationKind};

/// Zero is a valid ordinal; dense mapping allocation and checked exhaustion
/// stay owned by GraphStore. Every mutation retains its individual origin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GraphOrdinalMutation {
    pub source: u32,
    pub target: u32,
    pub kind: SemanticGraphRelationKind,
    pub active: bool,
    pub origin: GraphRelationOrigin,
}
