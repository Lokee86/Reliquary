//! Native conversation Archive persistence over the physical REL Container.
//!
//! This crate owns accepted turn, branch, and Episode semantics. Container owns
//! framing and bytes; Archive does not depend on the application or other owners.

mod fragment_passages;
mod history;
mod payload;
mod publications;
mod retained;
#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
mod turn_table;

use crate::codec::{
    Record, decode_record, encode_branch, encode_episode, encode_file_blob,
    encode_fragment_definitions, encode_import_receipt, encode_turn_with_text_location,
};
use crate::{
    AcceptedTurnSource, ArchiveError, ArchivePublicationBinding, ArchivePublicationCoordinate,
    AttachmentInput, Branch, ContentId, Episode, EpisodeBoundary, EpisodeId, EpisodeOrigin,
    ImportReceipt, Node, ProjectAttachmentInput, ProjectFileRef, ProjectRepositoryKind,
    ResolvedTurn, SourceAttachment, TurnAcceptance, TurnInput,
};
use history::BranchRevision;
use payload::{FrameRange, Payload};
use reliquary_container::{
    Container, ContainerError, ContainerIdentity, FileKind, LeasedFrameReader, ObjectRef,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::num::NonZeroU64;
use std::sync::Arc;
use turn_table::{AcceptedTurn, TurnTable};

pub(crate) fn archive_replay_frame(prefix: &[u8]) -> bool {
    if prefix.starts_with(b"RELPUB") {
        return match reliquary_container::decode_publication(prefix) {
            Ok(Some(publication)) => {
                publication.body.starts_with(b"RELAPB")
                    || publication
                        .body
                        .starts_with(crate::retained::PROJECT_BACKING_PREFIX)
            }
            _ => true,
        };
    }
    prefix.starts_with(b"RELAPB") || crate::codec::archive_record_prefix(prefix)
}

mod evidence;
pub use evidence::{
    ArchiveSearchResult, MAX_EVIDENCE_RESULT_BYTES, MAX_EVIDENCE_SEARCH_BYTES,
    MAX_EVIDENCE_SEARCH_RESULTS, MAX_EVIDENCE_SEARCH_SCAN, MAX_EVIDENCE_TURNS,
};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Archive {
    #[serde(skip)]
    reader: Option<LeasedFrameReader>,
    owner_id: Option<[u8; 16]>,
    container_identity: Option<ContainerIdentity>,
    turns: TurnTable,
    content: HashMap<ContentId, Payload<Arc<String>>>,
    rel_turn_count: u64,
    file_content: HashMap<ContentId, Payload<Vec<u8>>>,
    branches: BTreeMap<(String, String), Branch>,
    branch_history: Vec<(u64, BranchRevision)>,
    episodes: BTreeMap<EpisodeId, Episode>,
    episode_versions: BTreeMap<EpisodeId, u64>,
    import_receipts: BTreeMap<[u8; 32], ImportReceipt>,
    import_receipt_versions: BTreeMap<[u8; 32], u64>,
    fragment_definitions: BTreeMap<crate::FragmentId, crate::FragmentDefinition>,
    fragment_versions: BTreeMap<crate::FragmentId, u64>,
    conversation_tombstones: BTreeMap<String, crate::ConversationTombstone>,
    conversation_titles: BTreeMap<String, crate::ConversationTitle>,
    archive_version: u64,
    publication_bindings: Vec<ArchivePublicationBinding>,
    retained_publications: Vec<crate::RetainedArchivePublication>,
    retained_fragment_index: BTreeMap<crate::FragmentId, usize>,
    project_correlations: Vec<crate::ProjectCorrelation>,
    project_backing_bindings: Vec<crate::ProjectBackingBinding>,
    project_backing_indices: HashMap<[u8; 32], usize>,
}

impl Archive {
    pub fn empty() -> Self {
        Self {
            reader: None,
            owner_id: None,
            container_identity: None,
            turns: TurnTable::default(),
            content: HashMap::new(),
            rel_turn_count: 0,
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
            publication_bindings: Vec::new(),
            retained_publications: Vec::new(),
            retained_fragment_index: BTreeMap::new(),
            project_correlations: Vec::new(),
            project_backing_bindings: Vec::new(),
            project_backing_indices: HashMap::new(),
        }
    }

    /// Rebuild Archive-owned semantic state from a Container's accepted frames.
    /// Unknown frame families belong to other owners and are left uninterpreted.
    pub fn open(container: &Container) -> Result<Self, ArchiveError> {
        Self::open_with_payloads(container, false)
    }

    /// Rebuild Archive state while keeping accepted turn and file bytes on
    /// the supplied Container lease.
    pub fn open_leased(container: &Container) -> Result<Self, ArchiveError> {
        Self::open_with_payloads(container, true)
    }

    /// Bind an empty or verified checkpoint Archive to its owning lease.
    pub fn bind_read_lease(&mut self, container: &Container) -> Result<(), ArchiveError> {
        if self.owner_id.is_some() {
            self.validate_owner(container)?;
        } else {
            self.bind_empty_owner(container)?;
        }
        self.reader = Some(container.leased_frame_reader()?);
        Ok(())
    }

    fn open_with_payloads(container: &Container, leased: bool) -> Result<Self, ArchiveError> {
        Self::open_observing_prefixes(container, leased, archive_replay_frame, |_, _, _| {})
    }

    /// Observe Archive version and activity at selected physical frames.
    /// The selector must include all frames selected by `archive_replay_frame`.
    pub fn open_observing_prefixes(
        container: &Container,
        leased: bool,
        select: fn(&[u8]) -> bool,
        mut observe: impl FnMut(&[u8], u64, u64),
    ) -> Result<Self, ArchiveError> {
        let mut archive = Self::empty();
        if leased {
            archive.bind_read_lease(container)?;
        } else {
            archive.bind_empty_owner(container)?;
        }
        let mut cursor = container.matching_physical_cursor(|prefix| {
            prefix.starts_with(crate::PROJECT_BACKING_MARKER)
                || prefix.starts_with(crate::PROJECT_CORRELATION_MARKER)
        })?;
        let has_project_clock = cursor.next_frame()?.is_some();
        drop(cursor);
        if has_project_clock {
            let mut frames = container.scan_clocked_matching_frames(select)?;
            while let Some(frame) = frames.next_frame()? {
                observe(
                    frame.payload,
                    archive.archive_version,
                    archive.rel_turn_count,
                );
                archive.replay_frame(
                    container,
                    frame.object,
                    frame.payload,
                    frame.global_version,
                )?;
            }
        } else {
            let mut frames = container.scan_matching_frames(select)?;
            while let Some(frame) = frames.next_frame()? {
                observe(
                    frame.payload,
                    archive.archive_version,
                    archive.rel_turn_count,
                );
                archive.replay_frame(container, frame.object, frame.payload, 0)?;
            }
        }
        Ok(archive)
    }

    /// Apply a physical frame after the owning Container has admitted its
    /// clock, using the same semantics for initial and resumed replay.
    pub fn replay_frame(
        &mut self,
        container: &Container,
        object: ObjectRef,
        payload: &[u8],
        global_version: u64,
    ) -> Result<(), ArchiveError> {
        if let Some(publication) = crate::publication::in_frame(payload)? {
            self.replay_publication(container, object, payload, publication)?;
        } else if let Some(record) = decode_record(payload)? {
            match &record {
                Record::Retained(_) => {
                    return Err(ArchiveError::CorruptRecord(
                        "retained Archive record has no publication coordinate",
                    ));
                }
                Record::ProjectBacking(value) if value.global_version != global_version => {
                    return Err(ArchiveError::CorruptRecord(
                        "project backing does not match the physical global frontier",
                    ));
                }
                Record::ProjectCorrelation(value) if value.global_version > global_version => {
                    return Err(ArchiveError::CorruptRecord(
                        "project correlation exceeds its physical global frontier",
                    ));
                }
                _ => {}
            }
            self.apply_record(record, false, Some((object, payload)))?;
        }
        Ok(())
    }

    /// Accept a source turn once. Its stable node ID is supplied by the source;
    /// content identity is SHA-256 over the exact UTF-8 bytes. Activity position
    /// is derived from first-accept order for `user` and `assistant` roles only.
    pub fn accept_turn(
        &mut self,
        container: &mut Container,
        input: TurnInput,
    ) -> Result<TurnAcceptance<Node>, ArchiveError> {
        self.accept_turn_at(container, input, None)
    }

    /// Accept a turn at a supplied Archive/global publication coordinate.
    pub fn accept_turn_at(
        &mut self,
        container: &mut Container,
        input: TurnInput,
        coordinate: Option<ArchivePublicationCoordinate>,
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
        let mut attachments = prepare_attachments(input.attachments)?;
        attachments.extend(prepare_project_attachments(input.project_attachments)?);
        validate_source_attachments(&attachments, false, validate_inline_bytes)?;
        if let Some(parent_id) = input.parent_id.as_deref()
            && self.node(&input.conversation_id, parent_id).is_none()
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
        if let Some(existing) = self.turns.get(&node.conversation_id, &node.id) {
            if existing.node == node
                && self
                    .read_text(content_id)?
                    .is_some_and(|text| text.as_str() == input.content)
                && self.files_for_source(&node.conversation_id, &node.id)? == attachments
            {
                return Ok(TurnAcceptance {
                    value: existing.node.clone(),
                    inserted: false,
                    activity_position: existing.activity.map(NonZeroU64::get),
                    rel_turn_count: self.rel_turn_count,
                });
            }
            return Err(ArchiveError::ConflictingNode);
        }
        if self
            .read_text(content_id)?
            .is_some_and(|existing| existing.as_str() != input.content)
        {
            return Err(ArchiveError::HashCollision);
        }
        self.turns.reserve(&node.conversation_id)?;
        let activity = self.next_activity(&node.role)?;
        let (encoded, text_offset) =
            encode_turn_with_text_location(&node, &input.content, &attachments)?;
        let archive_version = self.next_archive_version()?;
        self.check_publication(container, archive_version, coordinate)?;
        let mut new_file_content = HashMap::new();
        for attachment in &attachments {
            if attachment.project_reference.is_some() {
                continue;
            }
            if let Some(existing) = self.read_file(attachment.content_id)? {
                if existing != attachment.bytes {
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
        let mut accepted_files = Vec::with_capacity(new_file_content.len());
        for (content_id, bytes) in new_file_content {
            let (encoded, offset) = encode_file_blob(content_id, &bytes)?;
            let frame = container.append(&encoded)?;
            let payload = if self.reader.is_some() {
                Payload::Leased(FrameRange::new(frame, offset, bytes.len()))
            } else {
                Payload::Resident(bytes)
            };
            accepted_files.push((content_id, payload));
        }
        let frame = self.persist_applying(container, coordinate, &encoded)?;
        let text = if self.reader.is_some() {
            let offset = frame.length - encoded.len() as u64 + text_offset;
            Payload::Leased(FrameRange::new(frame, offset, input.content.len()))
        } else {
            Payload::Resident(Arc::new(input.content))
        };
        self.file_content.extend(accepted_files);
        self.content.entry(content_id).or_insert(text);
        for attachment in &mut attachments {
            attachment.bytes = Vec::new();
        }
        self.record_turn(AcceptedTurn {
            node: node.clone(),
            archive_version,
            activity,
            attachments: attachments.into_boxed_slice(),
            prune: None,
        });
        Ok(TurnAcceptance {
            value: node,
            inserted: true,
            activity_position: activity.map(NonZeroU64::get),
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
        self.put_branch_at(container, branch, None)
    }

    /// Accept a Branch revision at a supplied Archive/global publication coordinate.
    pub fn put_branch_at(
        &mut self,
        container: &mut Container,
        branch: Branch,
        coordinate: Option<ArchivePublicationCoordinate>,
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
        self.check_publication(container, archive_version, coordinate)?;
        let frame = self.persist_applying(container, coordinate, &encoded)?;
        let range = self
            .reader
            .as_ref()
            .map(|_| FrameRange::new(frame, frame.length - encoded.len() as u64, encoded.len()));
        self.branch_history.push((
            archive_version,
            BranchRevision::new(&branch, range, &encoded),
        ));
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
        self.put_fragment_definitions(container, proposed)
    }

    fn put_fragment_definitions(
        &mut self,
        container: &mut Container,
        proposed: Vec<crate::FragmentDefinition>,
    ) -> Result<Vec<crate::FragmentDefinition>, ArchiveError> {
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
        let Some(turn) = self.turns.get(conversation_id, node_id) else {
            return Ok(None);
        };
        if turn.prune.is_some() {
            return Err(ArchiveError::MissingContent);
        }
        let node = &turn.node;
        let content = self
            .read_text(node.content_id)?
            .ok_or(ArchiveError::MissingContent)?
            .as_ref()
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
        self.turns.len()
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
    pub fn content_for_node(
        &self,
        conversation_id: &str,
        node_id: &str,
    ) -> Result<Option<Arc<String>>, ArchiveError> {
        let Some(turn) = self.turns.get(conversation_id, node_id) else {
            return Ok(None);
        };
        if turn.prune.is_some() {
            return Ok(None);
        }
        self.read_text(turn.node.content_id)
    }

    pub fn content_length_for_node(&self, conversation_id: &str, node_id: &str) -> Option<usize> {
        let turn = self.turns.get(conversation_id, node_id)?;
        if turn.prune.is_some() {
            return None;
        }
        self.text_length(turn.node.content_id)
    }

    pub fn node(&self, conversation_id: &str, node_id: &str) -> Option<&Node> {
        self.turns
            .get(conversation_id, node_id)
            .map(|turn| &turn.node)
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
        if !self.turns.contains_conversation(conversation_id) {
            return Err(ArchiveError::MissingNode);
        }
        if self.conversation_title(conversation_id) == Some(title) {
            return Ok(false);
        }
        let bytes = crate::codec::encode_conversation_title(conversation_id, title)?;
        container.append(&bytes)?;
        container.sync()?;
        self.apply_record(
            Record::ConversationTitle(conversation_id.to_owned(), title.to_owned()),
            false,
            None,
        )?;
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
        if !self.turns.contains_conversation(conversation_id) {
            return Err(ArchiveError::MissingNode);
        }
        let bytes = crate::codec::encode_conversation_tombstone(conversation_id)?;
        container.append(&bytes)?;
        container.sync()?;
        self.apply_record(
            Record::ConversationTombstone(conversation_id.to_owned()),
            false,
            None,
        )?;
        Ok(true)
    }

    pub fn pruned_turns(&self) -> Vec<(Node, [u8; 32])> {
        let mut values = self
            .turns
            .iter()
            .filter_map(|turn| {
                turn.prune
                    .as_deref()
                    .map(|digest| (turn.node.clone(), *digest))
            })
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

    /// Archive clock assigned when this Episode was first accepted.
    pub fn episode_archive_version(&self, id: EpisodeId) -> Option<u64> {
        self.episode_versions.get(&id).copied()
    }

    /// Archive clock assigned when this immutable source node was first
    /// accepted. Replays and later branch changes do not move the node cut.
    pub fn node_archive_version(&self, conversation_id: &str, node_id: &str) -> Option<u64> {
        self.turns
            .get(conversation_id, node_id)
            .map(|turn| turn.archive_version)
    }

    fn next_archive_version(&self) -> Result<u64, ArchiveError> {
        self.archive_version
            .checked_add(1)
            .ok_or(ArchiveError::ArchiveVersionExhausted)
    }

    fn next_activity(&self, role: &str) -> Result<Option<NonZeroU64>, ArchiveError> {
        if !activity_role(role) {
            return Ok(None);
        }
        let next = self
            .rel_turn_count
            .checked_add(1)
            .ok_or(ArchiveError::ActivityPositionExhausted)?;
        Ok(NonZeroU64::new(next))
    }

    fn record_turn(&mut self, turn: AcceptedTurn) {
        if let Some(activity) = turn.activity {
            self.rel_turn_count = activity.get();
        }
        self.archive_version = turn.archive_version;
        self.turns.push(turn);
    }

    pub fn activity_position_for_turn(&self, conversation_id: &str, node_id: &str) -> Option<u64> {
        self.turns
            .get(conversation_id, node_id)?
            .activity
            .map(NonZeroU64::get)
    }

    /// Immutable source keys in first-accept order with their optional,
    /// role-qualified activity positions. This projection invents no time.
    pub fn accepted_turn_sources(&self) -> Vec<AcceptedTurnSource> {
        self.turns
            .iter()
            .map(|turn| AcceptedTurnSource {
                conversation_id: turn.node.conversation_id.clone(),
                node_id: turn.node.id.clone(),
                activity_position: turn.activity.map(NonZeroU64::get),
            })
            .collect()
    }

    pub fn files_for_source(
        &self,
        conversation_id: &str,
        node_id: &str,
    ) -> Result<Vec<SourceAttachment>, ArchiveError> {
        let Some(turn) = self.turns.get(conversation_id, node_id) else {
            return Ok(Vec::new());
        };
        self.read_attachments(&turn.attachments)
    }

    fn read_attachments(
        &self,
        attachments: &[SourceAttachment],
    ) -> Result<Vec<SourceAttachment>, ArchiveError> {
        let mut attachments = attachments.to_vec();
        for attachment in &mut attachments {
            if attachment.project_reference.is_none() {
                attachment.bytes = self
                    .read_file(attachment.content_id)?
                    .ok_or(ArchiveError::MissingContent)?;
            }
        }
        Ok(attachments)
    }

    pub fn file_content_ids_for_source(
        &self,
        conversation_id: &str,
        node_id: &str,
    ) -> impl Iterator<Item = ContentId> + '_ {
        self.turns
            .get(conversation_id, node_id)
            .into_iter()
            .flat_map(|turn| turn.attachments.iter())
            .map(|attachment| attachment.content_id)
    }

    /// The physical owner identity is authoritative in the Container header.
    pub fn owner_id(&self) -> Option<[u8; 16]> {
        self.owner_id
    }

    fn apply_record(
        &mut self,
        record: Record<'_>,
        allow_retained_project_identity: bool,
        source: Option<(ObjectRef, &[u8])>,
    ) -> Result<(), ArchiveError> {
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
                if let Some(parent_id) = node.parent_id.as_deref()
                    && self.node(&node.conversation_id, parent_id).is_none()
                {
                    return Err(ArchiveError::MissingParent);
                }
                if Sha256::digest(content.as_bytes()).as_slice() != node.content_id.0 {
                    return Err(ArchiveError::CorruptContent);
                }
                self.validate_replayed_attachments(&attachments, allow_retained_project_identity)?;
                if let Some(existing) = self.turns.get(&node.conversation_id, &node.id) {
                    if existing.node == node
                        && self
                            .read_text(node.content_id)?
                            .is_some_and(|text| text.as_str() == content)
                        && existing.attachments.as_ref() == attachments.as_slice()
                    {
                        return Ok(());
                    }
                    return Err(ArchiveError::ConflictingNode);
                }
                if self
                    .read_text(node.content_id)?
                    .is_some_and(|existing| existing.as_str() != content)
                {
                    return Err(ArchiveError::HashCollision);
                }
                let archive_version = self.next_archive_version()?;
                self.turns.reserve(&node.conversation_id)?;
                let activity = self.next_activity(&node.role)?;
                let text = if let Some(range) = self.frame_range(source, content.as_bytes())? {
                    Payload::Leased(range)
                } else {
                    Payload::Resident(Arc::new(content.to_owned()))
                };
                self.content.entry(node.content_id).or_insert(text);
                self.record_turn(AcceptedTurn {
                    node,
                    archive_version,
                    activity,
                    attachments: attachments.into_boxed_slice(),
                    prune: None,
                });
            }
            Record::PrunedTurn(node, attachments, manifest_digest) => {
                validate_text(&node.id, "node id")?;
                validate_text(&node.conversation_id, "conversation id")?;
                validate_text(&node.role, "role")?;
                if self.node(&node.conversation_id, &node.id).is_some() {
                    return Err(ArchiveError::ConflictingNode);
                }
                if let Some(parent_id) = node.parent_id.as_deref()
                    && self.node(&node.conversation_id, parent_id).is_none()
                {
                    return Err(ArchiveError::MissingParent);
                }
                self.validate_replayed_attachments(&attachments, allow_retained_project_identity)?;
                let archive_version = self.next_archive_version()?;
                let activity = self.next_activity(&node.role)?;
                self.turns.reserve(&node.conversation_id)?;
                self.record_turn(AcceptedTurn {
                    node,
                    archive_version,
                    activity,
                    attachments: attachments.into_boxed_slice(),
                    prune: Some(Box::new(manifest_digest)),
                });
            }
            Record::FileBlob(content_id, bytes) => {
                if ContentId(Sha256::digest(bytes).into()) != content_id {
                    return Err(ArchiveError::CorruptContent);
                }
                if let Some(existing) = self.read_file(content_id)? {
                    if existing != bytes {
                        return Err(ArchiveError::HashCollision);
                    }
                } else {
                    let payload = if let Some(range) = self.frame_range(source, bytes)? {
                        Payload::Leased(range)
                    } else {
                        Payload::Resident(bytes.to_vec())
                    };
                    self.file_content.insert(content_id, payload);
                }
            }
            Record::Branch(branch, bytes) => {
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
                let leaf = self.require_node(&branch.conversation_id, &branch.leaf_node_id)?;
                let key = (branch.conversation_id.clone(), branch.id.clone());
                if let Some(existing) = self.branches.get(&key) {
                    if existing == &branch {
                        return Ok(());
                    }
                    if !self.has_ancestor(leaf, &existing.leaf_node_id)? {
                        return Err(ArchiveError::InvalidBranchRevision);
                    }
                }
                let archive_version = self.next_archive_version()?;
                let range = self.frame_range(source, bytes)?;
                self.branch_history
                    .push((archive_version, BranchRevision::new(&branch, range, bytes)));
                self.branches.insert(key, branch);
                self.archive_version = archive_version;
            }
            Record::Episode(episode) => {
                self.validate_replayed_episode(&episode)?;
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
                let mut branch_windows = BTreeMap::new();
                let mut passage_source = None;
                for definition in &definitions {
                    if definition.archive_cut != archive_cut {
                        return Err(ArchiveError::CorruptRecord("fragment cut mismatch"));
                    }
                    if definition.fragment.passage.is_some() {
                        self.validate_passage_definition(definition, &mut passage_source)?;
                    } else {
                        let key = (
                            definition.fragment.conversation_id.as_str(),
                            definition.branch_id.as_str(),
                            definition.config.turns,
                            definition.config.overlap,
                            definition.close_tail,
                        );
                        let windows = match branch_windows.entry(key) {
                            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                            std::collections::btree_map::Entry::Vacant(entry) => {
                                let windows = self
                                    .branch_fragment_windows(
                                        key.0,
                                        key.1,
                                        definition.config,
                                        key.4,
                                    )?
                                    .into_iter()
                                    .map(|window| (window.id, window))
                                    .collect::<BTreeMap<_, _>>();
                                entry.insert(windows)
                            }
                        };
                        if windows.get(&definition.fragment.id) != Some(&definition.fragment) {
                            return Err(ArchiveError::CorruptRecord("invalid fragment definition"));
                        }
                    }
                    if expected
                        .insert(definition.fragment.id, definition)
                        .is_some()
                    {
                        return Err(ArchiveError::CorruptRecord("duplicate fragment definition"));
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
                if !self.turns.contains_conversation(&conversation_id) {
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
            Record::Retained(value) => self.apply_retained_publication(value)?,
            Record::ProjectCorrelation(value) => self.apply_project_correlation(value)?,
            Record::ProjectBacking(value) => self.apply_project_backing(value)?,
            Record::ConversationTitle(conversation_id, title) => {
                validate_text(&conversation_id, "conversation id")?;
                validate_text(&title, "conversation title")?;
                if !self.turns.contains_conversation(&conversation_id) {
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
                        let turn = self
                            .turns
                            .get(&conversation.conversation_id, node_id)
                            .ok_or(ArchiveError::MissingNode)?;
                        let node = &turn.node;
                        let source_attachments = &turn.attachments;
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
        self.node(conversation_id, node_id)
            .ok_or(ArchiveError::MissingNode)
    }

    fn bind_empty_owner(&mut self, container: &Container) -> Result<(), ArchiveError> {
        if container.identity().file_kind != FileKind::Reliquary {
            return Err(ArchiveError::WrongFileKind);
        }
        self.owner_id = Some(container.owner_id());
        self.container_identity = Some(container.identity());
        Ok(())
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

    fn has_ancestor<'a>(
        &'a self,
        mut node: &'a Node,
        ancestor_id: &str,
    ) -> Result<bool, ArchiveError> {
        // Accepted turns already proved their parents at insertion. Stop once
        // the requested node is found instead of collecting the path to root.
        while node.id != ancestor_id {
            let Some(parent) = &node.parent_id else {
                return Ok(false);
            };
            node = self.require_node(&node.conversation_id, parent)?;
        }
        Ok(true)
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
                    .read_text(node.content_id)?
                    .ok_or(ArchiveError::MissingContent)?
                    .as_ref()
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
    validate_source_attachments(&attachments, false, validate_inline_bytes)?;
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

fn validate_inline_bytes(attachment: &SourceAttachment) -> Result<(), ArchiveError> {
    if attachment.byte_length != attachment.bytes.len() as u64
        || ContentId(Sha256::digest(&attachment.bytes).into()) != attachment.content_id
    {
        return Err(ArchiveError::InvalidAttachment);
    }
    Ok(())
}

fn validate_source_attachments(
    attachments: &[SourceAttachment],
    allow_retained_project_identity: bool,
    validate_inline: impl Fn(&SourceAttachment) -> Result<(), ArchiveError>,
) -> Result<(), ArchiveError> {
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
        } else {
            validate_inline(attachment)?;
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
            // A retained publication can preserve the ID assigned before Git offload.
            let retained_identity = allow_retained_project_identity
                && reference.content_hash == Some(attachment.content_id.0)
                && file_id(
                    &attachment.filename,
                    attachment.mime_type.as_deref(),
                    attachment.content_id,
                    attachment.byte_length,
                ) == attachment.id;
            if expected != attachment.id && !retained_identity {
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
