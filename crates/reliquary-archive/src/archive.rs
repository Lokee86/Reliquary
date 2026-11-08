//! Native conversation Archive persistence over the physical REL Container.
//!
//! This crate owns accepted turn, branch, and Episode semantics. Container owns
//! framing and bytes; Archive does not depend on the application or other owners.

mod history;
#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;

use crate::codec::{
    Record, decode_record, encode_branch, encode_episode, encode_file_blob,
    encode_fragment_definitions, encode_import_receipt, encode_turn,
};
use crate::{
    AcceptedTurnSource, ArchiveError, AttachmentInput, Branch, ContentId, Episode, EpisodeBoundary,
    EpisodeId, EpisodeOrigin, ImportReceipt, Node, ProjectAttachmentInput, ProjectFileRef,
    ProjectRepositoryKind, ResolvedTurn, SourceAttachment, TurnAcceptance, TurnInput,
};
use reliquary_container::{Container, ContainerError, ContainerIdentity, FileKind};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

mod evidence;
pub use evidence::{
    ArchiveSearchResult, MAX_EVIDENCE_RESULT_BYTES, MAX_EVIDENCE_SEARCH_BYTES,
    MAX_EVIDENCE_SEARCH_RESULTS, MAX_EVIDENCE_SEARCH_SCAN, MAX_EVIDENCE_TURNS,
};

pub struct Archive {
    owner_id: Option<[u8; 16]>,
    container_identity: Option<ContainerIdentity>,
    nodes: HashMap<(String, String), Node>,
    content: HashMap<ContentId, String>,
    pruned_nodes: HashMap<(String, String), [u8; 32]>,
    accepted_node_order: Vec<(String, String)>,
    node_archive_versions: HashMap<(String, String), u64>,
    activity_position_by_node: HashMap<(String, String), u64>,
    rel_turn_count: u64,
    source_attachments: HashMap<(String, String), Vec<SourceAttachment>>,
    file_content: HashMap<ContentId, Vec<u8>>,
    branches: BTreeMap<(String, String), Branch>,
    branch_history: Vec<(u64, Branch)>,
    episodes: BTreeMap<EpisodeId, Episode>,
    episode_versions: BTreeMap<EpisodeId, u64>,
    import_receipts: BTreeMap<[u8; 32], ImportReceipt>,
    import_receipt_versions: BTreeMap<[u8; 32], u64>,
    fragment_definitions: BTreeMap<crate::FragmentId, crate::FragmentDefinition>,
    fragment_versions: BTreeMap<crate::FragmentId, u64>,
    conversation_tombstones: BTreeMap<String, crate::ConversationTombstone>,
    conversation_titles: BTreeMap<String, crate::ConversationTitle>,
    archive_version: u64,
}

impl Archive {
    pub fn empty() -> Self {
        Self {
            owner_id: None,
            container_identity: None,
            nodes: HashMap::new(),
            content: HashMap::new(),
            pruned_nodes: HashMap::new(),
            accepted_node_order: Vec::new(),
            node_archive_versions: HashMap::new(),
            activity_position_by_node: HashMap::new(),
            rel_turn_count: 0,
            source_attachments: HashMap::new(),
            file_content: HashMap::new(),
            branches: BTreeMap::new(),
            branch_history: Vec::new(),
            episodes: BTreeMap::new(),
            episode_versions: BTreeMap::new(),
            import_receipts: BTreeMap::new(),
            import_receipt_versions: BTreeMap::new(),
            fragment_definitions: BTreeMap::new(),
            fragment_versions: BTreeMap::new(),
            conversation_tombstones: BTreeMap::new(),
            conversation_titles: BTreeMap::new(),
            archive_version: 0,
        }
    }

    /// Rebuild Archive-owned semantic state from a Container's accepted frames.
    /// Unknown frame families belong to other owners and are left uninterpreted.
    pub fn open(container: &Container) -> Result<Self, ArchiveError> {
        if container.identity().file_kind != FileKind::Reliquary {
            return Err(ArchiveError::WrongFileKind);
        }
        let mut archive = Self::empty();
        archive.owner_id = Some(container.owner_id());
        archive.container_identity = Some(container.identity());
        for frame in container.read_frames()? {
            if let Some(record) = decode_record(&frame.payload)? {
                archive.apply_record(record)?;
            }
        }
        Ok(archive)
    }

    /// Accept a source turn once. Its stable node ID is supplied by the source;
    /// content identity is SHA-256 over the exact UTF-8 bytes. Activity position
    /// is derived from first-accept order for `user` and `assistant` roles only.
    pub fn accept_turn(
        &mut self,
        container: &mut Container,
        input: TurnInput,
    ) -> Result<TurnAcceptance<Node>, ArchiveError> {
        self.bind_container(container)?;
        validate_text(&input.id, "node id")?;
        validate_text(&input.conversation_id, "conversation id")?;
        validate_text(&input.role, "role")?;
        if self
            .conversation_tombstones
            .contains_key(&input.conversation_id)
        {
            return Err(ArchiveError::InvalidField("terminal conversation"));
        }
        if input.parent_id.as_deref() == Some("") || input.principal_id.as_deref() == Some("") {
            return Err(ArchiveError::InvalidField("optional turn field"));
        }
        let key = (input.conversation_id.clone(), input.id.clone());
        let mut attachments = prepare_attachments(input.attachments)?;
        attachments.extend(prepare_project_attachments(input.project_attachments)?);
        validate_source_attachments(&attachments)?;
        if let Some(parent_id) = input.parent_id.as_deref()
            && !self
                .nodes
                .contains_key(&(input.conversation_id.clone(), parent_id.to_owned()))
        {
            return Err(ArchiveError::MissingParent);
        }
        let content_id = ContentId(Sha256::digest(input.content.as_bytes()).into());
        let node = Node {
            id: input.id,
            conversation_id: input.conversation_id,
            parent_id: input.parent_id,
            role: input.role,
            principal_id: input.principal_id,
            timestamp_ns: input.timestamp_ns,
            content_id,
        };
        if let Some(existing) = self.nodes.get(&key) {
            if existing == &node
                && self.content.get(&content_id) == Some(&input.content)
                && self
                    .source_attachments
                    .get(&key)
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
                    == attachments.as_slice()
            {
                return Ok(TurnAcceptance {
                    value: existing.clone(),
                    inserted: false,
                    activity_position: self.activity_position_by_node.get(&key).copied(),
                    rel_turn_count: self.rel_turn_count,
                });
            }
            return Err(ArchiveError::ConflictingNode);
        }
        if self
            .content
            .get(&content_id)
            .is_some_and(|existing| existing != &input.content)
        {
            return Err(ArchiveError::HashCollision);
        }
        self.accepted_node_order
            .try_reserve(1)
            .map_err(|_| ArchiveError::IndexCapacityExhausted)?;
        self.node_archive_versions
            .try_reserve(1)
            .map_err(|_| ArchiveError::IndexCapacityExhausted)?;
        let activity_position = if activity_role(&node.role) {
            let next = self
                .rel_turn_count
                .checked_add(1)
                .ok_or(ArchiveError::ActivityPositionExhausted)?;
            self.activity_position_by_node
                .try_reserve(1)
                .map_err(|_| ArchiveError::IndexCapacityExhausted)?;
            Some(next)
        } else {
            None
        };
        let encoded = encode_turn(&node, &input.content, &attachments)?;
        let archive_version = self.next_archive_version()?;
        let mut new_file_content = HashMap::new();
        for attachment in &attachments {
            if attachment.project_reference.is_some() {
                continue;
            }
            if let Some(existing) = self.file_content.get(&attachment.content_id) {
                if existing != &attachment.bytes {
                    return Err(ArchiveError::HashCollision);
                }
            } else if new_file_content
                .get(&attachment.content_id)
                .is_some_and(|existing| existing != &attachment.bytes)
            {
                return Err(ArchiveError::HashCollision);
            } else {
                new_file_content
                    .entry(attachment.content_id)
                    .or_insert_with(|| attachment.bytes.clone());
            }
        }
        for (content_id, bytes) in &new_file_content {
            container.append(&encode_file_blob(*content_id, bytes)?)?;
        }
        container.append(&encoded)?;
        container.sync()?;
        self.file_content.extend(new_file_content);
        self.nodes.insert(key.clone(), node.clone());
        self.content.entry(content_id).or_insert(input.content);
        self.source_attachments.insert(key.clone(), attachments);
        self.accepted_node_order.push(key.clone());
        self.node_archive_versions
            .insert(key.clone(), archive_version);
        if let Some(position) = activity_position {
            self.activity_position_by_node.insert(key, position);
            self.rel_turn_count = position;
        }
        self.archive_version = archive_version;
        Ok(TurnAcceptance {
            value: node,
            inserted: true,
            activity_position,
            rel_turn_count: self.rel_turn_count,
        })
    }

