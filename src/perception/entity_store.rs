#[path = "entity_store_read.rs"]
mod read;
#[path = "entity_store_validation.rs"]
mod validation;

use crate::entity_codec::{
    EntityTombstone, EntityVersion, encode_format, encode_record, encode_tombstone, encode_version,
};
use crate::entity_model::EntityRecord;
use crate::{Container, Entity, EntityDraft, EntityError, EntityGlobalId, EntityId};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use validation::{normalize_draft, validate_record};

pub(crate) struct EntityStore {
    records: Vec<EntityRecord>,
    current: HashMap<EntityId, usize>,
    by_mutation: HashMap<String, usize>,
    by_surface: HashMap<String, BTreeSet<EntityId>>,
    by_normalized_surface: HashMap<String, BTreeSet<EntityId>>,
    by_alias_surface: HashMap<String, BTreeSet<EntityId>>,
    revisions: HashMap<EntityId, u64>,
    retired: HashMap<EntityId, EntityId>,
    next_entity_version: u64,
}

impl EntityStore {
    pub(crate) fn empty() -> Self {
        Self {
            records: Vec::new(),
            current: HashMap::new(),
            by_mutation: HashMap::new(),
            by_surface: HashMap::new(),
            by_normalized_surface: HashMap::new(),
            by_alias_surface: HashMap::new(),
            revisions: HashMap::new(),
            retired: HashMap::new(),
            next_entity_version: 1,
        }
    }

    pub(crate) fn initialize(&self, container: &mut Container) -> Result<(), EntityError> {
        container.append(&encode_format())?;
        Ok(())
    }

    pub(crate) fn publish(
        &mut self,
        container: &mut Container,
        id: Option<EntityId>,
        expected_revision: u64,
        draft: EntityDraft,
    ) -> Result<(Entity, bool), EntityError> {
        self.publish_with_global_id(container, id, expected_revision, draft, None)
    }

    pub(crate) fn publish_with_global_id(
        &mut self,
        container: &mut Container,
        id: Option<EntityId>,
        expected_revision: u64,
        mut draft: EntityDraft,
        requested_global_id: Option<EntityGlobalId>,
    ) -> Result<(Entity, bool), EntityError> {
        normalize_draft(&mut draft)?;
        let id = if draft.kind == crate::entity_principal::PRINCIPAL_ENTITY_KIND {
            let principal_id = crate::entity_principal::principal_entity_id(&draft.canonical_name);
            if id.is_some_and(|id| id != principal_id) {
                return Err(EntityError::InvalidField("principal Entity ID"));
            }
            principal_id
        } else {
            id.unwrap_or_else(|| entity_id(&draft.mutation_id))
        };
        if let Some(index) = self.by_mutation.get(&draft.mutation_id).copied() {
            let existing = resolve(&self.records[index]);
            return if existing.id == id
                && self.current.contains_key(&existing.id)
                && same_draft(&existing, &draft)
            {
                Ok((existing, false))
            } else {
                Err(EntityError::MutationConflict)
            };
        }
        let current = self.current.get(&id).map(|index| &self.records[*index]);
        let current_revision = self.revisions.get(&id).copied().unwrap_or(0);
        if current_revision != expected_revision {
            return Err(EntityError::RevisionConflict);
        }
        if current.is_some_and(|record| record.created_at_ns != draft.created_at_ns) {
            return Err(EntityError::InvalidField("Entity created_at"));
        }
        let global_id = match current.and_then(|record| record.global_id) {
            Some(existing) => {
                if requested_global_id.is_some_and(|requested| requested != existing) {
                    return Err(EntityError::InvalidField("Entity global UUID"));
                }
                existing
            }
            None => requested_global_id.unwrap_or_else(|| {
                if draft.kind == crate::entity_principal::PRINCIPAL_ENTITY_KIND {
                    crate::entity_principal::principal_entity_global_id(&draft.canonical_name)
                } else {
                    EntityGlobalId(*uuid::Uuid::new_v4().as_bytes())
                }
            }),
        };

        let revision = expected_revision
            .checked_add(1)
            .ok_or(EntityError::VersionExhausted)?;
        let entity_version = self.next_entity_version;
        entity_version
            .checked_add(1)
            .ok_or(EntityError::VersionExhausted)?;

        let mut record = EntityRecord {
            id,
            global_id: Some(global_id),
            revision,
            canonical_name: draft.canonical_name,
            aliases: draft.aliases,
            kind: draft.kind,
            summary: draft.summary,
            mutation_id: draft.mutation_id,
            created_at_ns: draft.created_at_ns,
            updated_at_ns: draft.updated_at_ns,
            global_version: 0,
            entity_version,
        };
        let record_ref = container.append(&encode_record(&record)?)?;
        let global_version = container.allocate_version()?;
        container.append(&encode_version(EntityVersion {
            global_version,
            entity_version,
            record: record_ref,
        }))?;
        record.global_version = global_version;
        self.insert_rebuilt(record.clone())?;
        Ok((resolve(&record), true))
    }

