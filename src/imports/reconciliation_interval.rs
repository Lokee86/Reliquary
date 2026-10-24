//! Ordered owner replay within an accepted import interval.
use super::reconciliation::unsupported;
use crate::owner_lifecycle::ReliquaryOwner;
use crate::{CvaError, CvaReconcileError, FreshnessEventProof};
use std::collections::BTreeMap;

pub(super) fn physical_objects(
    owner: &ReliquaryOwner,
) -> Result<Vec<crate::container::ObjectRef>, CvaReconcileError> {
    let source = crate::owner_lifecycle::HistoryReadAccess::new(&owner.container)
        .snapshot()
        .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?;
    let mut cursor = source
        .cursor()
        .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?;
    let mut objects = Vec::new();
    while let Some(frame) = cursor
        .next_frame()
        .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?
    {
        objects.push(frame.object);
    }
    Ok(objects)
}

#[derive(Default)]
pub(super) struct Coordinates {
    resolutions:
        BTreeMap<crate::ObjectRef, crate::entity_resolution_codec::DecodedEntityResolution>,
    pub memory_revisions: usize,
    pub duplicate_memory_revisions: usize,
    pub entity_revisions: usize,
    pub duplicate_entity_revisions: usize,
    pub relationship_revisions: usize,
    pub duplicate_relationship_revisions: usize,
    pub file_memory_links: usize,
    pub graph_transactions: usize,
    pub graph_mutations: usize,
    pub duplicate_graph_mutations: usize,
    graph: BTreeMap<u64, u64>,
    memory_graph: BTreeMap<u64, u64>,
    community: BTreeMap<u64, u64>,
}
impl Coordinates {
    pub(super) fn shared(
        source: &mut ReliquaryOwner,
        end: usize,
    ) -> Result<Self, CvaReconcileError> {
        let mut result = Self::default();
        result.resolutions = source
            .history_read_access()
            .materialized_resolution_changes()?;
        result.graph.insert(0, 0);
        result.memory_graph.insert(0, 0);
        for chunk in physical_objects(source)?.into_iter().take(end) {
            let payload = source
                .history_read_access()
                .snapshot()
                .map_err(|e| CvaError::Container(e.0))?
                .read_object(chunk)
                .map_err(|e| CvaError::Container(e.0))?;
            if let Some(v) = crate::history::import_replay_read::graph_version(chunk, &payload)? {
                result.graph.insert(v.graph_version, v.graph_version);
                let mutation = source
                    .history_read_access()
                    .snapshot()
                    .map_err(|e| CvaError::Container(e.0))?
                    .read_object(v.mutation)
                    .map_err(|e| CvaError::Container(e.0))?;
                let changes = source
                    .graph
                    .decode_stored_mutations(&mutation)?
                    .ok_or(CvaReconcileError::InvalidGraphVersionRecord)?;
                if changes.iter().any(|change| {
                    matches!(change.kind, crate::SemanticGraphRelationKind::Memory(_))
                }) {
                    result.memory_graph.insert(v.graph_version, v.graph_version);
                }
            }
            if let Some(s) = crate::history::import_replay_read::community_snapshot(&payload)
                .map_err(CvaError::from)?
            {
                result.community.insert(s.generation, s.generation);
            }
        }
        Ok(result)
    }
    pub(super) fn retained_epoch(
        &mut self,
        source: &mut ReliquaryOwner,
        destination: &mut ReliquaryOwner,
        source_epoch: &super::reconciliation::CompletedEpoch,
        destination_epoch: &super::reconciliation::CompletedEpoch,
    ) -> Result<(), CvaReconcileError> {
        fn cuts(
            cva: &mut ReliquaryOwner,
            first: usize,
            end: usize,
        ) -> Result<(Vec<(u64, bool)>, Vec<u64>), CvaReconcileError> {
            let mut graphs = Vec::new();
            let mut communities = Vec::new();
            for chunk in physical_objects(cva)?.into_iter().take(end).skip(first) {
                let payload = cva
                    .history_read_access()
                    .snapshot()
                    .map_err(|e| CvaError::Container(e.0))?
                    .read_object(chunk)
                    .map_err(|e| CvaError::Container(e.0))?;
                if let Some(version) =
                    crate::history::import_replay_read::graph_version(chunk, &payload)?
                {
                    let raw = cva
                        .history_read_access()
                        .snapshot()
                        .map_err(|e| CvaError::Container(e.0))?
                        .read_object(version.mutation)
                        .map_err(|e| CvaError::Container(e.0))?;
                    let changes = cva
                        .graph
                        .decode_stored_mutations(&raw)?
                        .ok_or(CvaReconcileError::InvalidGraphVersionRecord)?;
                    graphs.push((
                        version.graph_version,
                        changes
                            .iter()
                            .any(|c| matches!(c.kind, crate::SemanticGraphRelationKind::Memory(_))),
                    ));
                }
                if let Some(snapshot) =
                    crate::history::import_replay_read::community_snapshot(&payload)
                        .map_err(CvaError::from)?
                {
                    communities.push(snapshot.generation);
                }
            }
            Ok((graphs, communities))
        }
        let (source_graphs, source_communities) =
            cuts(source, source_epoch.first, source_epoch.end)?;
        let (destination_graphs, destination_communities) =
            cuts(destination, destination_epoch.first, destination_epoch.end)?;
        if source_graphs.len() != destination_graphs.len()
            || source_communities.len() != destination_communities.len()
        {
            return Err(unsupported("retained import owner boundaries differ"));
        }
        for ((from, memory), (to, destination_memory)) in
            source_graphs.into_iter().zip(destination_graphs)
        {
            if memory != destination_memory {
                return Err(unsupported("retained import graph ownership differs"));
            }
            self.graph.insert(from, to);
            if memory {
                self.memory_graph.insert(from, to);
            }
        }
        self.community
            .extend(source_communities.into_iter().zip(destination_communities));
        Ok(())
    }
    fn map(map: &BTreeMap<u64, u64>, cut: u64) -> Result<u64, CvaReconcileError> {
        map.get(&cut)
            .copied()
            .ok_or_else(|| unsupported("accepted owner cut has no destination mapping"))
    }
    pub(super) fn proof(
        &self,
        proof: FreshnessEventProof,
    ) -> Result<FreshnessEventProof, CvaReconcileError> {
        Ok(FreshnessEventProof {
            graph_version: Self::map(&self.memory_graph, proof.graph_version)?,
            community_generation: proof
                .community_generation
                .map(|g| Self::map(&self.community, g))
                .transpose()?,
            community_graph_version: proof
                .community_graph_version
                .map(|g| Self::map(&self.memory_graph, g))
                .transpose()?,
        })
    }
}