    /// Create or advance a branch. An existing branch may only move to a leaf
    /// descended from its prior leaf, preserving the branch's history.
    pub fn put_branch(
        &mut self,
        container: &mut Container,
        branch: Branch,
    ) -> Result<bool, ArchiveError> {
        self.bind_container(container)?;
        validate_text(&branch.id, "branch id")?;
        validate_text(&branch.conversation_id, "conversation id")?;
        if self
            .conversation_tombstones
            .contains_key(&branch.conversation_id)
        {
            return Err(ArchiveError::InvalidField("terminal conversation"));
        }
        validate_text(&branch.leaf_node_id, "branch leaf node id")?;
        self.require_node(&branch.conversation_id, &branch.leaf_node_id)?;
        if let Some(existing) = self
            .branches
            .get(&(branch.conversation_id.clone(), branch.id.clone()))
        {
            if existing == &branch {
                return Ok(false);
            }
            let path = self.branch_path(&branch.conversation_id, &branch.leaf_node_id)?;
            if !path.iter().any(|node| node.id == existing.leaf_node_id) {
                return Err(ArchiveError::InvalidBranchRevision);
            }
        }
        let archive_version = self.next_archive_version()?;
        let encoded = encode_branch(&branch)?;
        container.append(&encoded)?;
        container.sync()?;
        self.branch_history.push((archive_version, branch.clone()));
        self.branches
            .insert((branch.conversation_id.clone(), branch.id.clone()), branch);
        self.archive_version = archive_version;
        Ok(true)
    }

    /// Finalize an Episode over an ancestor-to-descendant range. Its stable ID
    /// depends only on conversation and endpoint node identities.
    pub fn put_episode(
        &mut self,
        container: &mut Container,
        conversation_id: &str,
        start_node_id: &str,
        end_node_id: &str,
        origin: EpisodeOrigin,
        boundary: EpisodeBoundary,
        finalized_at_ns: i64,
    ) -> Result<EpisodeId, ArchiveError> {
        self.bind_container(container)?;
        validate_text(conversation_id, "conversation id")?;
        if self.conversation_tombstones.contains_key(conversation_id) {
            return Err(ArchiveError::InvalidField("terminal conversation"));
        }
        let end = self.require_node(conversation_id, end_node_id)?;
        if !self
            .branch_path(conversation_id, end_node_id)?
            .iter()
            .any(|node| node.id == start_node_id)
        {
            return Err(ArchiveError::InvalidEpisodeRange);
        }
        if finalized_at_ns < end.timestamp_ns {
            return Err(ArchiveError::InvalidEpisodeRange);
        }
        let episode = Episode::new(
            conversation_id.to_owned(),
            start_node_id.to_owned(),
            end_node_id.to_owned(),
            origin,
            boundary,
            end.timestamp_ns,
            finalized_at_ns,
        );
        if let Some(existing) = self.episodes.get(&episode.id) {
            if existing == &episode {
                return Ok(episode.id);
            }
            return Err(ArchiveError::ConflictingEpisode);
        }
        let archive_version = self.next_archive_version()?;
        let encoded = encode_episode(&episode)?;
        container.append(&encoded)?;
        container.sync()?;
        self.episodes.insert(episode.id, episode.clone());
        self.episode_versions.insert(episode.id, archive_version);
        self.archive_version = archive_version;
        Ok(episode.id)
    }

    /// Persist an owner-validated source-to-native mapping receipt after its
    /// turns, branches, and files have been accepted. The receipt contains no
    /// transcript or attachment bytes.
    pub fn put_import_receipt(
        &mut self,
        container: &mut Container,
        mut receipt: ImportReceipt,
    ) -> Result<ImportReceipt, ArchiveError> {
        self.bind_container(container)?;
        if receipt.owner_uuid != container.owner_id() {
            return Err(ArchiveError::WrongOwner);
        }
        if receipt.conversations.iter().any(|conversation| {
            self.conversation_tombstones
                .contains_key(&conversation.conversation_id)
        }) {
            return Err(ArchiveError::InvalidField("terminal conversation"));
        }
        receipt.receipt_digest = import_receipt_digest(&receipt)?;
        self.validate_import_receipt(&receipt)?;
        if let Some(existing) = self.import_receipts.get(&receipt.import_key) {
            return if existing == &receipt {
                Ok(existing.clone())
            } else {
                Err(ArchiveError::ConflictingImportReceipt)
            };
        }
        let next_version = self.next_archive_version()?;
        let encoded = encode_import_receipt(&receipt)?;
        container.append(&encoded)?;
        container.sync()?;
        self.import_receipts
            .insert(receipt.import_key, receipt.clone());
        self.import_receipt_versions
            .insert(receipt.import_key, next_version);
        self.archive_version = next_version;
        Ok(receipt)
    }

