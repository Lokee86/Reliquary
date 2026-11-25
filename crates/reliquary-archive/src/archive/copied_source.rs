use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub(crate) const MARKER: &[u8; 8] = b"RELCSR01";
const MAX_RECORD_BYTES: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CopiedSourceChange {
    Delete,
    Rewind {
        last_surviving_message_uid: Option<String>,
    },
}

/// A scoped owner publication request. The copied import receipt authenticates
/// the relationship between the namespace, source conversation and native copy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CopiedSourceChangeRequest {
    pub namespace: String,
    pub source_id: String,
    pub conversation_id: String,
    pub source_sha256: [u8; 32],
    pub event_id: String,
    pub change: CopiedSourceChange,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CopiedSourceChangeReceipt {
    pub event_id: String,
    pub source_sha256: [u8; 32],
    pub resulting_head_sha256: Option<[u8; 32]>,
    pub last_surviving_message_uid: Option<String>,
    pub archive_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AcceptedCopiedSourceChange {
    pub request: CopiedSourceChangeRequest,
    pub receipt: CopiedSourceChangeReceipt,
    pub branch_id: Option<String>,
    pub prior_leaf: Option<String>,
    pub surviving_node: Option<String>,
}

#[derive(Clone)]
pub struct CopiedSourceHead<'a> {
    pub branch_id: &'a str,
    pub node_id: Option<&'a str>,
    pub archive_version: u64,
}

#[derive(Default, Serialize, Deserialize)]
pub(super) struct CopiedSources {
    pub(super) changes: Vec<AcceptedCopiedSourceChange>,
    events: BTreeMap<(String, String, String), usize>,
    resulting_heads: BTreeMap<[u8; 32], usize>,
    branch_rewinds: BTreeMap<(String, String), u64>,
    removed_nodes: HashMap<String, HashMap<String, u64>>,
    pub(super) removal_version: u64,
}

impl CopiedSourceChangeRequest {
    fn event_key(&self) -> (String, String, String) {
        (
            self.namespace.clone(),
            self.conversation_id.clone(),
            self.event_id.clone(),
        )
    }
}

pub(crate) fn decode(bytes: &[u8]) -> Result<AcceptedCopiedSourceChange, ArchiveError> {
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(invalid());
    }
    let body = &bytes[MARKER.len()..];
    let record = serde_json::from_slice(body).map_err(|_| invalid())?;
    if serde_json::to_vec(&record).map_err(|_| invalid())? != body {
        return Err(invalid());
    }
    Ok(record)
}

impl Archive {
    pub fn source_removal_version(&self) -> u64 {
        self.copied_sources.removal_version
    }

    pub fn source_removed_at(&self, conversation: &str, node: &str, removal_cut: u64) -> bool {
        self.conversation_tombstones
            .get(conversation)
            .is_some_and(|tombstone| tombstone.archive_version <= removal_cut)
            || self
                .copied_sources
                .removed_nodes
                .get(conversation)
                .and_then(|nodes| nodes.get(node))
                .is_some_and(|version| *version <= removal_cut)
    }

    /// Inspect eligibility against the current or a historical removal cut.
    /// Check range validity separately when using coordinates as proof.
    pub fn source_range_removed_at(
        &self,
        conversation: &str,
        start: &str,
        end: &str,
        range_archive_version: u64,
        removal_cut: u64,
    ) -> bool {
        if self
            .conversation_tombstones
            .get(conversation)
            .is_some_and(|tombstone| tombstone.archive_version <= removal_cut)
        {
            return true;
        }
        let Some(removed) = self.copied_sources.removed_nodes.get(conversation) else {
            return false;
        };
        let mut current = Some(end);
        while let Some(id) = current {
            let Some(turn) = self.turns.get(conversation, id) else {
                return false;
            };
            if turn.archive_version > range_archive_version {
                return false;
            }
            if removed
                .get(id)
                .is_some_and(|version| *version <= removal_cut)
            {
                return true;
            }
            if id == start {
                return false;
            }
            current = turn.node.parent_id.as_deref();
        }
        false
    }

    pub fn source_removed(&self, conversation: &str, node: &str) -> bool {
        self.source_removed_at(conversation, node, self.archive_version())
    }

    pub fn source_range_removed(&self, conversation: &str, start: &str, end: &str) -> bool {
        self.source_range_removed_at(
            conversation,
            start,
            end,
            self.archive_version(),
            self.archive_version(),
        )
    }

    pub fn episode_source_removed(&self, id: EpisodeId) -> bool {
        self.episode(id).is_some_and(|episode| {
            self.source_range_removed(
                &episode.conversation_id,
                &episode.start_node_id,
                &episode.end_node_id,
            )
        })
    }

    pub fn copied_source_changes(&self) -> &[AcceptedCopiedSourceChange] {
        &self.copied_sources.changes
    }

