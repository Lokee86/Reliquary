use super::*;
use crate::staged_copy::StagedCopy;
use reliquary_memory_core::{MemoryDraft, PayloadState};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, io::ErrorKind, path::Path};

mod semantic;
use semantic::SemanticClosure;

pub(super) fn semantic_memory_closure(
    source: &CommonState,
    source_owner: [u8; 16],
    retained: BTreeSet<MemoryId>,
) -> Result<Vec<MemoryId>, String> {
    SemanticClosure::build(source, source_owner, retained)
        .map(|closure| closure.memories.into_iter().collect())
}

const LINEAGE_MAGIC: &[u8; 8] = b"RELREW01";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FreshHistoryIdentity {
    NewOwner([u8; 16]),
    PreserveOwner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreshHistoryArchive {
    Omit,
    RequiredProvenance,
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreshHistoryFreshness {
    Reset,
    PreserveCurrent,
    RetainHistory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FreshHistoryRequest {
    pub source_owner_id: [u8; 16],
    pub source_digest: [u8; 32],
    pub plan_digest: [u8; 32],
    pub identity: FreshHistoryIdentity,
    pub retained_memory_ids: Vec<MemoryId>,
    pub retain_all_revisions: bool,
    pub retain_revisions_since_memory_version: Option<u64>,
    pub retain_derived_state: bool,
    pub archive: FreshHistoryArchive,
    pub freshness: FreshHistoryFreshness,
    pub profile_ids: Vec<CompatibilityProfileId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FreshHistoryRewriteReport {
    pub source_owner_id: [u8; 16],
    pub destination_owner_id: [u8; 16],
    pub history_generation: [u8; 32],
    pub retained_memories: usize,
    pub retained_revisions: u64,
    pub retained_tombstones: u64,
    pub output_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FreshHistoryLineage {
    pub source_owner_id: [u8; 16],
    pub destination_owner_id: [u8; 16],
    pub source_digest: [u8; 32],
    pub plan_digest: [u8; 32],
    pub predecessor_generation: [u8; 32],
    pub history_generation: [u8; 32],
}

/// Build a new REL/PHY 2.1 history containing the selected Memory histories.
///
/// Semantic closures are replayed through their owning stores. Operational
/// Dream/import/Insomnia evidence is rejected before staging until it has an
/// exact replay contract, so it can never disappear silently.
pub fn rewrite_memory_history_copy(
    source_path: &Path,
    destination_path: &Path,
    request: &FreshHistoryRequest,
    graph_admission: &GraphAdmission,
) -> Result<FreshHistoryRewriteReport, String> {
    if request.plan_digest == [0; 32]
        || request.source_digest == [0; 32]
        || request.retained_memory_ids.len() > 16_384
        || request
            .retained_memory_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || (request.retain_all_revisions && request.retain_revisions_since_memory_version.is_some())
        || request
            .retain_revisions_since_memory_version
            .is_some_and(|cut| cut == 0)
    {
        return Err("invalid or noncanonical fresh-history request".into());
    }
    if destination_path.exists() {
        return Err("rewrite destination already exists".into());
    }
    let source_bytes = fs::read(source_path).map_err(|error| error.to_string())?;
    if Sha256::digest(&source_bytes).as_slice() != request.source_digest {
        return Err("rewrite source changed after planning".into());
    }
    let source = Container::open_read_only(source_path).map_err(|error| error.to_string())?;
    if source.owner_id() != request.source_owner_id {
        return Err("rewrite source owner changed after planning".into());
    }
    let destination_owner_id = match request.identity {
        FreshHistoryIdentity::NewOwner(owner) if owner != source.owner_id() && owner != [0; 16] => {
            owner
        }
        FreshHistoryIdentity::NewOwner(_) => {
            return Err("new-owner rewrite requires a distinct nonzero owner".into());
        }
        FreshHistoryIdentity::PreserveOwner => source.owner_id(),
    };

    let source_common = match source.identity().file_kind {
        FileKind::Reliquary => {
            RelSession::open(&source, graph_admission, [], [])
                .map_err(|error| error.to_string())?
                .common
        }
        FileKind::Phylactery => {
            PhySession::open(&source, graph_admission, [])
                .map_err(|error| error.to_string())?
                .common
        }
    };
    if source.identity().file_kind == FileKind::Phylactery
        && (request.archive != FreshHistoryArchive::Omit
            || request.freshness != FreshHistoryFreshness::Reset)
    {
        return Err("PHY rewrite cannot acquire REL Archive or Freshness state".into());
    }
    if !source_common.maintenance.is_empty() {
        return Err("fresh-history source has prior destructive maintenance state".into());
    }
    reject_unsupported_operational_state(&source)?;
    let selected: BTreeSet<_> = request.retained_memory_ids.iter().copied().collect();
    for id in &selected {
        source_common
            .memories
            .history(*id)
            .map_err(|error| error.to_string())?;
    }
    let semantic_closure = request
        .retain_derived_state
        .then(|| SemanticClosure::build(&source_common, source.owner_id(), selected.clone()))
        .transpose()?;
    if semantic_closure
        .as_ref()
        .is_some_and(|closure| closure.memories != selected)
    {
        return Err("fresh-history plan lacks complete semantic Memory closure".into());
    }

    let staged = StagedCopy::create(destination_path)?;
    let mut target = Container::create(&staged.path, source.identity(), destination_owner_id)
        .map_err(|error| error.to_string())?;
    super::create::initialize_owner(&mut target).map_err(|error| error.to_string())?;
    let mut target_memories = MemoryStore::open(&target).map_err(|error| error.to_string())?;
    if source.identity().file_kind == FileKind::Reliquary {
        copy_archive_state(&source, &mut target, request, &source_common)?;
    }
    let mut retained_revisions = 0u64;
    let mut retained_tombstones = 0u64;
    for id in &selected {
        let history = source_common
            .memories
            .history(*id)
            .map_err(|error| error.to_string())?;
        let revisions = selected_history_revisions(&history, request)?;
        let mut expected_revision = 0;
        for revision in revisions {
            let PayloadState::Available(body) = &revision.payload else {
                return Err("selected Memory revision payload is unavailable".into());
            };
            let draft = draft_from(revision, body);
            let published = target_memories
                .publish(&mut target, Some(*id), expected_revision, draft)
                .map_err(|error| error.to_string())?;
            if published.revision.record.id != *id {
                return Err("fresh-history replay changed stable Memory identity".into());
            }
            expected_revision = published.revision.record.revision;
            retained_revisions += 1;
        }
        if history.tombstone.is_some() {
            target_memories
                .tombstone(&mut target, *id, expected_revision)
                .map_err(|error| error.to_string())?;
            retained_tombstones += 1;
        }
    }
    if let Some(closure) = &semantic_closure {
        closure.copy_to(
            &source_common,
            source.owner_id(),
            &mut target,
            &mut target_memories,
        )?;
    } else if source_common.perception.entities().next().is_some()
        || source_common
            .perception
            .historical_entities()
            .next()
            .is_some()
        || source_common.perception.mentions().next().is_some()
        || source_common.perception.relationships().next().is_some()
        || source_common
            .perception
            .historical_relationships()
            .next()
            .is_some()
        || source_common.graph.graph_version() != 0
        || source_common.communities.current().is_some()
        || source_common.ego.version() != 0
    {
        return Err("rewrite policy omits present derived or Ego state".into());
    }
    if source.identity().file_kind == FileKind::Reliquary {
        copy_freshness_state(&source, &mut target, request)?;
    }
    copy_vectors(&source, &source_common, &mut target, request)?;
    let predecessor_generation = owner_history_generation(source_path)?;
    let history_generation: [u8; 32] = Sha256::digest(
        [
            b"reliquary-history-generation-v1\0".as_slice(),
            predecessor_generation.as_slice(),
            request.source_digest.as_slice(),
            request.plan_digest.as_slice(),
            destination_owner_id.as_slice(),
        ]
        .concat(),
    )
    .into();
    let lineage = FreshHistoryLineage {
        source_owner_id: source.owner_id(),
        destination_owner_id,
        source_digest: request.source_digest,
        plan_digest: request.plan_digest,
        predecessor_generation,
        history_generation,
    };
    let json = serde_json::to_vec(&lineage).map_err(|error| error.to_string())?;
    let mut bytes = Vec::with_capacity(16 + json.len() + 32);
    bytes.extend_from_slice(LINEAGE_MAGIC);
    bytes.extend_from_slice(&(json.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&json);
    bytes.extend_from_slice(&Sha256::digest(&bytes));
    target.append(&bytes).map_err(|error| error.to_string())?;
    target.sync().map_err(|error| error.to_string())?;
    drop(target_memories);
    drop(target);

    if Sha256::digest(fs::read(source_path).map_err(|error| error.to_string())?).as_slice()
        != request.source_digest
    {
        return Err("rewrite source changed during execution".into());
    }
    let reopened_graph = match &semantic_closure {
        Some(closure) => closure.replay_admission(&source_common, destination_owner_id)?,
        None => GraphAdmission::for_replay(destination_owner_id, [], [], [], [], []),
    };
    let reopened_count = match source.identity().file_kind {
        FileKind::Reliquary => {
            let reopened = RelSession::open_path(&staged.path, &reopened_graph, [], [])
                .map_err(|error| error.to_string())?;
            if let Some(closure) = &semantic_closure {
                closure.verify_reopened(
                    &source_common,
                    &reopened.common,
                    source.owner_id(),
                    destination_owner_id,
                )?;
            }
            count_memories(&reopened)?
        }
        FileKind::Phylactery => {
            let reopened = PhySession::open_path(&staged.path, &reopened_graph, [])
                .map_err(|error| error.to_string())?;
            if let Some(closure) = &semantic_closure {
                closure.verify_reopened(
                    &source_common,
                    &reopened.common,
                    source.owner_id(),
                    destination_owner_id,
                )?;
            }
            count_memories(&reopened)?
        }
    };
    if reopened_count != selected.len() {
        return Err("independent rewrite reopen changed selected Memory count".into());
    }
    // The staged candidate has been synced and independently reopened. Never
    // replace a destination created by another writer since the initial check.
    let output_bytes = fs::metadata(&staged.path)
        .map_err(|error| error.to_string())?
        .len();
    publish_rewrite_candidate(&staged, destination_path)?;
    Ok(FreshHistoryRewriteReport {
        source_owner_id: source.owner_id(),
        destination_owner_id,
        history_generation,
        retained_memories: selected.len(),
        retained_revisions,
        retained_tombstones,
        output_bytes,
    })
}

fn publish_rewrite_candidate(staged: &StagedCopy, destination: &Path) -> Result<(), String> {
    match fs::hard_link(&staged.path, destination) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            Err("rewrite destination already exists".into())
        }
        Err(error) => Err(format!(
            "could not publish verified rewritten owner: {error}"
        )),
    }
}

#[cfg(test)]
#[path = "fresh_history_rewrite/publication_tests.rs"]
mod publication_tests;

fn reject_unsupported_operational_state(source: &Container) -> Result<(), String> {
    const UNSUPPORTED: &[&[u8]] = &[
        b"CVDREAM",
        b"RELDR",
        b"RELINS",
        b"RELINSC",
        b"RELIEP",
        b"RELIFRS",
        b"RELX",
        b"RELECHO",
        reliquary_graph::USER_ASSOCIATION_MAGIC,
        crate::records::user_memory::CORRECTION,
        crate::records::user_memory::RETIREMENT,
        b"RELCMN01",
    ];
    for frame in source.read_frames().map_err(|error| error.to_string())? {
        let body = reliquary_container::decode_publication(&frame.payload)
            .map_err(|error| error.to_string())?
            .map(|publication| publication.body)
            .unwrap_or(frame.payload.as_slice());
        if UNSUPPORTED.iter().any(|prefix| body.starts_with(prefix)) {
            return Err(
                "fresh-history source contains operational evidence that cannot be safely replayed"
                    .into(),
            );
        }
    }
    Ok(())
}

fn copy_archive_state(
    source_container: &Container,
    target: &mut Container,
    request: &FreshHistoryRequest,
    common: &CommonState,
) -> Result<(), String> {
    if request.archive == FreshHistoryArchive::Omit {
        let archive = Archive::open(source_container).map_err(|error| error.to_string())?;
        if archive.node_count() != 0 {
            return Err("rewrite policy omits present REL Archive state".into());
        }
        return Ok(());
    }
    let source = Archive::open(source_container).map_err(|error| error.to_string())?;
    let mut required = BTreeSet::<(String, String)>::new();
    if request.archive == FreshHistoryArchive::All {
        required.extend(
            source
                .accepted_turn_sources()
                .into_iter()
                .map(|turn| (turn.conversation_id, turn.node_id)),
        );
    } else {
        for id in &request.retained_memory_ids {
            let history = common
                .memories
                .history(*id)
                .map_err(|error| error.to_string())?;
            for revision in selected_history_revisions(&history, request)? {
                let record = &revision.record;
                for pair in [
                    record
                        .content_source_conversation_id
                        .as_ref()
                        .zip(record.content_source_node_id.as_ref()),
                    record
                        .grounding_source_conversation_id
                        .as_ref()
                        .zip(record.grounding_source_node_id.as_ref()),
                ]
                .into_iter()
                .flatten()
                {
                    if let Some(node) = source.node(pair.0, pair.1) {
                        required.insert((node.conversation_id.clone(), node.id.clone()));
                    }
                }
                if let Some(reference) = &record.source_ref
                    && let Some(episode) =
                        source.episode(reliquary_archive::EpisodeId(reference.source_episode_id))
                {
                    required.insert((
                        episode.conversation_id.clone(),
                        reference.source_node_id.clone(),
                    ));
                }
            }
        }
        let seeds = required.clone();
        for (conversation, node_id) in seeds {
            let mut cursor = Some(node_id);
            while let Some(node_id) = cursor {
                let node = source
                    .node(&conversation, &node_id)
                    .ok_or("retained provenance references an absent Archive node")?;
                required.insert((conversation.clone(), node.id.clone()));
                cursor = node.parent_id.clone();
            }
        }
    }
    let mut destination = Archive::open(target).map_err(|error| error.to_string())?;
    for accepted in source.accepted_turn_sources() {
        if !required.contains(&(accepted.conversation_id.clone(), accepted.node_id.clone())) {
            continue;
        }
        let node = source
            .node(&accepted.conversation_id, &accepted.node_id)
            .ok_or("accepted Archive source is absent")?;
        let mut attachments = Vec::new();
        let mut project_attachments = Vec::new();
        for attachment in source.files_for_source(&accepted.conversation_id, &accepted.node_id) {
            if let Some(reference) = attachment.project_reference {
                project_attachments.push(reliquary_archive::ProjectAttachmentInput {
                    filename: attachment.filename,
                    mime_type: attachment.mime_type,
                    content_id: attachment.content_id,
                    byte_length: attachment.byte_length,
                    reference,
                });
            } else {
                attachments.push(reliquary_archive::AttachmentInput {
                    filename: attachment.filename,
                    mime_type: attachment.mime_type,
                    bytes: attachment.bytes,
                });
            }
        }
        destination
            .accept_turn(
                target,
                reliquary_archive::TurnInput {
                    id: node.id.clone(),
                    conversation_id: node.conversation_id.clone(),
                    parent_id: node.parent_id.clone(),
                    role: node.role.clone(),
                    principal_id: node.principal_id.clone(),
                    timestamp_ns: node.timestamp_ns,
                    content: source
                        .content_for_node(&node.conversation_id, &node.id)
                        .ok_or("Archive node content is unavailable")?
                        .to_owned(),
                    attachments,
                    project_attachments,
                },
            )
            .map_err(|error| error.to_string())?;
    }
    for branch in source.branches() {
        if required.contains(&(branch.conversation_id.clone(), branch.leaf_node_id.clone())) {
            destination
                .put_branch(target, branch)
                .map_err(|error| error.to_string())?;
        }
    }
    for episode in source.episodes() {
        if required.contains(&(
            episode.conversation_id.clone(),
            episode.start_node_id.clone(),
        )) && required.contains(&(episode.conversation_id.clone(), episode.end_node_id.clone()))
        {
            let id = destination
                .put_episode(
                    target,
                    &episode.conversation_id,
                    &episode.start_node_id,
                    &episode.end_node_id,
                    episode.origin,
                    episode.boundary,
                    episode.finalized_at_ns,
                )
                .map_err(|error| error.to_string())?;
            if id != episode.id {
                return Err("fresh-history rewrite changed stable Episode identity".into());
            }
        }
    }
    if request.archive == FreshHistoryArchive::All {
        for mut receipt in source.import_receipts() {
            receipt.owner_uuid = target.owner_id();
            destination
                .put_import_receipt(target, receipt)
                .map_err(|error| error.to_string())?;
        }
    }
    let mut fragment_jobs = source
        .fragment_definitions()
        .into_iter()
        .filter(|definition| {
            request.archive == FreshHistoryArchive::All
                || (required.contains(&(
                    definition.fragment.conversation_id.clone(),
                    definition.fragment.start_node_id.clone(),
                )) && required.contains(&(
                    definition.fragment.conversation_id.clone(),
                    definition.fragment.end_node_id.clone(),
                )))
        })
        .map(|definition| {
            (
                definition.fragment.conversation_id,
                definition.branch_id,
                definition.config,
                definition.close_tail,
            )
        })
        .collect::<Vec<_>>();
    fragment_jobs.sort_by(|left, right| (&left.0, &left.1).cmp(&(&right.0, &right.1)));
    fragment_jobs.dedup();
    for (conversation, branch, config, close_tail) in fragment_jobs {
        destination
            .define_branch_fragments(target, &conversation, &branch, config, close_tail)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn copy_freshness_state(
    source_container: &Container,
    target: &mut Container,
    request: &FreshHistoryRequest,
) -> Result<(), String> {
    let source = FreshnessStore::open(source_container)?;
    let selected = request
        .retained_memory_ids
        .iter()
        .map(|id| FreshnessMemoryId(id.0))
        .collect::<BTreeSet<_>>();
    if request.freshness == FreshHistoryFreshness::Reset {
        return Ok(());
    }
    let mut destination = FreshnessStore::open(target)?;
    if source.policy() != destination.policy() {
        return Err("fresh-history Freshness policy cannot be changed after initialization".into());
    }
    let entries = source.replay_entries();
    for entry in &entries {
        match entry {
            reliquary_freshness_store::FreshnessReplayEntry::Birth {
                memory_id,
                accepted_turn,
                admitted_at_birth,
            } if selected.contains(memory_id) => {
                destination.create_birth(target, *memory_id, *accepted_turn, *admitted_at_birth)?;
            }
            reliquary_freshness_store::FreshnessReplayEntry::Admission {
                memory_id,
                accepted_turn,
            } if selected.contains(memory_id) => {
                destination.admit(target, *memory_id, *accepted_turn)?;
            }
            _ => {}
        }
    }
    if request.freshness == FreshHistoryFreshness::RetainHistory {
        for entry in entries {
            match entry {
                reliquary_freshness_store::FreshnessReplayEntry::Event(mut event) => {
                    event.effects.retain(|id, _| selected.contains(id));
                    if !event.effects.is_empty() {
                        destination.append_event(target, event)?;
                    }
                }
                reliquary_freshness_store::FreshnessReplayEntry::AcceptedUse(receipt, proof) => {
                    let sources = receipt
                        .sources
                        .into_iter()
                        .filter(|id| selected.contains(id))
                        .collect::<Vec<_>>();
                    if !sources.is_empty() {
                        destination.accept_use(
                            target,
                            receipt.use_id,
                            receipt.accepted_turn,
                            sources,
                            proof,
                        )?;
                    }
                }
                _ => {}
            }
        }
    } else {
        let records = source
            .records()?
            .into_iter()
            .filter(|(id, _)| selected.contains(id))
            .collect::<std::collections::BTreeMap<_, _>>();
        if !records.is_empty() {
            let settlement_cut = records
                .values()
                .flat_map(|record| [record.admitted_at_turn, record.accounted_turn])
                .flatten()
                .max()
                .unwrap_or(0);
            destination.append_historical_snapshot(
                target,
                reliquary_freshness_store::HistoricalFreshnessSnapshot {
                    owner_uuid: target.owner_id(),
                    import_id: format!("rewrite-{}", hex(&request.plan_digest)),
                    base_cut: 0,
                    settlement_cut,
                    journal_entries_included: 0,
                    evidence_digest: request.plan_digest,
                    records,
                },
            )?;
        }
    }
    Ok(())
}

fn copy_vectors(
    source_container: &Container,
    source: &CommonState,
    target: &mut Container,
    request: &FreshHistoryRequest,
) -> Result<(), String> {
    let profiles = &request.profile_ids;
    let retained_memories = &request.retained_memory_ids;
    if profiles.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return Err("selected embedding profiles are not canonical".into());
    }
    let mut retained_bodies = BTreeSet::new();
    for id in retained_memories {
        let history = source
            .memories
            .history(*id)
            .map_err(|error| error.to_string())?;
        for revision in selected_history_revisions(&history, request)? {
            retained_bodies.insert(revision.record.body_id.0);
        }
    }
    let selected = profiles.iter().map(|id| id.0).collect::<BTreeSet<_>>();
    for id in profiles {
        if source.profiles.get_active(*id).is_none() {
            return Err("selected embedding profile is unavailable or retired".into());
        }
    }
    let mut target_profiles =
        CompatibilityProfileStore::open(target).map_err(|error| error.to_string())?;
    for id in profiles {
        target_profiles
            .put(target, source.profiles.get_active(*id).unwrap().clone())
            .map_err(|error| error.to_string())?;
    }
    let mut target_packed = PackedVectorStore::open(target).map_err(|error| error.to_string())?;
    let mut target_mappings = MemoryVectorStore::open(target, &target_packed, &target_profiles)
        .map_err(|error| error.to_string())?;
    for info in source.memory_vectors.infos() {
        if !selected.contains(&info.compatibility_profile_id.0) {
            continue;
        }
        let record = source
            .memory_vectors
            .get(info.id)
            .map_err(|error| error.to_string())?;
        let mut bodies = Vec::new();
        let mut rows = Vec::new();
        for (index, body) in record.memory_body_ids.iter().enumerate() {
            if retained_bodies.contains(&body.0) {
                bodies.push(*body);
                rows.push(
                    source
                        .packed_vectors
                        .read_f32_row(record.packed_vector_id, index as u64)
                        .map_err(|error| error.to_string())?,
                );
            }
        }
        if bodies.is_empty() {
            continue;
        }
        let packed = target_packed
            .put_f32_rows(target, &rows)
            .map_err(|error| error.to_string())?;
        target_mappings
            .put(
                target,
                &target_packed,
                &target_profiles,
                reliquary_vector_store::AcceptedMemoryMapping {
                    profile: info.compatibility_profile_id,
                    packed,
                    packed_rows: rows.len() as u64,
                    body_ids: bodies,
                },
            )
            .map_err(|error| error.to_string())?;
    }
    if source_container.identity().file_kind == FileKind::Reliquary {
        copy_archive_vectors(
            source_container,
            source,
            target,
            profiles,
            &target_profiles,
            &mut target_packed,
        )?;
    }
    Ok(())
}

fn selected_history_revisions<'a>(
    history: &'a reliquary_memory_core::MemoryHistory,
    request: &FreshHistoryRequest,
) -> Result<Vec<&'a reliquary_memory_core::MemoryRevision>, String> {
    let revisions = if request.retain_all_revisions {
        history.revisions.iter().collect::<Vec<_>>()
    } else if let Some(cut) = request.retain_revisions_since_memory_version {
        history
            .revisions
            .iter()
            .filter(|revision| revision.record.memory_version >= cut)
            .collect::<Vec<_>>()
    } else {
        history.revisions.last().into_iter().collect::<Vec<_>>()
    };
    if revisions.is_empty() {
        return Err("selected Memory has no revision at the requested cut".into());
    }
    Ok(revisions)
}

fn copy_archive_vectors(
    source_container: &Container,
    source: &CommonState,
    target: &mut Container,
    profiles: &[CompatibilityProfileId],
    target_profiles: &CompatibilityProfileStore,
    target_packed: &mut PackedVectorStore,
) -> Result<(), String> {
    let source_mappings = ArchiveVectorStore::open(source_container, &source.packed_vectors)
        .map_err(|error| error.to_string())?;
    let source_generations =
        VectorGenerationStore::open(source_container, &source.profiles, &source_mappings)
            .map_err(|error| error.to_string())?;
    let target_archive = Archive::open(target).map_err(|error| error.to_string())?;
    let definitions = target_archive
        .fragment_definitions()
        .into_iter()
        .map(|definition| (definition.fragment.id, definition))
        .collect::<std::collections::BTreeMap<_, _>>();
    if definitions.is_empty() {
        return Ok(());
    }
    let mut target_mappings =
        ArchiveVectorStore::open(target, target_packed).map_err(|error| error.to_string())?;
    let mut target_generations =
        VectorGenerationStore::open(target, target_profiles, &target_mappings)
            .map_err(|error| error.to_string())?;
    for profile in profiles {
        let generation = source_generations
            .current_generation(*profile)
            .ok_or("selected embedding profile lacks retained Archive-vector coverage")?;
        let mapping = source_mappings
            .get(generation.archive_vector_id)
            .map_err(|error| error.to_string())?;
        let mut fragment_ids = Vec::new();
        let mut rows = Vec::new();
        let mut max_fragment_archive_version = 0u64;
        for (index, fragment) in mapping.fragment_ids.iter().enumerate() {
            let archive_id = reliquary_archive::FragmentId(fragment.0);
            let Some(definition) = definitions.get(&archive_id) else {
                continue;
            };
            let version = target_archive
                .node_archive_version(
                    &definition.fragment.conversation_id,
                    &definition.fragment.end_node_id,
                )
                .ok_or("retained Archive fragment has no source version")?;
            max_fragment_archive_version = max_fragment_archive_version.max(version);
            fragment_ids.push(*fragment);
            rows.push(
                source
                    .packed_vectors
                    .read_f32_row(mapping.packed_vector_id, index as u64)
                    .map_err(|error| error.to_string())?,
            );
        }
        if fragment_ids.len() != definitions.len() {
            return Err("selected embedding profile lacks retained Archive-vector coverage".into());
        }
        let packed = target_packed
            .put_f32_rows(target, &rows)
            .map_err(|error| error.to_string())?;
        let archive_vector = target_mappings
            .put(
                target,
                target_packed,
                reliquary_vector_store::AcceptedArchiveMapping {
                    packed_vector_id: packed,
                    packed_rows: rows.len() as u64,
                    fragment_ids,
                    max_fragment_archive_version,
                },
            )
            .map_err(|error| error.to_string())?;
        target_generations
            .publish(
                target,
                target_profiles,
                &target_mappings,
                *profile,
                archive_vector,
                target_archive.archive_version(),
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn copy_ego(source: &EgoStore, target: &mut Container, memory_version: u64) -> Result<(), String> {
    let mut ego = EgoStore::open(target).map_err(|error| error.to_string())?;
    for identity in source.identities() {
        ego.create_identity(
            target,
            identity.id,
            identity.name.clone(),
            identity.text.clone(),
        )
        .map_err(|error| error.to_string())?;
    }
    if let Some(personality) = source.personality() {
        ego.put_personality(target, 0, personality.text.clone(), memory_version)
            .map_err(|error| error.to_string())?;
    }
    if let Some(synthesis) = source.synthesis() {
        ego.put_synthesis(target, 0, memory_version, synthesis.text.clone())
            .map_err(|error| error.to_string())?;
    }
    for anchor in source.anchors() {
        ego.create_anchor(target, anchor.id, anchor.priority, anchor.text.clone())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

trait MemoryPages {
    fn page(&self, after: Option<MemoryId>) -> Result<Vec<MemoryId>, String>;
}
macro_rules! memory_pages {
    ($session:ty) => {
        impl MemoryPages for $session {
            fn page(&self, after: Option<MemoryId>) -> Result<Vec<MemoryId>, String> {
                self.memory_ids_page(after, 128)
                    .map(|(_, page)| page)
                    .map_err(|error| error.to_string())
            }
        }
    };
}
memory_pages!(RelSession);
memory_pages!(PhySession);

fn count_memories(owner: &impl MemoryPages) -> Result<usize, String> {
    let mut count = 0usize;
    let mut after = None;
    loop {
        let page = owner.page(after)?;
        if page.is_empty() {
            return Ok(count);
        }
        count = count
            .checked_add(page.len())
            .ok_or("Memory count overflow")?;
        after = page.last().copied();
    }
}

/// Read and authenticate the single rewrite lineage attached to an owner.
pub fn fresh_history_lineage(path: &Path) -> Result<Option<FreshHistoryLineage>, String> {
    let container = Container::open_read_only(path).map_err(|error| error.to_string())?;
    fresh_history_lineage_from_container(&container)
}

fn fresh_history_lineage_from_container(
    container: &Container,
) -> Result<Option<FreshHistoryLineage>, String> {
    let mut found = None;
    for frame in container.read_frames().map_err(|error| error.to_string())? {
        if !frame.payload.starts_with(LINEAGE_MAGIC) {
            continue;
        }
        if found.is_some() || frame.payload.len() < 48 {
            return Err("invalid or duplicate fresh-history lineage".into());
        }
        let length = usize::try_from(u64::from_le_bytes(frame.payload[8..16].try_into().unwrap()))
            .map_err(|_| "fresh-history lineage length overflow")?;
        if length.checked_add(48) != Some(frame.payload.len()) {
            return Err("invalid fresh-history lineage length".into());
        }
        let end = 16 + length;
        if Sha256::digest(&frame.payload[..end]).as_slice() != &frame.payload[end..] {
            return Err("fresh-history lineage checksum mismatch".into());
        }
        let lineage: FreshHistoryLineage =
            serde_json::from_slice(&frame.payload[16..end]).map_err(|error| error.to_string())?;
        if lineage.destination_owner_id != container.owner_id()
            || lineage.source_digest == [0; 32]
            || lineage.plan_digest == [0; 32]
            || lineage.predecessor_generation == [0; 32]
            || lineage.history_generation == [0; 32]
        {
            return Err("fresh-history lineage does not match owner".into());
        }
        found = Some(lineage);
    }
    Ok(found)
}

/// Return the logical history generation used to fence reconciliation.
/// Owners without rewrite lineage share a stable genesis derived from owner
/// identity and kind; a rewrite always records and advances that generation.
pub fn owner_history_generation(path: &Path) -> Result<[u8; 32], String> {
    let container = Container::open_read_only(path).map_err(|error| error.to_string())?;
    owner_history_generation_from_container(&container)
}

pub(super) fn owner_history_generation_from_container(
    container: &Container,
) -> Result<[u8; 32], String> {
    if let Some(lineage) = fresh_history_lineage_from_container(container)? {
        return Ok(lineage.history_generation);
    }
    let identity = container.identity();
    let owner_id = container.owner_id();
    let kind = match identity.file_kind {
        FileKind::Reliquary => 1u8,
        FileKind::Phylactery => 2u8,
    };
    let scope = match identity.scope {
        reliquary_container::ReliquaryScope::Unspecified => 0u8,
        reliquary_container::ReliquaryScope::Organization => 1u8,
        reliquary_container::ReliquaryScope::Project => 2u8,
        reliquary_container::ReliquaryScope::Connection => 3u8,
    };
    Ok(Sha256::digest(
        [
            b"reliquary-history-genesis-v1\0".as_slice(),
            owner_id.as_slice(),
            &[kind, scope],
        ]
        .concat(),
    )
    .into())
}

fn draft_from(
    revision: &reliquary_memory_core::MemoryRevision,
    body: &reliquary_memory_core::MemoryBody,
) -> MemoryDraft {
    let record = &revision.record;
    MemoryDraft {
        title: body.title.clone(),
        content: body.content.clone(),
        category: record.category.clone(),
        memory_type: record.memory_type.clone(),
        authority_kind: record.authority_kind.clone(),
        temporal_status: record.temporal_status.clone(),
        scope: record.scope.clone(),
        lifecycle_state: record.lifecycle_state.clone(),
        archived: record.archived,
        parent_id: record.parent_id,
        superseded_by: record.superseded_by,
        source_node_id: record.source_node_id.clone(),
        content_source_conversation_id: record.content_source_conversation_id.clone(),
        content_source_node_id: record.content_source_node_id.clone(),
        grounding_source_conversation_id: record.grounding_source_conversation_id.clone(),
        grounding_source_node_id: record.grounding_source_node_id.clone(),
        source_episode_id: record.source_episode_id,
        source_ref: record.source_ref.clone(),
        source_time_ns: record.source_time_ns,
        temporal_inference: record.temporal_inference.clone(),
        mutation_id: record.mutation_id.clone(),
        created_at_ns: record.created_at_ns,
        updated_at_ns: record.updated_at_ns,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