    pub fn import_receipt(&self, import_key: &[u8; 32]) -> Option<&ImportReceipt> {
        self.import_receipts.get(import_key)
    }

    pub fn import_receipts(&self) -> Vec<ImportReceipt> {
        self.import_receipts.values().cloned().collect()
    }

    /// Return one accepted receipt witness for an exact native node.
    pub fn imported_turn_reference(
        &self,
        conversation: &str,
        node: &str,
    ) -> Option<(
        &ImportReceipt,
        &crate::ImportedConversation,
        &crate::ImportedMessage,
    )> {
        self.import_receipts.values().find_map(|receipt| {
            let conversation = receipt
                .conversations
                .iter()
                .find(|record| record.conversation_id == conversation)?;
            let message = conversation
                .messages
                .iter()
                .find(|record| record.node_id.as_deref() == Some(node))?;
            Some((receipt, conversation, message))
        })
    }

    pub fn branch_turns(
        &self,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        let branch = self
            .branches
            .get(&(conversation_id.to_owned(), branch_id.to_owned()))
            .ok_or(ArchiveError::MissingBranch)?;
        self.resolve_path(self.branch_path(conversation_id, &branch.leaf_node_id)?)
    }

    /// Select deterministic fragment windows from this owner's accepted branch.
    pub fn branch_fragment_windows(
        &self,
        conversation_id: &str,
        branch_id: &str,
        config: crate::FragmentConfig,
        close_tail: bool,
    ) -> Result<Vec<crate::Fragment>, ArchiveError> {
        let branch = self
            .branch(conversation_id, branch_id)
            .ok_or(ArchiveError::MissingBranch)?;
        let nodes = self
            .branch_path(conversation_id, &branch.leaf_node_id)?
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        crate::path_fragment_windows(&nodes, config, close_tail)
    }

    /// Persist immutable deterministic fragment definitions for the current
    /// accepted branch cut. Repeating the same publication is a no-op.
    pub fn define_branch_fragments(
        &mut self,
        container: &mut Container,
        conversation_id: &str,
        branch_id: &str,
        config: crate::FragmentConfig,
        close_tail: bool,
    ) -> Result<Vec<crate::FragmentDefinition>, ArchiveError> {
        self.bind_container(container)?;
        if self.conversation_tombstones.contains_key(conversation_id) {
            return Err(ArchiveError::InvalidField("terminal conversation"));
        }
        let fragments =
            self.branch_fragment_windows(conversation_id, branch_id, config, close_tail)?;
        let proposed = fragments
            .into_iter()
            .map(|fragment| crate::FragmentDefinition {
                fragment,
                branch_id: branch_id.to_owned(),
                archive_cut: self.archive_version,
                config,
                close_tail,
            })
            .collect::<Vec<_>>();
        let mut output = Vec::with_capacity(proposed.len());
        let mut new_definitions = Vec::new();
        for definition in proposed {
            if let Some(existing) = self.fragment_definitions.get(&definition.fragment.id) {
                if existing.fragment != definition.fragment {
                    return Err(ArchiveError::ConflictingFragmentDefinition);
                }
                output.push(existing.clone());
            } else {
                new_definitions.push(definition.clone());
                output.push(definition);
            }
        }
        if new_definitions.is_empty() {
            return Ok(output);
        }
        let next_version = self.next_archive_version()?;
        let encoded = encode_fragment_definitions(&new_definitions)?;
        container.append(&encoded)?;
        container.sync()?;
        for definition in &new_definitions {
            self.fragment_definitions
                .insert(definition.fragment.id, definition.clone());
            self.fragment_versions
                .insert(definition.fragment.id, next_version);
        }
        self.archive_version = next_version;
        Ok(output)
    }

    pub fn fragment_definition(&self, id: crate::FragmentId) -> Option<&crate::FragmentDefinition> {
        self.fragment_definitions.get(&id)
    }

    pub fn fragment_definitions(&self) -> Vec<crate::FragmentDefinition> {
        self.fragment_definitions.values().cloned().collect()
    }

    pub fn historical_turns(
        &self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.resolve_path(self.branch_path(conversation_id, leaf_node_id)?)
    }

    /// Resolve the exact accepted node range named by an Episode.
    ///
    /// The returned text and source identities are rehydrated from this
    /// Archive. The Episode start must be an ancestor of its end node.
    pub fn episode_turns(&self, id: EpisodeId) -> Result<Option<Vec<ResolvedTurn>>, ArchiveError> {
        let Some(episode) = self.episodes.get(&id) else {
            return Ok(None);
        };
        let mut path = self.branch_path(&episode.conversation_id, &episode.end_node_id)?;
        let start = path
            .iter()
            .position(|node| node.id == episode.start_node_id)
            .ok_or(ArchiveError::InvalidEpisodeRange)?;
        let selected = path.drain(start..).collect();
        self.resolve_path(selected).map(Some)
    }

    /// Rehydrate one turn by its immutable conversation and node identity.
    pub fn resolved_turn(
        &self,
        conversation_id: &str,
        node_id: &str,
    ) -> Result<Option<ResolvedTurn>, ArchiveError> {
        let Some(node) = self.node(conversation_id, node_id) else {
            return Ok(None);
        };
        if self
            .pruned_nodes
            .contains_key(&(conversation_id.to_owned(), node_id.to_owned()))
        {
            return Err(ArchiveError::MissingContent);
        }
        let content = self
            .content
            .get(&node.content_id)
            .ok_or(ArchiveError::MissingContent)?
            .clone();
        Ok(Some(ResolvedTurn {
            node_id: node.id.clone(),
            role: node.role.clone(),
            principal_id: node.principal_id.clone(),
            timestamp_ns: node.timestamp_ns,
            content,
        }))
    }

    pub fn branch(&self, conversation_id: &str, branch_id: &str) -> Option<&Branch> {
        self.branches
            .get(&(conversation_id.to_owned(), branch_id.to_owned()))
    }

    pub fn branches(&self) -> Vec<Branch> {
        self.branches.values().cloned().collect()
    }

