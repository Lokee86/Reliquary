use crate::transfer::decode_hex;
use reliquary_perception_store::{Entity, EntityId, EntityTombstone, RetainedEntity};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(
    tag = "family",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(super) enum Publication {
    Revision(Revision),
    Terminal(Terminal),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Revision {
    id: String,
    revision: u64,
    entity_version: u64,
    global_version: u64,
    transaction_time_ns: Option<i64>,
    global_id: Option<String>,
    canonical_name: String,
    aliases: Vec<String>,
    kind: String,
    summary: String,
    mutation_id: String,
    created_at_ns: i64,
    updated_at_ns: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Terminal {
    id: String,
    revision: u64,
    entity_version: u64,
    global_version: u64,
    transaction_time_ns: Option<i64>,
    replacement: Option<String>,
}

impl Publication {
    pub(super) fn into_native(self) -> Result<(u64, Option<i64>, RetainedEntity), &'static str> {
        Ok(match self {
            Self::Revision(value) => (
                value.global_version,
                value.transaction_time_ns,
                RetainedEntity::Revision {
                    entity_version: value.entity_version,
                    entity: Entity {
                        id: EntityId(decode_hex(&value.id)?),
                        revision: value.revision,
                        canonical_name: value.canonical_name,
                        aliases: value.aliases,
                        kind: value.kind,
                        summary: value.summary,
                        mutation_id: value.mutation_id,
                        created_at_ns: value.created_at_ns,
                        updated_at_ns: value.updated_at_ns,
                    },
                    global_id: value.global_id.as_deref().map(decode_hex).transpose()?,
                },
            ),
            Self::Terminal(value) => (
                value.global_version,
                value.transaction_time_ns,
                RetainedEntity::Terminal {
                    terminal: EntityTombstone {
                        id: EntityId(decode_hex(&value.id)?),
                        revision: value.revision,
                        perception_version: value.entity_version,
                    },
                    replacement: value
                        .replacement
                        .as_deref()
                        .map(decode_hex)
                        .transpose()?
                        .map(EntityId),
                },
            ),
        })
    }
}
