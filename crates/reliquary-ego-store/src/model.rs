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

/// One accepted Ego publication at its owner-local version. Historical proofs
/// use this sequence rather than only the final projected values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EgoHistoryRecord {
    Identity {
        ego_version: u64,
        id: EgoIdentityId,
        revision: u64,
        deleted: bool,
        name: String,
        text: String,
    },
    Personality {
        ego_version: u64,
        revision: u64,
        source_memory_version: u64,
        text: String,
    },
    Anchor {
        ego_version: u64,
        id: EgoAnchorId,
        revision: u64,
        deleted: bool,
        priority: EgoAnchorPriority,
        text: String,
    },
    Synthesis {
        ego_version: u64,
        revision: u64,
        source_memory_version: u64,
        text: String,
    },
}

/// Ego-local lifecycle facts, mapped into the product lifecycle at the root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EgoLifecycleState {
    Active,
    Tombstoned { ego_version: Option<u64> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EgoHistorySnapshot {
    pub ego_version: u64,
    pub records: Vec<EgoHistoryRecord>,
    pub identities: Vec<EgoIdentity>,
    pub tombstoned_identities: Vec<EgoIdentityId>,
    pub personality: Option<EgoPersonality>,
    pub synthesis: Option<EgoWebSynthesis>,
    pub anchors: Vec<EgoAnchor>,
    pub tombstoned_anchors: Vec<EgoAnchorId>,
}

pub const MAX_EGO_ANCHOR_CHARS: usize = 2_500;