    pub fn management_branches<'a>(
        &'a self,
        conversation: &'a str,
    ) -> impl Iterator<Item = &'a Branch> {
        self.branches
            .range((conversation.to_owned(), String::new())..)
            .map(|(_, branch)| branch)
            .take_while(move |branch| branch.conversation_id == conversation)
    }

    pub fn management_all_branches(&self) -> impl Iterator<Item = &Branch> {
        self.branches.values()
    }

    /// Select a root-to-leaf page before materializing any turn content.
    pub fn management_transcript_nodes(
        &self,
        conversation: &str,
        leaf: &str,
        offset: usize,
        limit: usize,
    ) -> Result<(usize, Vec<String>), ArchiveError> {
        let mut total = 0usize;
        let mut current = Some(leaf);
        while let Some(id) = current {
            let node = self.require_node(conversation, id)?;
            total += 1;
            current = node.parent_id.as_deref();
        }
        if limit == 0 || offset >= total {
            return Ok((total, Vec::new()));
        }
        let end = offset.saturating_add(limit).min(total);
        let mut nodes = Vec::new();
        current = Some(leaf);
        for depth in 0..total.saturating_sub(offset) {
            let node =
                self.require_node(conversation, current.ok_or(ArchiveError::MissingParent)?)?;
            if depth >= total - end {
                nodes.push(node.id.clone());
            }
            current = node.parent_id.as_deref();
        }
        nodes.reverse();
        Ok((total, nodes))
    }

    pub fn episode(&self, id: EpisodeId) -> Option<&Episode> {
        self.episodes.get(&id)
    }

    pub fn episodes(&self) -> Vec<Episode> {
        self.episodes.values().cloned().collect()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Confirm that this projection was opened for, or accepted into, the
    /// supplied owner container before another subsystem relies on its source
    /// identities as owner-local evidence.
    pub fn validate_owner(&self, container: &Container) -> Result<(), ArchiveError> {
        if self.owner_id == Some(container.owner_id())
            && self.container_identity == Some(container.identity())
        {
            Ok(())
        } else {
            Err(ArchiveError::WrongOwner)
        }
    }

    /// Read canonical turn text by immutable source identity. Derived indexes
    /// use this to validate byte spans and must still rehydrate on retrieval.
    pub fn content_for_node(&self, conversation_id: &str, node_id: &str) -> Option<&str> {
        if self
            .pruned_nodes
            .contains_key(&(conversation_id.to_owned(), node_id.to_owned()))
        {
            return None;
        }
        let node = self.node(conversation_id, node_id)?;
        self.content.get(&node.content_id).map(String::as_str)
    }

    pub fn node(&self, conversation_id: &str, node_id: &str) -> Option<&Node> {
        self.nodes
            .get(&(conversation_id.to_owned(), node_id.to_owned()))
    }

    pub fn conversation_tombstone(
        &self,
        conversation_id: &str,
    ) -> Option<&crate::ConversationTombstone> {
        self.conversation_tombstones.get(conversation_id)
    }

    pub fn conversation_title(&self, conversation_id: &str) -> Option<&str> {
        self.conversation_titles
            .get(conversation_id)
            .map(|title| title.title.as_str())
            .or_else(|| {
                self.import_receipts.values().find_map(|receipt| {
                    receipt
                        .conversations
                        .iter()
                        .find(|conversation| conversation.conversation_id == conversation_id)
                        .and_then(|conversation| conversation.title.as_deref())
                })
            })
    }

    pub fn set_conversation_title(
        &mut self,
        container: &mut Container,
        conversation_id: &str,
        title: &str,
    ) -> Result<bool, ArchiveError> {
        self.bind_container(container)?;
        validate_text(conversation_id, "conversation id")?;
        validate_text(title, "conversation title")?;
        if !self
            .nodes
            .keys()
            .any(|(conversation, _)| conversation == conversation_id)
        {
            return Err(ArchiveError::MissingNode);
        }
        if self.conversation_title(conversation_id) == Some(title) {
            return Ok(false);
        }
        let bytes = crate::codec::encode_conversation_title(conversation_id, title)?;
        container.append(&bytes)?;
        container.sync()?;
        self.apply_record(Record::ConversationTitle(
            conversation_id.to_owned(),
            title.to_owned(),
        ))?;
        Ok(true)
    }

    pub fn tombstone_conversation(
        &mut self,
        container: &mut Container,
        conversation_id: &str,
    ) -> Result<bool, ArchiveError> {
        self.validate_owner(container)?;
        validate_text(conversation_id, "conversation id")?;
        if self.conversation_tombstones.contains_key(conversation_id) {
            return Ok(false);
        }
        if !self
            .nodes
            .keys()
            .any(|(conversation, _)| conversation == conversation_id)
        {
            return Err(ArchiveError::MissingNode);
        }
        let bytes = crate::codec::encode_conversation_tombstone(conversation_id)?;
        container.append(&bytes)?;
        container.sync()?;
        self.apply_record(Record::ConversationTombstone(conversation_id.to_owned()))?;
        Ok(true)
    }

    pub fn pruned_turns(&self) -> Vec<(Node, [u8; 32])> {
        let mut values = self
            .pruned_nodes
            .iter()
            .filter_map(|(key, digest)| self.nodes.get(key).cloned().map(|node| (node, *digest)))
            .collect::<Vec<_>>();
        values
            .sort_by(|a, b| (&a.0.conversation_id, &a.0.id).cmp(&(&b.0.conversation_id, &b.0.id)));
        values
    }

    pub fn rel_turn_count(&self) -> u64 {
        self.rel_turn_count
    }

    /// Archive-local accepted record clock. One tick is allocated for each
    /// newly accepted Turn, Branch revision, or Episode; FileBlob companions
    /// and exact retries do not allocate a tick.
    pub fn archive_version(&self) -> u64 {
        self.archive_version
    }

    /// Archive clock assigned when this immutable source node was first
    /// accepted. Replays and later branch changes do not move the node cut.
    pub fn node_archive_version(&self, conversation_id: &str, node_id: &str) -> Option<u64> {
        self.node_archive_versions
            .get(&(conversation_id.to_owned(), node_id.to_owned()))
            .copied()
    }

    fn next_archive_version(&self) -> Result<u64, ArchiveError> {
        self.archive_version
            .checked_add(1)
            .ok_or(ArchiveError::ArchiveVersionExhausted)
    }

    pub fn activity_position_for_turn(&self, conversation_id: &str, node_id: &str) -> Option<u64> {
        self.activity_position_by_node
            .get(&(conversation_id.to_owned(), node_id.to_owned()))
            .copied()
    }

    /// Immutable source keys in first-accept order with their optional,
    /// role-qualified activity positions. This projection invents no time.
    pub fn accepted_turn_sources(&self) -> Vec<AcceptedTurnSource> {
        self.accepted_node_order
            .iter()
            .map(|(conversation_id, node_id)| AcceptedTurnSource {
                conversation_id: conversation_id.clone(),
                node_id: node_id.clone(),
                activity_position: self
                    .activity_position_by_node
                    .get(&(conversation_id.clone(), node_id.clone()))
                    .copied(),
            })
            .collect()
    }

    pub fn files_for_source(&self, conversation_id: &str, node_id: &str) -> Vec<SourceAttachment> {
        self.source_attachments
            .get(&(conversation_id.to_owned(), node_id.to_owned()))
            .cloned()
            .unwrap_or_default()
    }

    /// The physical owner identity is authoritative in the Container header.
    pub fn owner_id(&self) -> Option<[u8; 16]> {
        self.owner_id
    }

    fn apply_record(&mut self, record: Record) -> Result<(), ArchiveError> {
        match record {
            Record::Turn(node, content, attachments) => {
                if self
                    .conversation_tombstones
                    .contains_key(&node.conversation_id)
                {
                    return Err(ArchiveError::CorruptRecord("turn after terminal tombstone"));
                }
                validate_text(&node.id, "node id")?;
                validate_text(&node.conversation_id, "conversation id")?;
                validate_text(&node.role, "role")?;
                if node.parent_id.as_deref() == Some("") || node.principal_id.as_deref() == Some("")
                {
                    return Err(ArchiveError::CorruptRecord("empty optional turn field"));
                }
                let key = (node.conversation_id.clone(), node.id.clone());
                if let Some(parent_id) = node.parent_id.as_deref()
                    && !self
                        .nodes
                        .contains_key(&(node.conversation_id.clone(), parent_id.to_owned()))
                {
                    return Err(ArchiveError::MissingParent);
                }
                if Sha256::digest(content.as_bytes()).as_slice() != node.content_id.0 {
                    return Err(ArchiveError::CorruptContent);
                }
                let mut attachments = attachments;
                for attachment in &mut attachments {
                    if attachment.project_reference.is_none() {
                        let bytes = self
                            .file_content
                            .get(&attachment.content_id)
                            .ok_or(ArchiveError::MissingContent)?
                            .clone();
                        attachment.bytes = bytes;
                    }
                }
                if let Some(existing) = self.nodes.get(&key) {
                    if existing == &node
                        && self.content.get(&node.content_id) == Some(&content)
                        && self
                            .source_attachments
                            .get(&key)
                            .map(Vec::as_slice)
                            .unwrap_or(&[])
                            == attachments.as_slice()
                    {
                        return Ok(());
                    }
                    return Err(ArchiveError::ConflictingNode);
                }
                if self
                    .content
                    .get(&node.content_id)
                    .is_some_and(|existing| existing != &content)
                {
                    return Err(ArchiveError::HashCollision);
                }
                validate_source_attachments(&attachments)?;
                let archive_version = self.next_archive_version()?;
                self.accepted_node_order
                    .try_reserve(1)
                    .map_err(|_| ArchiveError::IndexCapacityExhausted)?;
                self.node_archive_versions
                    .try_reserve(1)
                    .map_err(|_| ArchiveError::IndexCapacityExhausted)?;
                let position = if activity_role(&node.role) {
                    let next = self
                        .rel_turn_count
                        .checked_add(1)
                        .ok_or(ArchiveError::ActivityPositionExhausted)?;
                    self.activity_position_by_node
                        .try_reserve(1)
                        .map_err(|_| ArchiveError::IndexCapacityExhausted)?;
                    Some(next)
                } else {
                    None
                };
                self.nodes.insert(key.clone(), node.clone());
                self.content.entry(node.content_id).or_insert(content);
                self.source_attachments.insert(key.clone(), attachments);
                let key = (node.conversation_id.clone(), node.id.clone());
                self.accepted_node_order.push(key.clone());
                self.node_archive_versions
                    .insert(key.clone(), archive_version);
                if let Some(position) = position {
                    self.activity_position_by_node.insert(key, position);
                    self.rel_turn_count = position;
                }
                self.archive_version = archive_version;
            }
            Record::PrunedTurn(node, mut attachments, manifest_digest) => {
                validate_text(&node.id, "node id")?;
                validate_text(&node.conversation_id, "conversation id")?;
                validate_text(&node.role, "role")?;
                let key = (node.conversation_id.clone(), node.id.clone());
                if self.nodes.contains_key(&key) || self.pruned_nodes.contains_key(&key) {
                    return Err(ArchiveError::ConflictingNode);
                }
                if let Some(parent_id) = node.parent_id.as_deref()
                    && !self
                        .nodes
                        .contains_key(&(node.conversation_id.clone(), parent_id.to_owned()))
                {
                    return Err(ArchiveError::MissingParent);
                }
                for attachment in &mut attachments {
                    if attachment.project_reference.is_none() {
                        attachment.bytes = self
                            .file_content
                            .get(&attachment.content_id)
                            .ok_or(ArchiveError::MissingContent)?
                            .clone();
                    }
                }
                validate_source_attachments(&attachments)?;
                let archive_version = self.next_archive_version()?;
                let position = if activity_role(&node.role) {
                    Some(
                        self.rel_turn_count
                            .checked_add(1)
                            .ok_or(ArchiveError::ActivityPositionExhausted)?,
                    )
                } else {
                    None
                };
                self.nodes.insert(key.clone(), node);
                self.pruned_nodes.insert(key.clone(), manifest_digest);
                self.source_attachments.insert(key.clone(), attachments);
                self.accepted_node_order.push(key.clone());
                self.node_archive_versions
                    .insert(key.clone(), archive_version);
                if let Some(position) = position {
                    self.activity_position_by_node.insert(key, position);
                    self.rel_turn_count = position;
                }
                self.archive_version = archive_version;
            }
            Record::FileBlob(content_id, bytes) => {
                if ContentId(Sha256::digest(&bytes).into()) != content_id {
                    return Err(ArchiveError::CorruptContent);
                }
                if let Some(existing) = self.file_content.get(&content_id) {
                    if existing != &bytes {
                        return Err(ArchiveError::HashCollision);
                    }
                } else {
                    self.file_content.insert(content_id, bytes);
                }
            }
            Record::Branch(branch) => {
                if self
                    .conversation_tombstones
                    .contains_key(&branch.conversation_id)
                {
                    return Err(ArchiveError::CorruptRecord(
                        "branch after terminal tombstone",
                    ));
                }
                validate_text(&branch.id, "branch id")?;
                validate_text(&branch.conversation_id, "conversation id")?;
                validate_text(&branch.leaf_node_id, "branch leaf node id")?;
                self.require_node(&branch.conversation_id, &branch.leaf_node_id)?;
                let key = (branch.conversation_id.clone(), branch.id.clone());
                if let Some(existing) = self.branches.get(&key) {
                    if existing == &branch {
                        return Ok(());
                    }
                    if !self
                        .branch_path(&branch.conversation_id, &branch.leaf_node_id)?
                        .iter()
                        .any(|node| node.id == existing.leaf_node_id)
                    {
                        return Err(ArchiveError::InvalidBranchRevision);
                    }
                }
                let archive_version = self.next_archive_version()?;
                self.branch_history.push((archive_version, branch.clone()));
                self.branches.insert(key, branch);
                self.archive_version = archive_version;
            }
            Record::Episode(episode) => {
                if self
                    .conversation_tombstones
                    .contains_key(&episode.conversation_id)
                {
                    return Err(ArchiveError::CorruptRecord(
                        "Episode after terminal tombstone",
                    ));
                }
                validate_text(&episode.conversation_id, "conversation id")?;
                validate_text(&episode.start_node_id, "Episode start node id")?;
                validate_text(&episode.end_node_id, "Episode end node id")?;
                let end = self.require_node(&episode.conversation_id, &episode.end_node_id)?;
                if episode.id
                    != Episode::stable_id(
                        &episode.conversation_id,
                        &episode.start_node_id,
                        &episode.end_node_id,
                    )
                    || episode.source_through_ns != end.timestamp_ns
                    || episode.finalized_at_ns < episode.source_through_ns
                    || !self
                        .branch_path(&episode.conversation_id, &episode.end_node_id)?
                        .iter()
                        .any(|node| node.id == episode.start_node_id)
                {
                    return Err(ArchiveError::InvalidEpisodeRange);
                }
                if let Some(existing) = self.episodes.get(&episode.id) {
                    if existing != &episode {
                        return Err(ArchiveError::ConflictingEpisode);
                    }
                } else {
                    let archive_version = self.next_archive_version()?;
                    self.episode_versions.insert(episode.id, archive_version);
                    self.episodes.insert(episode.id, episode);
                    self.archive_version = archive_version;
                }
            }
            Record::Import(receipt) => {
                if receipt.conversations.iter().any(|conversation| {
                    self.conversation_tombstones
                        .contains_key(&conversation.conversation_id)
                }) {
                    return Err(ArchiveError::CorruptRecord(
                        "import after terminal tombstone",
                    ));
                }
                self.validate_import_receipt(&receipt)?;
                if let Some(existing) = self.import_receipts.get(&receipt.import_key) {
                    return if existing == &receipt {
                        Ok(())
                    } else {
                        Err(ArchiveError::ConflictingImportReceipt)
                    };
                }
                let archive_version = self.next_archive_version()?;
                self.import_receipt_versions
                    .insert(receipt.import_key, archive_version);
                self.import_receipts.insert(receipt.import_key, receipt);
                self.archive_version = archive_version;
            }
            Record::FragmentDefinitions(definitions) => {
                if definitions.is_empty() {
                    return Err(ArchiveError::CorruptRecord(
                        "empty fragment definition record",
                    ));
                }
                let archive_cut = self.archive_version;
                if definitions.iter().any(|definition| {
                    self.conversation_tombstones
                        .contains_key(&definition.fragment.conversation_id)
                }) {
                    return Err(ArchiveError::CorruptRecord(
                        "fragment after terminal tombstone",
                    ));
                }
                let mut expected = BTreeMap::new();
                for definition in &definitions {
                    if definition.archive_cut != archive_cut {
                        return Err(ArchiveError::CorruptRecord("fragment cut mismatch"));
                    }
                    let fragments = self.branch_fragment_windows(
                        &definition.fragment.conversation_id,
                        &definition.branch_id,
                        definition.config,
                        definition.close_tail,
                    )?;
                    if !fragments.contains(&definition.fragment)
                        || expected
                            .insert(definition.fragment.id, definition)
                            .is_some()
                    {
                        return Err(ArchiveError::CorruptRecord("invalid fragment definition"));
                    }
                }
                if expected.len() != definitions.len()
                    || definitions.iter().any(|definition| {
                        self.fragment_definitions
                            .contains_key(&definition.fragment.id)
                    })
                {
                    return Err(ArchiveError::CorruptRecord("duplicate fragment definition"));
                }
                let archive_version = self.next_archive_version()?;
                for definition in definitions {
                    self.fragment_versions
                        .insert(definition.fragment.id, archive_version);
                    self.fragment_definitions
                        .insert(definition.fragment.id, definition);
                }
                self.archive_version = archive_version;
            }
            Record::ConversationTombstone(conversation_id) => {
                validate_text(&conversation_id, "conversation id")?;
                if !self
                    .nodes
                    .keys()
                    .any(|(conversation, _)| conversation == &conversation_id)
                {
                    return Err(ArchiveError::MissingNode);
                }
                if self.conversation_tombstones.contains_key(&conversation_id) {
                    return Err(ArchiveError::CorruptRecord(
                        "duplicate conversation tombstone",
                    ));
                }
                let archive_version = self.next_archive_version()?;
                self.conversation_tombstones.insert(
                    conversation_id.clone(),
                    crate::ConversationTombstone {
                        conversation_id,
                        archive_version,
                    },
                );
                self.archive_version = archive_version;
            }
            Record::ConversationTitle(conversation_id, title) => {
                validate_text(&conversation_id, "conversation id")?;
                validate_text(&title, "conversation title")?;
                if !self
                    .nodes
                    .keys()
                    .any(|(conversation, _)| conversation == &conversation_id)
                {
                    return Err(ArchiveError::MissingNode);
                }
                let archive_version = self.next_archive_version()?;
                self.conversation_titles.insert(
                    conversation_id.clone(),
                    crate::ConversationTitle {
                        conversation_id,
                        title,
                        archive_version,
                    },
                );
                self.archive_version = archive_version;
            }
        }
        Ok(())
    }

    fn validate_import_receipt(&self, receipt: &ImportReceipt) -> Result<(), ArchiveError> {
        if receipt.schema_version != 1
            || Some(receipt.owner_uuid) != self.owner_id
            || receipt.source_digest != receipt.import_key
            || !valid_import_text(&receipt.source_namespace)
            || !valid_import_text(&receipt.source_format)
            || import_receipt_digest(receipt)? != receipt.receipt_digest
        {
            return Err(ArchiveError::CorruptRecord(
                "invalid import receipt identity",
            ));
        }
        let mut conversation_sources = std::collections::BTreeSet::new();
        let mut native_conversations = std::collections::BTreeSet::new();
        for conversation in &receipt.conversations {
            if !valid_import_text(&conversation.source_id)
                || !valid_import_text(&conversation.conversation_id)
                || !conversation_sources.insert(conversation.source_id.as_str())
                || !native_conversations.insert(conversation.conversation_id.as_str())
            {
                return Err(ArchiveError::CorruptRecord(
                    "invalid imported conversation mapping",
                ));
            }
            let mut source_messages = std::collections::BTreeSet::new();
            for message in &conversation.messages {
                if !valid_import_text(&message.source_id)
                    || !valid_import_text(&message.revision)
                    || !source_messages.insert(message.source_id.as_str())
                {
                    return Err(ArchiveError::CorruptRecord(
                        "invalid imported message mapping",
                    ));
                }
                if message.source_author.as_ref().is_some_and(|author| {
                    receipt.source_format != crate::HERMES_COPIED_SOURCE_FORMAT
                        || author
                            .id
                            .as_deref()
                            .is_some_and(|id| !valid_import_text(id))
                }) {
                    return Err(ArchiveError::CorruptRecord(
                        "invalid imported source author",
                    ));
                }
                if message
                    .source_parent_id
                    .as_deref()
                    .is_some_and(|parent| !valid_import_text(parent))
                    || message
                        .replaces
                        .as_deref()
                        .is_some_and(|value| !valid_import_text(value))
                {
                    return Err(ArchiveError::CorruptRecord(
                        "invalid imported message ancestry",
                    ));
                }
                match (message.deleted, message.node_id.as_deref()) {
                    (true, None) if message.artifacts.is_empty() => {}
                    (false, Some(node_id)) if valid_import_text(node_id) => {
                        let node = self
                            .node(&conversation.conversation_id, node_id)
                            .ok_or(ArchiveError::MissingNode)?;
                        let source_attachments = self
                            .source_attachments
                            .get(&(conversation.conversation_id.clone(), node_id.to_owned()))
                            .map(Vec::as_slice)
                            .unwrap_or(&[]);
                        if message.artifacts.len() != source_attachments.len() {
                            return Err(ArchiveError::InvalidAttachment);
                        }
                        let mut artifact_sources = std::collections::BTreeSet::new();
                        for artifact in &message.artifacts {
                            if !valid_import_text(&artifact.source_id)
                                || !valid_import_text(&artifact.source_ref)
                                || !artifact_sources.insert(artifact.source_id.as_str())
                            {
                                return Err(ArchiveError::InvalidAttachment);
                            }
                            let found = source_attachments.iter().any(|attachment| {
                                attachment.id == artifact.file_id
                                    && attachment.content_id.0 == artifact.sha256
                            });
                            if !found {
                                return Err(ArchiveError::InvalidAttachment);
                            }
                        }
                        if node.conversation_id != conversation.conversation_id {
                            return Err(ArchiveError::MissingNode);
                        }
                    }
                    _ => {
                        return Err(ArchiveError::CorruptRecord(
                            "invalid imported message mapping",
                        ));
                    }
                }
            }
            for message in &conversation.messages {
                if message
                    .source_parent_id
                    .as_deref()
                    .is_some_and(|parent| !source_messages.contains(parent))
                {
                    return Err(ArchiveError::CorruptRecord(
                        "missing imported source parent",
                    ));
                }
            }
            let mut source_branches = std::collections::BTreeSet::new();
            for branch in &conversation.branches {
                if !valid_import_text(&branch.source_id)
                    || !valid_import_text(&branch.branch_id)
                    || !source_branches.insert(branch.source_id.as_str())
                {
                    return Err(ArchiveError::CorruptRecord(
                        "invalid imported branch mapping",
                    ));
                }
                if let Some(leaf) = branch.leaf_node_id.as_deref() {
                    let accepted = self.branch(&conversation.conversation_id, &branch.branch_id);
                    if !valid_import_text(leaf)
                        || !accepted.is_some_and(|value| {
                            value.leaf_node_id == leaf && value.canonical == branch.canonical
                        })
                    {
                        return Err(ArchiveError::MissingBranch);
                    }
                } else if self
                    .branch(&conversation.conversation_id, &branch.branch_id)
                    .is_some()
                {
                    return Err(ArchiveError::CorruptRecord(
                        "all-deleted imported branch has a native head",
                    ));
                }
            }
        }
        Ok(())
    }

    fn require_node(&self, conversation_id: &str, node_id: &str) -> Result<&Node, ArchiveError> {
        self.nodes
            .get(&(conversation_id.to_owned(), node_id.to_owned()))
            .ok_or(ArchiveError::MissingNode)
    }

    fn bind_container(&mut self, container: &Container) -> Result<(), ArchiveError> {
        if container.identity().file_kind != FileKind::Reliquary {
            return Err(ArchiveError::WrongFileKind);
        }
        match self.owner_id {
            Some(owner_id)
                if owner_id != container.owner_id()
                    || self.container_identity != Some(container.identity()) =>
            {
                Err(ArchiveError::WrongOwner)
            }
            Some(_) => Ok(()),
            None => {
                *self = Self::open(container)?;
                Ok(())
            }
        }
    }

    fn branch_path(
        &self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<&Node>, ArchiveError> {
        let mut path = Vec::new();
        let mut current = Some(leaf_node_id);
        while let Some(node_id) = current {
            let node = self.require_node(conversation_id, node_id)?;
            path.push(node);
            current = node.parent_id.as_deref();
        }
        path.reverse();
        Ok(path)
    }

    fn resolve_path(&self, nodes: Vec<&Node>) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        nodes
            .into_iter()
            .map(|node| {
                let content = self
                    .content
                    .get(&node.content_id)
                    .ok_or(ArchiveError::MissingContent)?
                    .clone();
                Ok(ResolvedTurn {
                    node_id: node.id.clone(),
                    role: node.role.clone(),
                    principal_id: node.principal_id.clone(),
                    timestamp_ns: node.timestamp_ns,
                    content,
                })
            })
            .collect()
    }
}

