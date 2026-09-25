use crate::entity_model::EntityRecord;
use crate::{
    EntityDraft, EntityError, MAX_ENTITY_ALIASES, MAX_ENTITY_KIND_BYTES, MAX_ENTITY_NAME_BYTES,
    MAX_ENTITY_SUMMARY_BYTES,
};

pub(super) fn normalize_draft(draft: &mut EntityDraft) -> Result<(), EntityError> {
    draft.canonical_name = draft.canonical_name.trim().to_owned();
    draft.kind = draft.kind.trim().to_owned();
    draft.summary = draft.summary.trim().to_owned();
    draft.mutation_id = draft.mutation_id.trim().to_owned();
    draft.aliases = draft
        .aliases
        .drain(..)
        .map(|alias| alias.trim().to_owned())
        .filter(|alias| !alias.is_empty())
        .collect();
    draft.aliases.sort();
    draft.aliases.dedup();
    validate_fields(
        &draft.canonical_name,
        &draft.aliases,
        &draft.kind,
        &draft.summary,
        &draft.mutation_id,
        draft.created_at_ns,
        draft.updated_at_ns,
    )
}

pub(super) fn validate_record(record: &EntityRecord) -> Result<(), EntityError> {
    if record.kind == crate::entity_principal::PRINCIPAL_ENTITY_KIND {
        if record.id != crate::entity_principal::principal_entity_id(&record.canonical_name) {
            return Err(EntityError::InvalidField("principal Entity ID"));
        }
        if record.global_id.is_some_and(|global_id| {
            global_id != crate::entity_principal::principal_entity_global_id(&record.canonical_name)
        }) {
            return Err(EntityError::InvalidField("principal Entity global UUID"));
        }
    }
    validate_fields(
        &record.canonical_name,
        &record.aliases,
        &record.kind,
        &record.summary,
        &record.mutation_id,
        record.created_at_ns,
        record.updated_at_ns,
    )
}

fn validate_fields(
    canonical_name: &str,
    aliases: &[String],
    kind: &str,
    summary: &str,
    mutation_id: &str,
    created_at_ns: i64,
    updated_at_ns: i64,
) -> Result<(), EntityError> {
    if invalid_text(canonical_name, MAX_ENTITY_NAME_BYTES, true) {
        return Err(EntityError::InvalidField("Entity canonical name"));
    }
    if invalid_text(kind, MAX_ENTITY_KIND_BYTES, true) {
        return Err(EntityError::InvalidField("Entity kind"));
    }
    if kind == crate::entity_principal::PRINCIPAL_ENTITY_KIND
        && !crate::entity_principal::is_phy_principal_id(canonical_name)
    {
        return Err(EntityError::InvalidField("principal Entity identity"));
    }
    if invalid_text(summary, MAX_ENTITY_SUMMARY_BYTES, false) {
        return Err(EntityError::InvalidField("Entity summary"));
    }
    if aliases.len() > MAX_ENTITY_ALIASES
        || aliases.windows(2).any(|pair| pair[0] >= pair[1])
        || aliases
            .iter()
            .any(|alias| invalid_text(alias, MAX_ENTITY_NAME_BYTES, true))
    {
        return Err(EntityError::InvalidField("Entity aliases"));
    }
    if mutation_id.is_empty() || mutation_id.chars().any(char::is_control) {
        return Err(EntityError::InvalidField("Entity mutation ID"));
    }
    if updated_at_ns < created_at_ns {
        return Err(EntityError::InvalidField("Entity timestamps"));
    }
    Ok(())
}

fn invalid_text(value: &str, max_bytes: usize, require_nonempty: bool) -> bool {
    (require_nonempty && value.is_empty())
        || value.len() > max_bytes
        || value.chars().any(char::is_control)
}