    pub(crate) fn insert_rebuilt(&mut self, record: EntityRecord) -> Result<(), EntityError> {
        if record.revision == 0
            || record.entity_version == 0
            || record.global_version == 0
            || record.entity_version != self.next_entity_version
        {
            return Err(EntityError::InvalidVersion);
        }
        if let Some(index) = self.by_mutation.get(&record.mutation_id).copied() {
            return if self.records[index] == record {
                Ok(())
            } else {
                Err(EntityError::MutationConflict)
            };
        }
        let current = self
            .current
            .get(&record.id)
            .map(|index| &self.records[*index]);
        let expected_revision = self
            .revisions
            .get(&record.id)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(EntityError::VersionExhausted)?;
        if record.revision != expected_revision
            || current.is_some_and(|prior| prior.created_at_ns != record.created_at_ns)
        {
            return Err(EntityError::RevisionConflict);
        }
        validate_record(&record)?;

        if let Some(index) = self.current.get(&record.id).copied() {
            let prior = &self.records[index];
            for surface in entity_surfaces(&prior.canonical_name, &prior.aliases) {
                remove_surface(&mut self.by_surface, &surface, record.id);
            }
            for surface in normalized_entity_surfaces(&prior.canonical_name, &prior.aliases) {
                remove_surface(&mut self.by_normalized_surface, &surface, record.id);
            }
            for surface in alias_entity_surfaces(&prior.canonical_name, &prior.aliases) {
                remove_surface(&mut self.by_alias_surface, &surface, record.id);
            }
        }

        let index = self.records.len();
        self.by_mutation.insert(record.mutation_id.clone(), index);
        self.current.insert(record.id, index);
        self.revisions.insert(record.id, record.revision);
        self.retired.remove(&record.id);
        if record.kind != "principal" {
            for surface in entity_surfaces(&record.canonical_name, &record.aliases) {
                self.by_surface
                    .entry(surface)
                    .or_default()
                    .insert(record.id);
            }
            for surface in normalized_entity_surfaces(&record.canonical_name, &record.aliases) {
                self.by_normalized_surface
                    .entry(surface)
                    .or_default()
                    .insert(record.id);
            }
            for surface in alias_entity_surfaces(&record.canonical_name, &record.aliases) {
                self.by_alias_surface
                    .entry(surface)
                    .or_default()
                    .insert(record.id);
            }
        }
        self.records.push(record);
        self.next_entity_version = self
            .next_entity_version
            .checked_add(1)
            .ok_or(EntityError::VersionExhausted)?;
        Ok(())
    }

