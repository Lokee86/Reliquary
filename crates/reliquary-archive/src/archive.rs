//! Native conversation Archive persistence over the physical REL Container.
//!
//! This crate owns accepted turn, branch, and Episode semantics. Container owns
//! framing and bytes; Archive does not depend on the application or other owners.

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;

use crate::codec::{
    Record, decode_record, encode_branch, encode_episode, encode_file_blob, encode_turn,
};
use crate::{
    AcceptedTurnSource, ArchiveError, AttachmentInput, Branch, ContentId, Episode, EpisodeBoundary,
    EpisodeId, EpisodeOrigin, Node, ProjectAttachmentInput, ProjectFileRef, ProjectRepositoryKind,
    ResolvedTurn, SourceAttachment, TurnAcceptance, TurnInput,
};
use reliquary_container::{Container, ContainerError, ContainerIdentity, FileKind};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

pub struct Archive {
    owner_id: Option<[u8; 16]>,
    container_identity: Option<ContainerIdentity>,
    nodes: HashMap<(String, String), Node>,
    content: HashMap<ContentId, String>,
    accepted_node_order: Vec<(String, String)>,
    activity_position_by_node: HashMap<(String, String), u64>,
    rel_turn_count: u64,
    source_attachments: HashMap<(String, String), Vec<SourceAttachment>>,
    file_content: HashMap<ContentId, Vec<u8>>,
    branches: BTreeMap<(String, String), Branch>,
    episodes: BTreeMap<EpisodeId, Episode>,
}

impl Archive {
    pub fn empty() -> Self {
        Self {
            owner_id: None,
            container_identity: None,
            nodes: HashMap::new(),
            content: HashMap::new(),
            accepted_node_order: Vec::new(),
            activity_position_by_node: HashMap::new(),
            rel_turn_count: 0,
            source_attachments: HashMap::new(),
            file_content: HashMap::new(),
            branches: BTreeMap::new(),
            episodes: BTreeMap::new(),
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
        if let Some(position) = activity_position {
            self.activity_position_by_node.insert(key, position);
            self.rel_turn_count = position;
        }
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
        let encoded = encode_branch(&branch)?;
        container.append(&encoded)?;
        container.sync()?;
        self.branches
            .insert((branch.conversation_id.clone(), branch.id.clone()), branch);
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
        let encoded = encode_episode(&episode)?;
        container.append(&encoded)?;
        container.sync()?;
        self.episodes.insert(episode.id, episode.clone());
        Ok(episode.id)
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
        let node = self.node(conversation_id, node_id)?;
        self.content.get(&node.content_id).map(String::as_str)
    }

    pub fn node(&self, conversation_id: &str, node_id: &str) -> Option<&Node> {
        self.nodes
            .get(&(conversation_id.to_owned(), node_id.to_owned()))
    }

    pub fn rel_turn_count(&self) -> u64 {
        self.rel_turn_count
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
                self.accepted_node_order
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
                if let Some(position) = position {
                    self.activity_position_by_node.insert(key, position);
                    self.rel_turn_count = position;
                }
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
                self.branches.insert(key, branch);
            }
            Record::Episode(episode) => {
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
                    self.episodes.insert(episode.id, episode);
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