    pub fn copied_source_change(
        &self,
        request: &CopiedSourceChangeRequest,
    ) -> Option<&AcceptedCopiedSourceChange> {
        let index = self.copied_sources.events.get(&request.event_key())?;
        self.copied_sources.changes.get(*index)
    }

    /// Resolve a pre-rewind import receipt or an acknowledged rewind head.
    pub fn copied_source_head(
        &self,
        namespace: &str,
        source_id: &str,
        conversation_id: &str,
        source_sha256: &[u8; 32],
    ) -> Option<CopiedSourceHead<'_>> {
        if let Some(index) = self.copied_sources.resulting_heads.get(source_sha256) {
            let accepted = self.copied_sources.changes.get(*index)?;
            let request = &accepted.request;
            if request.namespace != namespace
                || request.source_id != source_id
                || request.conversation_id != conversation_id
            {
                return None;
            }
            return Some(CopiedSourceHead {
                branch_id: accepted.branch_id.as_deref()?,
                node_id: accepted.surviving_node.as_deref(),
                archive_version: accepted.receipt.archive_version,
            });
        }
        let receipt = self.import_receipt(source_sha256)?;
        if receipt.source_namespace != namespace
            || receipt.source_format != crate::HERMES_COPIED_SOURCE_FORMAT
        {
            return None;
        }
        let conversation = receipt
            .conversations
            .iter()
            .find(|row| row.source_id == source_id && row.conversation_id == conversation_id)?;
        let node_id = conversation.messages.last()?.node_id.as_deref()?;
        let branch_id = match conversation.branches.as_slice() {
            [] => "hermes-copy:authorized-view",
            [branch] if branch.canonical && branch.leaf_node_id.as_deref() == Some(node_id) => {
                branch.branch_id.as_str()
            }
            _ => return None,
        };
        Some(CopiedSourceHead {
            branch_id,
            node_id: Some(node_id),
            archive_version: *self.import_receipt_versions.get(source_sha256)?,
        })
    }

    pub fn copied_source_head_is_current(
        &self,
        conversation: &str,
        head: &CopiedSourceHead<'_>,
    ) -> bool {
        if self.conversation_tombstones.contains_key(conversation)
            || self
                .copied_sources
                .branch_rewinds
                .get(&(conversation.to_owned(), head.branch_id.to_owned()))
                .is_some_and(|version| *version > head.archive_version)
        {
            return false;
        }
        match (self.branch(conversation, head.branch_id), head.node_id) {
            (Some(branch), Some(node)) => branch.canonical && branch.leaf_node_id == node,
            (None, None) => true,
            _ => false,
        }
    }

    pub fn publish_copied_source_change(
        &mut self,
        container: &mut Container,
        request: CopiedSourceChangeRequest,
    ) -> Result<CopiedSourceChangeReceipt, ArchiveError> {
        self.validate_owner(container)?;
        if let Some(existing) = self.copied_source_change(&request) {
            return if existing.request == request {
                Ok(existing.receipt.clone())
            } else {
                Err(invalid())
            };
        }
        let (record, removed) = self.prepare_copied_source_change(request)?;
        let mut bytes = MARKER.to_vec();
        bytes.extend(serde_json::to_vec(&record).map_err(|_| invalid())?);
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(invalid());
        }
        let coordinate = ArchivePublicationCoordinate {
            archive_version: record.receipt.archive_version,
            global_version: container
                .latest_version()
                .checked_add(1)
                .ok_or_else(invalid)?,
            transaction_time_ns: None,
            bound_reservation: false,
        };
        self.check_publication(container, coordinate.archive_version, Some(coordinate))?;
        self.persist_applying(container, Some(coordinate), &bytes)?;
        let receipt = record.receipt.clone();
        self.apply_copied_source_change(record, removed);
        Ok(receipt)
    }

    pub(super) fn replay_copied_source_change(
        &mut self,
        record: AcceptedCopiedSourceChange,
    ) -> Result<(), ArchiveError> {
        if self.copied_source_change(&record.request).is_some() {
            return Err(invalid());
        }
        let (expected, removed) = self.prepare_copied_source_change(record.request.clone())?;
        if record != expected {
            return Err(invalid());
        }
        self.apply_copied_source_change(record, removed);
        Ok(())
    }

    fn prepare_copied_source_change(
        &self,
        request: CopiedSourceChangeRequest,
    ) -> Result<(AcceptedCopiedSourceChange, Vec<String>), ArchiveError> {
        let conversation = &request.conversation_id;
        let head = self
            .copied_source_head(
                &request.namespace,
                &request.source_id,
                conversation,
                &request.source_sha256,
            )
            .ok_or_else(invalid)?;
        if !valid_identifier(&request.event_id) {
            return Err(invalid());
        }
        let archive_version = self.next_archive_version()?;
        let mut record = AcceptedCopiedSourceChange {
            receipt: CopiedSourceChangeReceipt {
                event_id: request.event_id.clone(),
                source_sha256: request.source_sha256,
                archive_version,
                resulting_head_sha256: None,
                last_surviving_message_uid: None,
            },
            branch_id: None,
            prior_leaf: None,
            surviving_node: None,
            request,
        };
        let request = &record.request;
        let conversation = &request.conversation_id;
        let CopiedSourceChange::Rewind {
            last_surviving_message_uid,
        } = &request.change
        else {
            return Ok((record, Vec::new()));
        };
        if !self.copied_source_head_is_current(conversation, &head) {
            return Err(invalid());
        }
        let path = match head.node_id {
            Some(leaf) => self.branch_path(conversation, leaf)?,
            None => Vec::new(),
        };
        let surviving_index = if let Some(uid) = last_surviving_message_uid {
            if !valid_identifier(uid) {
                return Err(invalid());
            }
            let index = path
                .iter()
                .position(|node| node.id == *uid)
                .ok_or_else(invalid)?;
            let (receipt, _, message) = self
                .imported_turn_reference(conversation, uid)
                .ok_or_else(invalid)?;
            if receipt.source_namespace != request.namespace
                || receipt.source_format != crate::HERMES_COPIED_SOURCE_FORMAT
                || message.source_id != *uid
            {
                return Err(invalid());
            }
            index + 1
        } else {
            0
        };
        let mut surviving = HashSet::new();
        for branch in self
            .management_branches(conversation)
            .filter(|branch| branch.id != head.branch_id)
        {
            surviving.extend(
                self.branch_path(conversation, &branch.leaf_node_id)?
                    .into_iter()
                    .map(|node| node.id.as_str()),
            );
        }
        let removed = path[surviving_index..]
            .iter()
            .filter(|node| !surviving.contains(node.id.as_str()))
            .map(|node| node.id.clone())
            .collect();
        record.branch_id = Some(head.branch_id.to_owned());
        record.prior_leaf = head.node_id.map(str::to_owned);
        record.surviving_node = last_surviving_message_uid.clone();
        record.receipt.last_surviving_message_uid = last_surviving_message_uid.clone();
        let mut hash = Sha256::new();
        hash.update(b"reliquary/copied-source-rewind-head/v1\0");
        hash.update(self.owner_id.ok_or(ArchiveError::WrongOwner)?);
        hash.update(serde_json::to_vec(&record).map_err(|_| invalid())?);
        let resulting_head = hash.finalize().into();
        if self.import_receipts.contains_key(&resulting_head)
            || self
                .copied_sources
                .resulting_heads
                .contains_key(&resulting_head)
        {
            return Err(invalid());
        }
        record.receipt.resulting_head_sha256 = Some(resulting_head);
        Ok((record, removed))
    }

    fn apply_copied_source_change(
        &mut self,
        record: AcceptedCopiedSourceChange,
        removed: Vec<String>,
    ) {
        let conversation = &record.request.conversation_id;
        let version = record.receipt.archive_version;
        let index = self.copied_sources.changes.len();
        if let (Some(branch_id), Some(resulting_head)) =
            (&record.branch_id, record.receipt.resulting_head_sha256)
        {
            let key = (conversation.clone(), branch_id.clone());
            if let Some(anchor) = &record.surviving_node {
                let branch = Branch {
                    id: branch_id.clone(),
                    conversation_id: conversation.clone(),
                    leaf_node_id: anchor.clone(),
                    canonical: true,
                };
                self.branch_history.push((version, branch.clone()));
                self.branches.insert(key.clone(), branch);
            } else {
                self.branches.remove(&key);
            }
            self.copied_sources.branch_rewinds.insert(key, version);
            self.copied_sources
                .resulting_heads
                .insert(resulting_head, index);
            let removed_nodes = self
                .copied_sources
                .removed_nodes
                .entry(conversation.clone())
                .or_default();
            for node in removed {
                removed_nodes.entry(node).or_insert(version);
            }
            self.copied_sources.removal_version = version;
        } else if !self.conversation_tombstones.contains_key(conversation) {
            self.apply_terminal_conversation(conversation.clone(), version);
        }
        self.copied_sources
            .events
            .insert(record.request.event_key(), index);
        self.copied_sources.changes.push(record);
        self.archive_version = version;
    }

    pub(super) fn apply_terminal_conversation(&mut self, conversation_id: String, version: u64) {
        self.conversation_tombstones.insert(
            conversation_id.clone(),
            crate::ConversationTombstone {
                conversation_id,
                archive_version: version,
            },
        );
        self.copied_sources.removal_version = version;
    }
}

fn valid_identifier(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.:-".contains(&byte))
}

fn invalid() -> ArchiveError {
    ArchiveError::InvalidField("conflicting copied source change")
}