impl Default for Archive {
    fn default() -> Self {
        Self::empty()
    }
}

fn validate_text(value: &str, field: &'static str) -> Result<(), ArchiveError> {
    if value.is_empty() {
        Err(ArchiveError::InvalidField(field))
    } else {
        Ok(())
    }
}

fn valid_import_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && !value.contains('\0')
}

fn import_receipt_digest(receipt: &ImportReceipt) -> Result<[u8; 32], ArchiveError> {
    let mut unsigned = receipt.clone();
    unsigned.receipt_digest = [0; 32];
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|_| ArchiveError::CorruptRecord("invalid import receipt"))?;
    Ok(Sha256::digest(bytes).into())
}

fn activity_role(role: &str) -> bool {
    matches!(role, "user" | "assistant")
}

const FILE_ID_DOMAIN: &[u8] = &[
    0x43, 0x56, 0x41, 0x46, 0x49, 0x4c, 0x45, 0x31, 0x2d, 0x49, 0x44, 0,
];
const PROJECT_FILE_ID_DOMAIN: &[u8] = &[
    0x57, 0x41, 0x52, 0x4c, 0x4f, 0x43, 0x4b, 0x2d, 0x50, 0x52, 0x4f, 0x4a, 0x45, 0x43, 0x54, 0x2d,
    0x46, 0x49, 0x4c, 0x45, 0x31, 0x2d, 0x49, 0x44, 0,
];

