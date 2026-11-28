use crate::EgoIdentityId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoSourceCut {
    pub owner_id: [u8; 16],
    pub history_generation: [u8; 32],
    pub memory_version: u64,
    pub audit_frontier: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoCompletion {
    pub id: [u8; 32],
    pub cut: EgoSourceCut,
    pub batch_ids: Vec<[u8; 32]>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum EgoTextField {
    Title,
    Content,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct EgoCitation {
    pub owner_id: [u8; 16],
    pub history_generation: [u8; 32],
    pub memory_id: [u8; 32],
    pub revision: u64,
    pub body_id: [u8; 32],
    pub field: EgoTextField,
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoIdentitySelection {
    pub revision: u64,
    pub id: Option<EgoIdentityId>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct EgoTopicId(pub [u8; 16]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct EgoPointId(pub [u8; 16]);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoEmbedding {
    pub profile_id: [u8; 32],
    /// Exact IEEE-754 bits for each finite component of the compatible vector.
    pub components: Vec<u32>,
}

impl EgoEmbedding {
    pub fn from_components(profile_id: [u8; 32], components: &[f32]) -> Self {
        Self {
            profile_id,
            components: components
                .iter()
                .map(|component| component.to_bits())
                .collect(),
        }
    }

    pub fn components(&self) -> impl Iterator<Item = f32> + '_ {
        self.components.iter().copied().map(f32::from_bits)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EgoPointStatus {
    Observed,
    Planned,
    Requested,
    Possible,
    Expected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EgoTimeScope {
    Past,
    Current,
    Future,
    Unspecified,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoTopic {
    pub title: String,
    pub embedding: Option<EgoEmbedding>,
    /// Exact evidence revisions already processed for this topic. They may
    /// produce zero verified points and still count toward topic coverage.
    #[serde(default)]
    pub references: Vec<EgoCitation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoPoint {
    pub topic_id: EgoTopicId,
    pub statement: String,
    pub status: EgoPointStatus,
    pub time_scope: EgoTimeScope,
    pub verified_at_ns: i64,
    pub citations: Vec<EgoCitation>,
    pub embedding: Option<EgoEmbedding>,
}

/// The item keeps its last verified value even after withdrawal. Its revision
/// advances with every change, including withdrawal and lifting suppression.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoSummaryItem<T> {
    pub revision: u64,
    pub value_revision: u64,
    pub value: T,
    pub tombstone: Option<EgoTombstone>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoTombstone {
    pub suppressed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EgoItemChange<T> {
    Put(T),
    Tombstone { suppress: bool },
    LiftSuppression,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EgoRelChange {
    Topic {
        id: EgoTopicId,
        expected_revision: u64,
        change: EgoItemChange<EgoTopic>,
    },
    Point {
        id: EgoPointId,
        expected_revision: u64,
        change: EgoItemChange<EgoPoint>,
    },
}

/// A refresh carries its completion. A direct user change omits it and keeps
/// the most recently accepted audit cut.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoRelPublication {
    pub source_owner_id: [u8; 16],
    pub expected_revision: u64,
    pub completion: Option<EgoCompletion>,
    pub changes: Vec<EgoRelChange>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoRelSummary {
    pub revision: u64,
    pub completion: Option<EgoCompletion>,
    pub topics: BTreeMap<EgoTopicId, EgoSummaryItem<EgoTopic>>,
    pub points: BTreeMap<EgoPointId, EgoSummaryItem<EgoPoint>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoParagraph {
    pub start_byte: u32,
    pub end_byte: u32,
    pub citations: Vec<EgoCitation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoPhyPublication {
    pub expected_revision: u64,
    pub completion: EgoCompletion,
    /// No replacement is needed when the accepted refresh is empty. An
    /// explicitly empty verified replacement can clear unsupported old text.
    pub replacement: Option<EgoPhyText>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoPhyText {
    pub text: String,
    pub paragraphs: Vec<EgoParagraph>,
    pub token_count: u32,
}

pub const MAX_EGO_PHY_SUMMARY_TOKENS: u32 = 4_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EgoPhySummary {
    pub revision: u64,
    pub completion: EgoCompletion,
    pub text: Option<EgoPhyText>,
}