pub(super) fn turn(
    source: &ReliquaryOwner,
    output: &ReliquaryOwner,
    cut: u64,
) -> Result<u64, String> {
    if cut == 0 {
        return Ok(0);
    }
    let node = source
        .archive
        .nodes
        .iter()
        .find(|node| {
            source
                .archive
                .activity_position_for_turn(&node.conversation_id, &node.id)
                == Some(cut)
        })
        .ok_or("source cut has no accepted activity")?;
    output
        .archive
        .activity_position_for_turn(&node.conversation_id, &node.id)
        .ok_or_else(|| "destination lacks accepted activity".into())
}

pub(super) fn replay_frame(
    source: &mut ReliquaryOwner,
    output: &mut ReliquaryOwner,
    index: usize,
    payload: &[u8],
    coordinates: &mut Coordinates,
) -> Result<usize, CvaReconcileError> {
    if let Some(bytes) = crate::perception_import_replay::retained_resolution_carrier(payload)? {
        crate::owner_lifecycle::import_epoch_transport::append_replay_payload(
            &mut output.container,
            &bytes,
        )?;
        return Ok(0);
    }
    let archive =
        crate::history::import_replay_read::ImportReplayRead::new(source.history_read_access())
            .archive_range(&source.archive, &source.project_files, index, index + 1)?;
    crate::owner_lifecycle::archive_ordered_replay::replay_archive_tail(
        crate::owner_lifecycle::archive_ordered_replay::ArchiveReplayDestination {
            container: &mut output.container,
            archive: &mut output.archive,
            project_files: &mut output.project_files,
        },
        &archive,
    )?;
    coordinates.file_memory_links +=
        crate::owner_lifecycle::archive_ordered_replay::replay_file_memory_links(
            &mut output.container,
            &mut output.archive,
            &output.memories,
            &archive.file_memory_links,
        )?;
    if let Some(mut completion) =
        crate::owner_lifecycle::insomnia_import_replay::decode_completion(payload)
            .map_err(CvaReconcileError::InvalidInsomniaCompletion)?
    {
        source
            .history_read_access()
            .snapshot()
            .map_err(|e| CvaError::Container(e.0))?
            .hydrate_grouped_completion(&source.memories, &mut completion)
            .map_err(crate::owner_lifecycle::OwnerError::from)?;
        for id in &completion.memory_ids {
            if completion.records.iter().any(|record| record.id == *id) {
                continue;
            }
            let mut incoming =
                source
                    .history_read_access()
                    .memory_revision(&source.memories, *id, 1)?;
            let mut existing =
                output
                    .history_read_access()
                    .memory_revision(&output.memories, *id, 1)?;
            incoming.global_version = 0;
            incoming.memory_version = 0;
            existing.global_version = 0;
            existing.memory_version = 0;
            if incoming != existing {
                return Err(unsupported(
                    "reused grouped Memory has conflicting first publication",
                ));
            }
        }
        let record_count = completion.records.len();
        coordinates.memory_revisions += record_count;
        return crate::owner_lifecycle::grouped_import_replay::replay_grouped_import_completion(
            crate::owner_lifecycle::grouped_import_replay::GroupedImportDestination {
                container: &mut output.container,
                archive: &output.archive,
                memories: &mut output.memories,
                insomnia: &mut output.insomnia,
                freshness: &mut output.freshness,
                dream_freshness: &mut output.dream_freshness,
            },
            completion,
        )
        .map(usize::from)
        .map_err(Into::into);
    }
    if crate::memory_compact_codec::publication::coordinates(payload)?.is_some() {
        let memory =
            crate::history::import_replay_read::ImportReplayRead::new(source.history_read_access())
                .memory_range(&source.memories, index, index + 1)?;
        let result = crate::memory_ordered_replay::replay_memory_tail(
            crate::memory_ordered_replay::MemoryReplayDestination {
                archive: &output.archive,
                memories: &mut output.memories,
                insomnia: &mut output.insomnia,
                container: &mut output.container,
            },
            memory,
        )?;
        coordinates.memory_revisions += result.revisions;
        coordinates.duplicate_memory_revisions += result.duplicate_revisions;
    }
    if let Some(metadata) = crate::memory_routing_codec::decode(payload)? {
        output
            .memories
            .put_routing_metadata(&mut output.container, metadata)?;
    }
    let entities =
        crate::history::import_replay_read::ImportReplayRead::new(source.history_read_access())
            .entity_range(&source.entities, index, index + 1)?;
    let result = crate::owner_lifecycle::import_epoch_transport::replay_entity_tail(
        &mut output.container,
        &mut output.entities,
        entities,
    )?;
    coordinates.entity_revisions += result.revisions;
    coordinates.duplicate_entity_revisions += result.duplicate_revisions;
    let relationships =
        crate::history::import_replay_read::ImportReplayRead::new(source.history_read_access())
            .relationship_range(&source.relationships, index, index + 1)?;
    let result = crate::owner_lifecycle::import_epoch_transport::replay_relationship_tail(
        &mut output.container,
        &mut output.relationships,
        relationships,
    )?;
    coordinates.relationship_revisions += result.revisions;
    coordinates.duplicate_relationship_revisions += result.duplicate_revisions;
    if let Some((_, graph_version)) =
        crate::history::import_replay_read::graph_coordinates(payload)?
    {
        let graph = read_graph_range(source, index, index + 1)?;
        let preserve_import_postimages = output.import_epochs.open().is_some();
        let result = crate::owner_lifecycle::graph_ordered_replay::replay_with_skips(
            crate::owner_lifecycle::graph_ordered_replay::GraphReplayDestination {
                container: &mut output.container,
                memories: &output.memories,
                entities: &output.entities,
                graph: &mut output.graph,
            },
            &graph,
            &std::collections::HashMap::new(),
            preserve_import_postimages,
        )?;
        coordinates.graph_transactions += result.transactions;
        coordinates.graph_mutations += result.mutations;
        coordinates.duplicate_graph_mutations += result.duplicate_mutations;
        coordinates
            .graph
            .insert(graph_version, output.graph.graph_version());
        if result.source_memory_graph_changed {
            coordinates
                .memory_graph
                .insert(graph_version, output.graph.memory_graph_version());
        }
    }
    if let Some(mut snapshot) =
        crate::history::import_replay_read::community_snapshot(payload).map_err(CvaError::from)?
    {
        let original = snapshot.generation;
        snapshot.derived_graph_version =
            Coordinates::map(&coordinates.memory_graph, snapshot.derived_graph_version)?;
        snapshot.generation = output
            .communities
            .next_generation()
            .map_err(CvaError::from)?;
        let owner = output
            .owner_uuid()
            .ok_or_else(|| unsupported("Community replay lacks owner"))?;
        let generation = snapshot.generation;
        output
            .communities
            .publish(&mut output.container, &output.graph, owner, snapshot)
            .map_err(CvaError::from)?;
        coordinates.community.insert(original, generation);
    }
    let source_facts = crate::history::freshness_reconciliation::FreshnessReplaySource {
        owner_uuid: source.owner_uuid(),
        archive: &source.archive,
        freshness: &source.freshness,
        dream_freshness: &source.dream_freshness,
        graph: &source.graph,
        communities: &source.communities,
    };
    let entries = crate::history::freshness_reconciliation::normalize_replay_payload(
        &source_facts,
        &output.archive,
        payload,
    )?;
    for rebased in entries {
        let mut entry = rebased.entry;
        if let crate::freshness_storage::FreshnessReplayEntry::Event { proof, .. } = &mut entry {
            *proof = coordinates.proof(*proof)?;
        }
        let owner_uuid = output.owner_uuid();
        if let Some(outcome) = crate::owner_lifecycle::freshness_reconciliation::replay_entry(
            &mut output.container,
            &output.archive,
            &output.memories,
            owner_uuid,
            &mut output.freshness,
            entry,
            rebased.destination_turn,
        )? {
            // The native owner returned by reconciliation is reopened before it
            // reaches the facade, which rebuilds the disposable due projection.
            let _ = outcome;
        }
    }
    if payload.starts_with(b"CVDREAM1") || payload.starts_with(b"CVDREAM2") {
        let chunk = *physical_objects(source)?
            .get(index)
            .ok_or_else(|| unsupported("Dream source physical frame is absent"))?;
        if let Some(mut pass) = source
            .history_read_access()
            .snapshot()
            .map_err(|e| CvaError::Container(e.0))?
            .materialized_dream_begin(&source.archive, &source.memories, chunk, payload)?
        {
            pass.turn = turn(source, output, pass.turn).map_err(CvaError::Freshness)?;
            pass.graph_version_before =
                Coordinates::map(&coordinates.graph, pass.graph_version_before)?;
            if !output
                .dream_freshness
                .begin_is_retry(&pass)
                .map_err(CvaError::Freshness)?
            {
                let owner = output
                    .owner_uuid()
                    .ok_or_else(|| unsupported("Dream destination lacks owner"))?;
                let current =
                    crate::owner_lifecycle::dream_begin_transport::current_dream_begin_payload(
                        &output.container,
                        &output.archive,
                        &output.memories,
                        &pass,
                    )?;
                crate::owner_lifecycle::import_epoch_transport::append_replay_payload(
                    &mut output.container,
                    &current,
                )?;
                output
                    .dream_freshness
                    .ingest_materialized_begin(owner, pass)
                    .map_err(CvaError::Freshness)?;
            }
        } else {
            let rebased = crate::dream_replay_transport::rebase_payload(
                &output.dream_freshness,
                payload,
                |cut| turn(source, output, cut),
                |cut| Coordinates::map(&coordinates.graph, cut).map_err(|e| e.to_string()),
                |proof| coordinates.proof(proof).map_err(|e| e.to_string()),
            )
            .map_err(CvaError::Freshness)?;
            if !rebased.is_empty() {
                crate::owner_lifecycle::import_epoch_transport::append_replay_payload(
                    &mut output.container,
                    &rebased,
                )?;
                output
                    .dream_freshness
                    .ingest(&rebased)
                    .map_err(CvaError::Freshness)?;
            }
        }
    }

    if payload.starts_with(b"CVAPEP01") {
        let mut accepted = crate::processing_epoch_store::ProcessingEpochStore::default();
        accepted.ingest(payload)?;
        accepted.validate(&output.memories)?;
        for (memory, lane, state) in accepted.records() {
            output
                .processing_epochs
                .put(&mut output.container, memory, lane, state)?;
        }
    }
    // These records have no container coordinates. Preserve retries,
    // failures and lease transitions in their accepted physical order.
    match crate::owner_lifecycle::insomnia_import_replay::replay_record(
        &mut output.container,
        &mut output.insomnia,
        payload,
    ) {
        Ok(_) => {}
        Err(crate::owner_lifecycle::insomnia_import_replay::ReplayError::Decode(error)) => {
            return Err(error.into());
        }
        Err(crate::owner_lifecycle::insomnia_import_replay::ReplayError::Append(error)) => {
            return Err(error.into());
        }
    }
    if crate::perception_import_replay::is_resolution_delta(payload) {
        let object = *physical_objects(source)?
            .get(index)
            .ok_or_else(|| unsupported("resolution source frame unavailable"))?;
        // Tests/direct callers may construct empty Coordinates; resolve against
        // the actual source prefix, never the destination's inherited evidence.
        if !coordinates.resolutions.contains_key(&object) {
            coordinates.resolutions = source
                .history_read_access()
                .materialized_resolution_changes()?;
        }
        let change = coordinates
            .resolutions
            .get(&object)
            .cloned()
            .ok_or_else(|| unsupported("resolution source postimage unavailable"))?;
        output
            .entity_resolutions
            .persist_change(&mut output.container, change)?;
    }
    Ok(0)
}

pub(super) fn read_graph_range(
    source: &ReliquaryOwner,
    start_chunk: usize,
    end_chunk: usize,
) -> Result<crate::graph_ordered_replay::GraphTail, CvaReconcileError> {
    crate::history::import_replay_read::ImportReplayRead::new(source.history_read_access())
        .graph_range(&source.graph, start_chunk, end_chunk)
}
