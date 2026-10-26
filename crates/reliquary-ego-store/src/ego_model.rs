#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EgoIdentityId(pub [u8; 16]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EgoIdentity {
    pub id: EgoIdentityId,
    pub revision: u64,
    pub name: String,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EgoPersonality {
    pub revision: u64,
    pub text: String,
    /// PHY Memory watermark from which this behavioral projection was synthesized.
    pub source_memory_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EgoAnchorId(pub [u8; 16]);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EgoAnchorPriority {
    High,
    #[default]
    Normal,
    Low,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EgoAnchor {
    pub id: EgoAnchorId,
    pub revision: u64,
    pub priority: EgoAnchorPriority,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EgoWebSynthesis {
    pub revision: u64,
    pub source_memory_version: u64,
    pub text: String,
}

pub const MAX_EGO_ANCHOR_CHARS: usize = 2_500;
