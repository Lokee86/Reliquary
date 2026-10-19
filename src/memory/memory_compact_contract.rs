//! Owner-local creation/delta contract. No wire codec or writer cutover yet.
use crate::memory_model::MemoryRecord;
use crate::{EpisodeId, MemoryError, MemoryId, MemorySourceRef, MemoryTemporalInference};

/// For optional T, Set(None) explicitly clears; Unchanged inherits the prior
/// accepted value. This operates on resolved records, never caller arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MemoryFieldChange<T> {
    Unchanged,
    Set(T),
}
impl<T: Clone + Eq> MemoryFieldChange<T> {
    fn between(previous: &T, next: &T) -> Self {
        if previous == next {
            Self::Unchanged
        } else {
            Self::Set(next.clone())
        }
    }
}

macro_rules! memory_fields {
    ($($bit:literal => $field:ident: $ty:ty),+ $(,)?) => {
        /// Each bit has exactly one typed value, serialized in ascending order.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub(crate) struct MemoryDeltaFields {
            $(pub $field: MemoryFieldChange<$ty>,)+
        }
        pub(crate) const MEMORY_DELTA_FIELD_ORDER: &[&str] = &[$(stringify!($field)),+];
        pub(crate) const MEMORY_DELTA_KNOWN_MASK: u64 = 0 $(| (1 << $bit))+;
        impl MemoryDeltaFields {
            pub(crate) fn between(previous: &MemoryRecord, next: &MemoryRecord) -> Self {
                // Exhaustive pattern: a future record field cannot silently fall
                // outside this owner contract.
                let MemoryRecord { id: _, revision: _, body_id: _, mutation_id: _,
                    global_version: _, memory_version: _, $($field: _,)+ } = next;
                Self { $($field: MemoryFieldChange::between(&previous.$field, &next.$field),)+ }
            }
            pub(crate) fn mask(&self) -> u64 {
                0 $(| if matches!(self.$field, MemoryFieldChange::Unchanged) { 0 } else { 1 << $bit })+
            }
        }
    };
}

memory_fields! {
    0 => category: String,
    1 => memory_type: String,
    2 => authority_kind: String,
    3 => temporal_status: String,
    4 => scope: String,
    5 => lifecycle_state: String,
    6 => archived: bool,
    7 => superseded_by: Option<MemoryId>,
    8 => parent_id: Option<MemoryId>,
    9 => source_node_id: Option<String>,
    10 => content_source_conversation_id: Option<String>,
    11 => content_source_node_id: Option<String>,
    12 => grounding_source_conversation_id: Option<String>,
    13 => grounding_source_node_id: Option<String>,
    14 => source_episode_id: Option<EpisodeId>,
    15 => source_time_ns: Option<i64>,
    16 => source_ref: Option<MemorySourceRef>,
    17 => temporal_inference: Option<MemoryTemporalInference>,
    18 => created_at_ns: i64,
    19 => updated_at_ns: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MemoryCreation {
    pub record: MemoryRecord,
}
impl MemoryCreation {
    pub(crate) fn new(record: MemoryRecord) -> Result<Self, MemoryError> {
        if record.revision != 1 {
            return Err(MemoryError::RevisionConflict);
        }
        if record.global_version == 0 || record.memory_version == 0 {
            return Err(MemoryError::CorruptRecord(
                "zero Memory creation coordinate",
            ));
        }
        if record.mutation_id.is_empty() {
            return Err(MemoryError::InvalidField("mutation_id"));
        }
        Ok(Self { record })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MemoryDelta {
    pub id: MemoryId,
    pub previous_revision: u64,
    pub revision: u64,
    pub global_version: u64,
    pub memory_version: u64,
    pub mutation_id: String,
    pub fields: MemoryDeltaFields,
}
impl MemoryDelta {
    pub(crate) fn between(
        previous: &MemoryRecord,
        next: &MemoryRecord,
    ) -> Result<Self, MemoryError> {
        if previous.id != next.id || previous.body_id != next.body_id {
            return Err(MemoryError::SemanticMutation);
        }
        if previous.revision == 0 || previous.revision.checked_add(1) != Some(next.revision) {
            return Err(MemoryError::RevisionConflict);
        }
        if previous.global_version == 0
            || previous.memory_version == 0
            || next.global_version <= previous.global_version
            || next.memory_version <= previous.memory_version
        {
            return Err(MemoryError::CorruptRecord(
                "invalid Memory delta coordinates",
            ));
        }
        if next.mutation_id.is_empty() || next.mutation_id == previous.mutation_id {
            return Err(MemoryError::MutationConflict);
        }
        Ok(Self {
            id: next.id,
            previous_revision: previous.revision,
            revision: next.revision,
            global_version: next.global_version,
            memory_version: next.memory_version,
            mutation_id: next.mutation_id.clone(),
            fields: MemoryDeltaFields::between(previous, next),
        })
    }

    pub(crate) fn validate_mask(mask: u64) -> Result<(), MemoryError> {
        if mask & !MEMORY_DELTA_KNOWN_MASK != 0 {
            return Err(MemoryError::CorruptRecord("unknown Memory delta field"));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "memory_compact_contract_tests.rs"]
mod tests;
