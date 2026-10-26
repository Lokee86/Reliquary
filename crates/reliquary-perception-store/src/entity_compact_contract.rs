//! Entity-owned retained-record contract; wire framing and publication stay separate.
use crate::entity_model::EntityRecord;
use crate::entity_store::validate_record;
use crate::{EntityError, EntityGlobalId, EntityId};
use reliquary_container::codec::FieldChange;

/// Opaque authenticated maintenance evidence, not an authored description.
/// File-level replay must still verify the exact manifest authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityDescriptionProof {
    /// SHA-256 of the original record carrier bytes, before description pruning.
    pub original_carrier_digest: [u8; 32],
    /// SHA-256 of the complete canonical encoded CVAHPR01 manifest.
    /// File-level validation checks its descriptor authorizes this revision.
    pub manifest_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityCreation {
    pub record: EntityRecord,
    pub description_proof: Option<EntityDescriptionProof>,
}

impl EntityCreation {
    pub fn new(record: EntityRecord) -> Result<Self, EntityError> {
        if record.revision != 1 || record.global_version == 0 || record.entity_version == 0 {
            return Err(EntityError::InvalidVersion);
        }
        validate_record(&record)?;
        Ok(Self {
            record,
            description_proof: None,
        })
    }

    pub fn new_pruned(
        record: EntityRecord,
        proof: EntityDescriptionProof,
    ) -> Result<Self, EntityError> {
        validate_pruned_description(&record)?;
        let mut creation = Self::new(record)?;
        creation.description_proof = Some(proof);
        Ok(creation)
    }
}

macro_rules! entity_fields {
    ($($bit:literal => $field:ident: $ty:ty),+ $(,)?) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct EntityDeltaFields { $(pub $field: FieldChange<$ty>,)+ }
        pub const ENTITY_DELTA_FIELD_ORDER: &[&str] = &[$(stringify!($field)),+];
        pub const ENTITY_DELTA_KNOWN_MASK: u64 = 0 $(| (1 << $bit))+;
        impl EntityDeltaFields {
            pub fn between(previous: &EntityRecord, next: &EntityRecord) -> Self {
                // A new record field must receive an explicit owner decision.
                let EntityRecord { id: _, revision: _, mutation_id: _, created_at_ns: _,
                    global_version: _, entity_version: _, $($field: _,)+ } = next;
                Self { $($field: FieldChange::between(&previous.$field, &next.$field),)+ }
            }
            pub fn mask(&self) -> u64 {
                0 $(| if matches!(self.$field, FieldChange::Unchanged) { 0 } else { 1 << $bit })+
            }
        }
    };
}

entity_fields! {
    0 => global_id: Option<EntityGlobalId>,
    1 => canonical_name: String,
    2 => aliases: Vec<String>,
    3 => kind: String,
    4 => summary: String,
    5 => updated_at_ns: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityDelta {
    pub id: EntityId,
    pub previous_revision: u64,
    pub revision: u64,
    pub global_version: u64,
    pub entity_version: u64,
    pub mutation_id: String,
    pub fields: EntityDeltaFields,
    pub description_proof: Option<EntityDescriptionProof>,
}

impl EntityDelta {
    pub fn between(
        previous: &EntityRecord,
        next: &EntityRecord,
    ) -> Result<Self, EntityError> {
        validate_record(previous)?;
        validate_record(next)?;
        if previous.id != next.id {
            return Err(EntityError::InvalidField("Entity delta identity"));
        }
        if previous.revision == 0
            || previous.global_version == 0
            || previous.entity_version == 0
            || next.global_version == 0
            || next.entity_version <= previous.entity_version
        {
            return Err(EntityError::InvalidVersion);
        }
        let revision = previous
            .revision
            .checked_add(1)
            .ok_or(EntityError::VersionExhausted)?;
        if next.revision != revision || next.created_at_ns != previous.created_at_ns {
            return Err(EntityError::RevisionConflict);
        }
        if next.mutation_id == previous.mutation_id {
            return Err(EntityError::MutationConflict);
        }
        Ok(Self {
            id: next.id,
            previous_revision: previous.revision,
            revision,
            global_version: next.global_version,
            entity_version: next.entity_version,
            mutation_id: next.mutation_id.clone(),
            fields: EntityDeltaFields::between(previous, next),
            description_proof: None,
        })
    }

    pub fn between_pruned(
        previous: &EntityRecord,
        next: &EntityRecord,
        proof: EntityDescriptionProof,
    ) -> Result<Self, EntityError> {
        validate_pruned_description(next)?;
        let mut delta = Self::between(previous, next)?;
        delta.description_proof = Some(proof);
        Ok(delta)
    }

    pub fn validate_mask(mask: u64) -> Result<(), EntityError> {
        if !reliquary_container::codec::mask_is_known(mask, ENTITY_DELTA_KNOWN_MASK) {
            return Err(EntityError::CorruptRecord("unknown Entity delta field"));
        }
        Ok(())
    }
}

fn validate_pruned_description(record: &EntityRecord) -> Result<(), EntityError> {
    if record.kind == crate::entity_identity::PRINCIPAL_ENTITY_KIND {
        return Err(EntityError::PayloadRequiredByLiveTarget);
    }
    if record.canonical_name != "[pruned]"
        || !record.aliases.is_empty()
        || !record.summary.is_empty()
    {
        return Err(EntityError::CorruptRecord(
            "noncanonical Entity pruned description",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "entity_compact_contract_tests.rs"]
mod tests;