fn prepare_attachments(
    inputs: Vec<AttachmentInput>,
) -> Result<Vec<SourceAttachment>, ArchiveError> {
    let mut attachments = Vec::with_capacity(inputs.len());
    for input in inputs {
        validate_text(&input.filename, "attachment filename")?;
        if let Some(mime) = input.mime_type.as_deref() {
            validate_text(mime, "attachment MIME type")?;
        }
        let byte_length =
            u64::try_from(input.bytes.len()).map_err(|_| ArchiveError::FieldTooLarge)?;
        let content_id = ContentId(Sha256::digest(&input.bytes).into());
        let id = file_id(
            &input.filename,
            input.mime_type.as_deref(),
            content_id,
            byte_length,
        );
        attachments.push(SourceAttachment {
            id,
            content_id,
            filename: input.filename,
            mime_type: input.mime_type,
            byte_length,
            bytes: input.bytes,
            project_reference: None,
        });
    }
    validate_source_attachments(&attachments)?;
    Ok(attachments)
}

fn prepare_project_attachments(
    inputs: Vec<ProjectAttachmentInput>,
) -> Result<Vec<SourceAttachment>, ArchiveError> {
    inputs
        .into_iter()
        .map(|input| {
            validate_project_file_ref(&input.reference, input.content_id)?;
            validate_text(&input.filename, "attachment filename")?;
            if let Some(mime) = input.mime_type.as_deref() {
                validate_text(mime, "attachment MIME type")?;
            }
            let id = project_file_id(
                &input.filename,
                input.mime_type.as_deref(),
                input.content_id,
                input.byte_length,
                &input.reference,
            );
            Ok(SourceAttachment {
                id,
                content_id: input.content_id,
                filename: input.filename,
                mime_type: input.mime_type,
                byte_length: input.byte_length,
                bytes: Vec::new(),
                project_reference: Some(input.reference),
            })
        })
        .collect()
}