    pub(crate) fn tombstone(
        &mut self,
        container: &mut Container,
        id: EntityId,
        expected_revision: u64,
        replacement_id: EntityId,
    ) -> Result<bool, EntityError> {
        if id == replacement_id || !self.current.contains_key(&replacement_id) {
            return Err(EntityError::InvalidField("Entity replacement"));
        }
        let Some(current_index) = self.current.get(&id).copied() else {
            return if self.retired.get(&id).copied() == Some(replacement_id) {
                Ok(false)
            } else {
                Err(EntityError::MissingEntity)
            };
        };
        let current = &self.records[current_index];
        if current.revision != expected_revision {
            return Err(EntityError::RevisionConflict);
        }
        let revision = expected_revision
            .checked_add(1)
            .ok_or(EntityError::VersionExhausted)?;
        let entity_version = self.next_entity_version;
        entity_version
            .checked_add(1)
            .ok_or(EntityError::VersionExhausted)?;
        let mutation = container.append(&encode_tombstone(EntityTombstone {
            id,
            revision,
            replacement_id,
        }))?;
        let global_version = container.allocate_version()?;
        container.append(&encode_version(EntityVersion {
            global_version,
            entity_version,
            record: mutation,
        }))?;
        self.insert_tombstone_rebuilt(
            EntityTombstone {
                id,
                revision,
                replacement_id,
            },
            global_version,
            entity_version,
        )?;
        Ok(true)
    }

    pub(crate) fn insert_tombstone_rebuilt(
        &mut self,
        tombstone: EntityTombstone,
        global_version: u64,
        entity_version: u64,
    ) -> Result<(), EntityError> {
        if global_version == 0
            || entity_version == 0
            || entity_version != self.next_entity_version
            || tombstone.id == tombstone.replacement_id
            || !self.current.contains_key(&tombstone.replacement_id)
        {
            return Err(EntityError::InvalidVersion);
        }
        let expected_revision = self
            .revisions
            .get(&tombstone.id)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(EntityError::VersionExhausted)?;
        if tombstone.revision != expected_revision {
            return Err(EntityError::RevisionConflict);
        }
        let index = self
            .current
            .remove(&tombstone.id)
            .ok_or(EntityError::MissingEntity)?;
        let prior = &self.records[index];
        deindex_entity(
            &mut self.by_surface,
            &mut self.by_normalized_surface,
            &mut self.by_alias_surface,
            prior,
        );
        self.revisions.insert(tombstone.id, tombstone.revision);
        self.retired.insert(tombstone.id, tombstone.replacement_id);
        self.next_entity_version = self
            .next_entity_version
            .checked_add(1)
            .ok_or(EntityError::VersionExhausted)?;
        Ok(())
    }

    pub(crate) fn retired_replacement(&self, id: EntityId) -> Option<EntityId> {
        self.retired.get(&id).copied()
    }

    pub(crate) fn canonical_active_id(&self, id: EntityId) -> Option<EntityId> {
        if self.current.contains_key(&id) {
            return Some(id);
        }

        let mut current = id;
        for _ in 0..=self.retired.len() {
            current = self.retired.get(&current).copied()?;
            if self.current.contains_key(&current) {
                return Some(current);
            }
        }
        None
    }
}

fn entity_id(mutation_id: &str) -> EntityId {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-entity-id\0");
    hash.update((mutation_id.len() as u64).to_le_bytes());
    hash.update(mutation_id.as_bytes());
    EntityId(hash.finalize().into())
}

fn resolve(record: &EntityRecord) -> Entity {
    Entity {
        id: record.id,
        global_id: record.global_id,
        revision: record.revision,
        canonical_name: record.canonical_name.clone(),
        aliases: record.aliases.clone(),
        kind: record.kind.clone(),
        summary: record.summary.clone(),
        mutation_id: record.mutation_id.clone(),
        created_at_ns: record.created_at_ns,
        updated_at_ns: record.updated_at_ns,
        global_version: record.global_version,
        entity_version: record.entity_version,
    }
}

fn normalize_surface(value: &str) -> String {
    value.trim().to_lowercase()
}

fn normalized_surface(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|value| value.is_alphanumeric())
        .collect()
}

fn entity_surfaces(canonical_name: &str, aliases: &[String]) -> BTreeSet<String> {
    std::iter::once(canonical_name)
        .chain(aliases.iter().map(String::as_str))
        .map(normalize_surface)
        .collect()
}

