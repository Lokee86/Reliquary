use reliquary_memory_core::MemoryId;

pub const DREAM_CLASSIFIER_CONTRACT_VERSION: &str = "v3";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DreamRelationKind {
    None,
    Topical,
    Factual,
    Causal,
    Recurrent,
    DuplicateOf,
    Supersedes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DreamRelationDirection {
    None,
    Undirected,
    AToB,
    BToA,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DreamEvidenceSide {
    A,
    B,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DreamPairEvidence {
    pub side: DreamEvidenceSide,
    pub quote: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DreamPairClassification {
    pub model: String,
    pub a: MemoryId,
    pub b: MemoryId,
    pub relation: DreamRelationKind,
    pub direction: DreamRelationDirection,
    pub evidence: Vec<DreamPairEvidence>,
}
