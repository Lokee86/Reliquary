use crate::{EntityId, MemoryId, MemoryTextField};

pub type MemoryEntityMentionKey =
    crate::memory_text_field_contract::MemoryEntityMentionKey<MemoryId>;

impl MemoryEntityMentionKey {
    pub fn new(memory_id: MemoryId, mention: &crate::MemoryEntityMention) -> Self {
        Self::from_parts(
            memory_id,
            mention.field,
            mention.start_byte,
            mention.end_byte,
        )
    }
}

pub const MAX_ENTITY_RESOLUTION_CANDIDATES: usize = 8;
pub const DEFAULT_ENTITY_RESOLUTION_PENDING_TTL_NS: i64 = 30 * 24 * 60 * 60 * 1_000_000_000;
pub const DEFAULT_ENTITY_RESOLUTION_DORMANT_TTL_NS: i64 = 180 * 24 * 60 * 60 * 1_000_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityResolutionReason {
    ContextMatch,
    ContextConflictNewIdentity,
    NamedReferent,
    PersistentArtifact,
    DescriptiveIdentity,
    FirstSeenIdentity,
    InsufficientEvidence,
    GenericRole,
    AbstractProcess,
    TransientValue,
    SentenceLocal,
    WrapperCategory,
    Ambiguous,
    RecurrenceRequired,
    PrincipalIdentity,
}

impl EntityResolutionReason {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::ContextMatch => 1,
            Self::ContextConflictNewIdentity => 2,
            Self::NamedReferent => 3,
            Self::PersistentArtifact => 4,
            Self::DescriptiveIdentity => 5,
            Self::FirstSeenIdentity => 6,
            Self::InsufficientEvidence => 7,
            Self::GenericRole => 8,
            Self::AbstractProcess => 9,
            Self::TransientValue => 10,
            Self::SentenceLocal => 11,
            Self::WrapperCategory => 12,
            Self::Ambiguous => 13,
            Self::RecurrenceRequired => 14,
            Self::PrincipalIdentity => 15,
        }
    }

    pub(crate) fn from_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            1 => Self::ContextMatch,
            2 => Self::ContextConflictNewIdentity,
            3 => Self::NamedReferent,
            4 => Self::PersistentArtifact,
            5 => Self::DescriptiveIdentity,
            6 => Self::FirstSeenIdentity,
            7 => Self::InsufficientEvidence,
            8 => Self::GenericRole,
            9 => Self::AbstractProcess,
            10 => Self::TransientValue,
            11 => Self::SentenceLocal,
            12 => Self::WrapperCategory,
            13 => Self::Ambiguous,
            14 => Self::RecurrenceRequired,
            15 => Self::PrincipalIdentity,
            _ => return None,
        })
    }

    pub(crate) fn is_rejection_class(self) -> bool {
        matches!(
            self,
            Self::GenericRole
                | Self::AbstractProcess
                | Self::TransientValue
                | Self::SentenceLocal
                | Self::WrapperCategory
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityResolutionPending {
    pub candidate_entity_ids: Vec<EntityId>,
    pub reason: EntityResolutionReason,
    pub candidate_set_fingerprint: [u8; 32],
    pub context_fingerprint: [u8; 32],
    pub first_seen_at_ns: i64,
    pub last_attempt_at_ns: i64,
    pub attempt_count: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityResolutionDormant {
    pub candidate_entity_ids: Vec<EntityId>,
    pub reason: EntityResolutionReason,
    pub candidate_set_fingerprint: [u8; 32],
    pub context_fingerprint: [u8; 32],
    pub first_seen_at_ns: i64,
    pub last_attempt_at_ns: i64,
    pub attempt_count: u16,
    pub dormant_at_ns: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryEntityResolutionStatus {
    Resolved {
        entity_id: EntityId,
        reason: EntityResolutionReason,
    },
    Rejected {
        reason: EntityResolutionReason,
    },
    Pending(EntityResolutionPending),
    Dormant(EntityResolutionDormant),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryEntityResolution {
    pub key: MemoryEntityMentionKey,
    pub revision: u32,
    pub updated_at_ns: i64,
    pub status: MemoryEntityResolutionStatus,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EntityResolutionCompaction {
    pub dormant: usize,
    pub purged: usize,
    pub rejected_purged: usize,
}