fn validate_source_attachments(attachments: &[SourceAttachment]) -> Result<(), ArchiveError> {
    let mut ids = std::collections::HashSet::new();
    for attachment in attachments {
        if !ids.insert(attachment.id) {
            return Err(ArchiveError::DuplicateAttachment);
        }
        if let Some(reference) = &attachment.project_reference {
            validate_project_file_ref(reference, attachment.content_id)?;
            if !attachment.bytes.is_empty() {
                return Err(ArchiveError::InvalidAttachment);
            }
        } else if attachment.byte_length != attachment.bytes.len() as u64
            || ContentId(Sha256::digest(&attachment.bytes).into()) != attachment.content_id
        {
            return Err(ArchiveError::InvalidAttachment);
        }
        validate_text(&attachment.filename, "attachment filename")?;
        if let Some(mime) = attachment.mime_type.as_deref() {
            validate_text(mime, "attachment MIME type")?;
        }
        if let Some(reference) = &attachment.project_reference {
            let expected = project_file_id(
                &attachment.filename,
                attachment.mime_type.as_deref(),
                attachment.content_id,
                attachment.byte_length,
                reference,
            );
            if expected != attachment.id {
                return Err(ArchiveError::InvalidAttachment);
            }
            continue;
        }
        if file_id(
            &attachment.filename,
            attachment.mime_type.as_deref(),
            attachment.content_id,
            attachment.byte_length,
        ) != attachment.id
        {
            return Err(ArchiveError::InvalidAttachment);
        }
    }
    Ok(())
}

