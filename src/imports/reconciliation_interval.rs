//! Ordered owner replay within an accepted import interval.
use super::reconciliation::unsupported;
use crate::{Cva, CvaError, CvaReconcileError, FreshnessEventProof};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Coordinates {
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
    pub(super) fn shared(source: &mut Cva, end: usize) -> Result<Self, CvaReconcileError> {
        let mut result = Self::default();
        result.graph.insert(0, 0);
        result.memory_graph.insert(0, 0);
        for chunk in source.container.chunks()?.into_iter().take(end) {
            let payload = source.container.read(chunk)?;
            if let Some(v) = crate::graph_codec::decode_version(&payload)? {
                result.graph.insert(v.graph_version, v.graph_version);
                let mutation = source.container.read(v.mutation)?;
                let changes = if let Some(batch) = crate::graph_codec::decode_batch(&mutation)? {
                    batch
                } else if let Some(change) = crate::graph_codec::decode_mutation(&mutation)? {
                    vec![change]
                } else {
                    return Err(CvaReconcileError::InvalidGraphVersionRecord);
                };
                if changes.iter().any(|change| {
                    matches!(change.kind, crate::SemanticGraphRelationKind::Memory(_))
                }) {
                    result.memory_graph.insert(v.graph_version, v.graph_version);
                }
            }
            if let Some(s) =
                crate::community_codec::decode_snapshot(&payload).map_err(CvaError::from)?
            {
                result.community.insert(s.generation, s.generation);
            }
        }
        Ok(result)
    }
    pub(super) fn retained_epoch(
        &mut self,
        source: &mut Cva,
        destination: &mut Cva,
        source_epoch: &super::reconciliation::CompletedEpoch,
        destination_epoch: &super::reconciliation::CompletedEpoch,
    ) -> Result<(), CvaReconcileError> {
        fn cuts(
            cva: &mut Cva,
            first: usize,
            end: usize,
        ) -> Result<(Vec<(u64, bool)>, Vec<u64>), CvaReconcileError> {
            let mut graphs = Vec::new();
            let mut communities = Vec::new();
            for chunk in cva.container.chunks()?.into_iter().take(end).skip(first) {
                let payload = cva.container.read(chunk)?;
                if let Some(version) = crate::graph_codec::decode_version(&payload)? {
                    let raw = cva.container.read(version.mutation)?;
                    let changes = match crate::graph_codec::decode_batch(&raw)? {
                        Some(batch) => batch,
                        None => vec![
                            crate::graph_codec::decode_mutation(&raw)?
                                .ok_or(CvaReconcileError::InvalidGraphVersionRecord)?,
                        ],
                    };
                    graphs.push((
                        version.graph_version,
                        changes
                            .iter()
                            .any(|c| matches!(c.kind, crate::SemanticGraphRelationKind::Memory(_))),
                    ));
                }
                if let Some(snapshot) =
                    crate::community_codec::decode_snapshot(&payload).map_err(CvaError::from)?
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

pub(super) fn turn(source: &Cva, output: &Cva, cut: u64) -> Result<u64, String> {
    if cut == 0 {
        return Ok(0);
    }
    let node = source
        .archive
        .nodes
        .iter()
        .find(|node| {
            source.activity_position_for_turn(&node.conversation_id, &node.id) == Some(cut)
        })
        .ok_or("source cut has no accepted activity")?;
    output
        .activity_position_for_turn(&node.conversation_id, &node.id)
        .ok_or_else(|| "destination lacks accepted activity".into())
}

pub(super) fn replay_frame(
    source: &mut Cva,
    output: &mut Cva,
    index: usize,
    payload: &[u8],
    coordinates: &mut Coordinates,
) -> Result<usize, CvaReconcileError> {
    let archive = crate::cva_reconcile_archive::read_archive_range(source, index, index + 1)?;
    crate::cva_reconcile_archive::replay_archive_tail(output, &archive)?;
    coordinates.file_memory_links +=
        crate::cva_reconcile_archive::replay_file_memory_links(output, &archive.file_memory_links)?;
    if let Some(mut completion) = crate::insomnia::completion::decode_completion(payload)
        .map_err(CvaReconcileError::InvalidInsomniaCompletion)?
    {
        source
            .hydrate_grouped_import_completion_bodies(&mut completion)
            .map_err(CvaReconcileError::from)?;
        for id in &completion.memory_ids {
            if completion.records.iter().any(|record| record.id == *id) {
                continue;
            }
            let mut incoming = source.memory_revision(*id, 1)?;
            let mut existing = output.memory_revision(*id, 1)?;
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
        return output
            .replay_grouped_import_completion(completion)
            .map(usize::from)
            .map_err(Into::into);
    }
    if crate::memory_codec::decode_version(payload)?.is_some() {
        let memory = crate::cva_reconcile_memory::read_memory_range(source, index, index + 1)?;
        let result = crate::cva_reconcile_memory::replay_memory_tail(output, memory)?;
        coordinates.memory_revisions += result.revisions;
        coordinates.duplicate_memory_revisions += result.duplicate_revisions;
    }
    if let Some(metadata) = crate::memory_routing_codec::decode(payload)? {
        output
            .memories
            .put_routing_metadata(&mut output.container, metadata)?;
    }
    let entities = crate::cva_reconcile_entity::read_entity_range(source, index, index + 1)?;
    let result = crate::cva_reconcile_entity::replay_entity_tail(output, entities)?;
    coordinates.entity_revisions += result.revisions;
    coordinates.duplicate_entity_revisions += result.duplicate_revisions;
    let relationships =
        crate::cva_reconcile_relationship::read_relationship_range(source, index, index + 1)?;
    let result =
        crate::cva_reconcile_relationship::replay_relationship_tail(output, relationships)?;
    coordinates.relationship_revisions += result.revisions;
    coordinates.duplicate_relationship_revisions += result.duplicate_revisions;
    if let Some(version) = crate::graph_codec::decode_version(payload)? {
        let graph = crate::cva_reconcile_graph::read_graph_range(source, index, index + 1)?;
        let result = crate::cva_reconcile_graph::replay_graph_tail(output, &graph)?;
        coordinates.graph_transactions += result.transactions;
        coordinates.graph_mutations += result.mutations;
        coordinates.duplicate_graph_mutations += result.duplicate_mutations;
        coordinates
            .graph
            .insert(version.graph_version, output.graph_version());
        if result.source_memory_graph_changed {
            coordinates
                .memory_graph
                .insert(version.graph_version, output.memory_graph_version());
        }
    }
    if let Some(mut snapshot) =
        crate::community_codec::decode_snapshot(payload).map_err(CvaError::from)?
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
    crate::cva_reconcile_freshness::replay_freshness_payload(source, output, payload, |p| {
        coordinates.proof(p)
    })?;
    if payload.starts_with(b"CVDREAM1") {
        let rebased = output
            .dream_freshness
            .replay_payload(
                payload,
                |cut| turn(source, output, cut),
                |cut| Coordinates::map(&coordinates.graph, cut).map_err(|e| e.to_string()),
                |p| coordinates.proof(p).map_err(|e| e.to_string()),
            )
            .map_err(CvaError::Freshness)?;
        if !rebased.is_empty() {
            output.container.append(&rebased)?;
            output
                .dream_freshness
                .ingest(&rebased)
                .map_err(CvaError::Freshness)?;
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
    if let Some(record) = crate::insomnia::codec::decode_record(payload)? {
        // These records have no container coordinates. Preserve retries,
        // failures and lease transitions in their accepted physical order.
        match record {
            crate::insomnia::codec::InsomniaRecord::Work(work) => {
                output.container.append(payload)?;
                output.insomnia.apply_work(work);
            }
            crate::insomnia::codec::InsomniaRecord::Attempt(attempt) => {
                output.container.append(payload)?;
                output.insomnia.apply_attempt(attempt);
            }
        }
    }
    if crate::entity_resolution_codec::decode_resolution(payload)?.is_some() {
        let mut staged = output.entity_resolutions.clone();
        staged.ingest(payload)?;
        output.container.append(payload)?;
        output.entity_resolutions = staged;
    }
    Ok(0)
}
