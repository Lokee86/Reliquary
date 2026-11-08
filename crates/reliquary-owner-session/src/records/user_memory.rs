//! One recoverable user Memory mutation with exact ordered store effects.
use reliquary_graph::{NodeRef, Relation, RelationKind, RelationOrigin};
use reliquary_memory_core::{
    GroupedManagementEffect, GroupedMemorySupersession, MemoryBodyId, MemoryId, MemoryRecord,
    decode_grouped_creation, history_change_in_frame, memory_id,
};
use serde::{Deserialize, Serialize};

pub(crate) const CORRECTION: &[u8; 8] = b"RELMCOR1";
pub(crate) const RETIREMENT: &[u8; 8] = b"RELMRET1";
const MAX_GROUP: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MemoryEvidence {
    pub id: [u8; 32],
    pub revision: u64,
    pub body: [u8; 32],
}

impl MemoryEvidence {
    pub(crate) fn matches(self, record: &MemoryRecord) -> bool {
        self.id == record.id.0 && self.revision == record.revision && self.body == record.body_id.0
    }

    pub(crate) fn new(id: MemoryId, revision: u64, body: MemoryBodyId) -> Self {
        Self {
            id: id.0,
            revision,
            body: body.0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UserMemoryRecord {
    pub owner: [u8; 16],
    pub principal: String,
    pub key: String,
    pub request_hash: [u8; 32],
    pub memory_cut: u64,
    pub previous: MemoryEvidence,
    pub effect: UserMemoryEffect,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) enum UserMemoryEffect {
    Correction {
        graph_cut: u64,
        creation: Vec<u8>,
        delta: Vec<u8>,
    },
    Retirement {
        successor: MemoryEvidence,
        tombstone: Vec<u8>,
    },
}

impl UserMemoryRecord {
    pub(crate) fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut bytes = match self.effect {
            UserMemoryEffect::Correction { .. } => CORRECTION.to_vec(),
            UserMemoryEffect::Retirement { .. } => RETIREMENT.to_vec(),
        };
        bytes.extend(serde_json::to_vec(self).map_err(|error| error.to_string())?);
        if bytes.len() > MAX_GROUP {
            return Err("user Memory record exceeds limit".into());
        }
        Ok(bytes)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Option<Self>, String> {
        if !bytes.starts_with(b"RELMCOR") && !bytes.starts_with(b"RELMRET") {
            return Ok(None);
        }
        if bytes.len() > MAX_GROUP
            || !(bytes.starts_with(CORRECTION) || bytes.starts_with(RETIREMENT))
        {
            return Err("invalid user Memory record header".into());
        }
        let record: Self =
            serde_json::from_slice(&bytes[8..]).map_err(|error| error.to_string())?;
        if record.encode()? != bytes {
            return Err("noncanonical user Memory record".into());
        }
        Ok(Some(record))
    }

    fn validate(&self) -> Result<(), String> {
        let next = self
            .memory_cut
            .checked_add(1)
            .ok_or("exhausted Memory cut")?;
        if self.key.len() != "management:".len() + 64
            || !self.key.starts_with("management:")
            || !self.key["management:".len()..]
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || self.principal.trim().is_empty()
            || self.principal.len() > 4096
            || self.principal.chars().any(char::is_control)
            || self.previous.revision == 0
        {
            return Err("invalid user Memory authority".into());
        }
        match &self.effect {
            UserMemoryEffect::Correction {
                graph_cut,
                creation,
                delta,
            } => {
                let record = decode_grouped_creation(creation).map_err(|e| e.to_string())?;
                let source = record
                    .source_ref
                    .as_ref()
                    .ok_or("missing correction provenance")?;
                let owner: String = self
                    .owner
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
                if *graph_cut == u64::MAX
                    || record.id != memory_id(&self.key)
                    || record.id.0 == self.previous.id
                    || record.mutation_id != self.key
                    || record.memory_version != next
                    || record.authority_kind != "direct"
                    || record.source_episode_id.is_some()
                    || source.owner_id != owner
                    || source.principal_id.as_deref() != Some(self.principal.as_str())
                    || source.source_episode_id != [0; 32]
                    || source.source_node_id != format!("management:{}", self.key)
                    || record.source_node_id.as_deref() != Some(source.source_node_id.as_str())
                    || record.parent_id.is_some()
                    || record.content_source_conversation_id.is_some()
                    || record.content_source_node_id.is_some()
                    || record.grounding_source_conversation_id.is_some()
                    || record.grounding_source_node_id.is_some()
                    || source.content_source_conversation_id.is_some()
                    || source.content_source_node_id.is_some()
                    || source.grounding_source_conversation_id.is_some()
                    || source.grounding_source_node_id.is_some()
                    || !delta.starts_with(b"RELMDLT1")
                    || history_change_in_frame(delta).map_err(|e| e.to_string())?
                        != Some((
                            MemoryId(self.previous.id),
                            next.checked_add(1).ok_or("exhausted Memory cut")?,
                        ))
                {
                    return Err("invalid correction effect or provenance".into());
                }
            }
            UserMemoryEffect::Retirement {
                successor,
                tombstone,
            } => {
                if successor.id == self.previous.id
                    || successor.revision == 0
                    || !tombstone.starts_with(b"RELMTMB1")
                    || history_change_in_frame(tombstone).map_err(|e| e.to_string())?
                        != Some((MemoryId(self.previous.id), next))
                    || tombstone.get(40..48)
                        != self
                            .previous
                            .revision
                            .checked_add(1)
                            .as_ref()
                            .map(|revision| revision.to_le_bytes())
                            .as_ref()
                            .map(|bytes| bytes.as_slice())
                {
                    return Err("invalid retirement effect".into());
                }
            }
        }
        Ok(())
    }

    pub(crate) fn memory_effect(&self) -> Result<GroupedManagementEffect, String> {
        match &self.effect {
            UserMemoryEffect::Correction {
                creation, delta, ..
            } => {
                let record = decode_grouped_creation(creation).map_err(|e| e.to_string())?;
                Ok(GroupedManagementEffect::Correction {
                    creation: creation.clone(),
                    supersession: GroupedMemorySupersession {
                        older: MemoryId(self.previous.id),
                        newer: record.id,
                        previous_revision: self.previous.revision,
                        updated_at_ns: record.updated_at_ns,
                        mutation_id: format!("{}:supersession", self.key),
                        delta: delta.clone(),
                    },
                })
            }
            UserMemoryEffect::Retirement { tombstone, .. } => {
                Ok(GroupedManagementEffect::Retirement(tombstone.clone()))
            }
        }
    }

    pub(crate) fn relation(&self) -> Option<Relation> {
        let UserMemoryEffect::Correction { graph_cut, .. } = self.effect else {
            return None;
        };
        Some(Relation {
            source: NodeRef::Memory(memory_id(&self.key).0),
            target: NodeRef::Memory(self.previous.id),
            kind: RelationKind::Supersedes,
            active: true,
            origin: RelationOrigin::User,
            graph_version: graph_cut + 1,
        })
    }

    pub(crate) fn history_frames(&self) -> Vec<&[u8]> {
        match &self.effect {
            UserMemoryEffect::Correction {
                creation, delta, ..
            } => vec![creation, delta],
            UserMemoryEffect::Retirement { tombstone, .. } => vec![tombstone],
        }
    }
}