fn normalized_entity_surfaces(canonical_name: &str, aliases: &[String]) -> BTreeSet<String> {
    std::iter::once(canonical_name)
        .chain(aliases.iter().map(String::as_str))
        .map(normalized_surface)
        .filter(|surface| !surface.is_empty())
        .collect()
}

fn alias_entity_surfaces(canonical_name: &str, aliases: &[String]) -> BTreeSet<String> {
    std::iter::once(canonical_name)
        .chain(aliases.iter().map(String::as_str))
        .flat_map(alias_surface_keys)
        .collect()
}

fn alias_surface_keys(value: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    if let Some(repository) = repository_alias_key(value) {
        keys.insert(format!("repository:{repository}"));
    }
    if let Some(acronym) = acronym_alias_key(value) {
        keys.insert(format!("acronym:{acronym}"));
    }
    keys
}

fn repository_alias_key(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    let repo = if lower.starts_with("http://") || lower.starts_with("https://") {
        let without_scheme = trimmed.split_once("://")?.1;
        let mut segments = without_scheme
            .split('/')
            .filter(|segment| !segment.is_empty());
        let _host = segments.next()?;
        segments.last()?.trim_end_matches(".git").to_owned()
    } else if lower.ends_with(" repository") {
        trimmed[..trimmed.len() - " repository".len()]
            .trim()
            .trim_start_matches('@')
            .to_owned()
    } else if lower.ends_with(" repo") {
        trimmed[..trimmed.len() - " repo".len()]
            .trim()
            .trim_start_matches('@')
            .to_owned()
    } else {
        return None;
    };
    let key = normalized_surface(&repo);
    (!key.is_empty()).then_some(key)
}

fn acronym_alias_key(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
        return None;
    }

    let plural_base = trimmed.strip_suffix('s').unwrap_or(trimmed);
    if plural_base.len() >= 2
        && plural_base
            .chars()
            .all(|value| value.is_ascii_uppercase() || value.is_ascii_digit())
    {
        return Some(plural_base.to_ascii_lowercase());
    }

    let prefix: String = trimmed
        .chars()
        .take_while(|value| value.is_ascii_uppercase() || value.is_ascii_digit())
        .collect();
    if (2..=4).contains(&prefix.len())
        && prefix.len() < trimmed.len()
        && trimmed[prefix.len()..]
            .chars()
            .next()
            .is_some_and(|value| value.is_ascii_lowercase())
    {
        return Some(prefix.to_ascii_lowercase());
    }

    None
}

fn deindex_entity(
    by_surface: &mut HashMap<String, BTreeSet<EntityId>>,
    by_normalized_surface: &mut HashMap<String, BTreeSet<EntityId>>,
    by_alias_surface: &mut HashMap<String, BTreeSet<EntityId>>,
    record: &EntityRecord,
) {
    for surface in entity_surfaces(&record.canonical_name, &record.aliases) {
        remove_surface(by_surface, &surface, record.id);
    }
    for surface in normalized_entity_surfaces(&record.canonical_name, &record.aliases) {
        remove_surface(by_normalized_surface, &surface, record.id);
    }
    for surface in alias_entity_surfaces(&record.canonical_name, &record.aliases) {
        remove_surface(by_alias_surface, &surface, record.id);
    }
}

fn remove_surface(
    index: &mut HashMap<String, BTreeSet<EntityId>>,
    surface: &str,
    entity_id: EntityId,
) {
    if let Some(ids) = index.get_mut(surface) {
        ids.remove(&entity_id);
        if ids.is_empty() {
            index.remove(surface);
        }
    }
}

fn same_draft(entity: &Entity, draft: &EntityDraft) -> bool {
    entity.canonical_name == draft.canonical_name
        && entity.aliases == draft.aliases
        && entity.kind == draft.kind
        && entity.summary == draft.summary
        && entity.mutation_id == draft.mutation_id
        && entity.created_at_ns == draft.created_at_ns
        && entity.updated_at_ns == draft.updated_at_ns
}