fn file_id(
    filename: &str,
    mime_type: Option<&str>,
    content_id: ContentId,
    byte_length: u64,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(FILE_ID_DOMAIN);
    hasher.update(content_id.0);
    hasher.update(byte_length.to_le_bytes());
    hasher.update((filename.len() as u64).to_le_bytes());
    hasher.update(filename.as_bytes());
    let mime = mime_type.unwrap_or("");
    hasher.update((mime.len() as u64).to_le_bytes());
    hasher.update(mime.as_bytes());
    hasher.finalize().into()
}

/// Compute the native immutable file identity for exact imported bytes.
pub fn file_id_from_bytes(
    filename: &str,
    mime_type: Option<&str>,
    bytes: &[u8],
) -> Result<[u8; 32], ArchiveError> {
    validate_text(filename, "attachment filename")?;
    if let Some(mime_type) = mime_type {
        validate_text(mime_type, "attachment MIME type")?;
    }
    let byte_length = u64::try_from(bytes.len()).map_err(|_| ArchiveError::FieldTooLarge)?;
    let content_id = ContentId(Sha256::digest(bytes).into());
    Ok(file_id(filename, mime_type, content_id, byte_length))
}

fn project_file_id(
    filename: &str,
    mime_type: Option<&str>,
    content_id: ContentId,
    byte_length: u64,
    reference: &ProjectFileRef,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PROJECT_FILE_ID_DOMAIN);
    hasher.update([match reference.revision.repository.kind {
        ProjectRepositoryKind::Lore => 1,
        ProjectRepositoryKind::Git => 2,
    }]);
    for value in [
        &reference.revision.repository.repository_id,
        &reference.revision.repository.project_path,
        &reference.revision.revision,
        &reference.path,
    ] {
        hasher.update((value.len() as u64).to_le_bytes());
        hasher.update(value.as_bytes());
    }
    hasher.update(content_id.0);
    hasher.update(byte_length.to_le_bytes());
    hasher.update((filename.len() as u64).to_le_bytes());
    hasher.update(filename.as_bytes());
    let mime = mime_type.unwrap_or("");
    hasher.update((mime.len() as u64).to_le_bytes());
    hasher.update(mime.as_bytes());
    hasher.finalize().into()
}

pub fn validate_project_file_reference(reference: &ProjectFileRef) -> Result<(), ArchiveError> {
    let repository = &reference.revision.repository;
    if repository.repository_id.trim().is_empty() || reference.revision.revision.trim().is_empty() {
        return Err(ArchiveError::InvalidField("project repository identity"));
    }
    validate_repo_relative_path(&repository.project_path, true)?;
    validate_repo_relative_path(&reference.path, false)?;
    if repository.project_path != "."
        && !reference
            .path
            .starts_with(&format!("{}/", repository.project_path))
    {
        return Err(ArchiveError::InvalidField(
            "project file path outside project subtree",
        ));
    }
    Ok(())
}

fn validate_project_file_ref(
    reference: &ProjectFileRef,
    content_id: ContentId,
) -> Result<(), ArchiveError> {
    validate_project_file_reference(reference)?;
    if reference
        .content_hash
        .is_some_and(|hash| hash != content_id.0)
    {
        return Err(ArchiveError::InvalidAttachment);
    }
    Ok(())
}

fn validate_repo_relative_path(path: &str, allow_root: bool) -> Result<(), ArchiveError> {
    if allow_root && path == "." {
        return Ok(());
    }
    let windows_absolute = path
        .as_bytes()
        .get(0..2)
        .is_some_and(|p| p[0].is_ascii_alphabetic() && p[1] == b':');
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || windows_absolute
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err(ArchiveError::InvalidField(
            "project repository-relative path",
        ));
    }
    Ok(())
}

impl From<ContainerError> for ArchiveError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}
