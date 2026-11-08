//! Owner-owned atomic publication of a bounded, terminal Dream frontier.
use super::dream_batch_model::*;
use super::*;
use crate::records::dream_batch::{self, DreamBatchRecord, Lifecycle, Observation, Pair};
fn prepare(
    common: &CommonState,
    owner: [u8; 16],
    cut: &DreamBatchCut,
) -> Result<Option<DreamBatchRecord>, OwnerSessionError> {
    if cut.owner != owner {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    let digest = dream_batch::fingerprint(cut).map_err(OwnerSessionError::Record)?;
    if let Some(prior) = common.dream_pairs.batch(cut.publication_id) {
        return if prior.fingerprint == digest {
            Ok(None)
        } else {
            Err(OwnerSessionError::StaleDreamCut)
        };
    }
    if cut.observed.is_empty()
        || cut.observed.len() > 128
        || (cut.pairs.is_empty() && cut.changes.is_empty() && cut.lifecycles.is_empty())
        || cut.pairs.len() > 16
        || cut.changes.len() > 256
        || cut.lifecycles.len() > 128
        || cut.graph_version != common.graph.graph_version()
    {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    for observed in &cut.observed {
        let current = common
            .memories
            .current(observed.id)
            .map_err(|_| OwnerSessionError::StaleDreamCut)?;
        if current.record.revision != observed.revision
            || current.record.body_id != observed.body_id
            || !matches!(
                current.payload,
                reliquary_memory_core::PayloadState::Available(_)
            )
        {
            return Err(OwnerSessionError::StaleDreamCut);
        }
    }
    for pair in &cut.pairs {
        if common.dream_pairs.contains(pair.source, pair.candidate) {
            return Err(OwnerSessionError::StaleDreamCut);
        }
        for id in [pair.source, pair.candidate] {
            if common.memories.current(id)?.record.archived
                && !(id == pair.candidate && pair.rejected && pair.kind.is_none())
            {
                return Err(OwnerSessionError::StaleDreamCut);
            }
        }
    }
    let admission = GraphAdmission::for_changes(owner, cut.observed.iter().map(|o| o.id.0), []);
    let graph_records = common
        .graph
        .prepare_dream_changes(&admission, &cut.changes)?;
    let mut graph = common.graph.clone();
    for bytes in &graph_records {
        graph.apply_dream_relation(
            &admission,
            reliquary_graph::dream_relation_in_frame(bytes, owner)?,
        )?;
    }
    let mut memories = common.memories.clone();
    let mut lifecycles = Vec::new();
    for intent in &cut.lifecycles {
        let observed = cut
            .observed
            .iter()
            .find(|o| o.id == intent.memory_id)
            .ok_or(OwnerSessionError::StaleDreamCut)?;
        let mut event = reliquary_memory_core::GroupedMemoryLifecycle {
            id: intent.memory_id,
            previous_revision: observed.revision,
            state: intent.lifecycle_state.clone(),
            archived: intent.archived,
            superseded_by: intent.superseded_by,
            mutation_id: format!(
                "dream-batch:{}:{}",
                hex(&cut.publication_id),
                hex(&intent.memory_id.0)
            ),
            updated_at_ns: cut.updated_at_ns,
            delta: vec![],
        };
        event.delta = memories.prepare_dream_lifecycle(&event)?;
        memories.apply_dream_lifecycle(&event)?;
        lifecycles.push(Lifecycle {
            id: event.id.0,
            revision: event.previous_revision,
            state: event.state,
            archived: event.archived,
            superseded_by: event.superseded_by.map(|id| id.0),
            mutation: event.mutation_id,
            updated_at_ns: event.updated_at_ns,
            delta: event.delta,
        });
    }
    let mut pairs = Vec::new();
    for p in &cut.pairs {
        let mut pair = Pair {
            source: p.source.0,
            candidate: p.candidate.0,
            kind: dream_batch::kind(p.kind).map_err(OwnerSessionError::Record)?,
            rejected: p.rejected,
            direct: false,
        };
        pair.direct = dream_batch::direct::direct(&common.graph, &graph, &pair)
            .map_err(OwnerSessionError::Record)?;
        pairs.push(pair);
    }
    let record = DreamBatchRecord {
        owner,
        publication_id: cut.publication_id,
        fingerprint: digest,
        graph_version: cut.graph_version,
        memory_version: common.memories.memory_version(),
        observed: cut
            .observed
            .iter()
            .copied()
            .map(Observation::from)
            .collect(),
        pairs,
        graph_records,
        lifecycles,
        updated_at_ns: cut.updated_at_ns,
    };
    record.encode().map_err(OwnerSessionError::Record)?;
    Ok(Some(record))
}
fn apply(common: &mut CommonState, record: DreamBatchRecord) -> Result<(), OwnerSessionError> {
    for l in &record.lifecycles {
        common.memories.apply_dream_lifecycle(&l.event())?;
    }
    let admission =
        GraphAdmission::for_changes(record.owner, record.observed.iter().map(|o| o.id), []);
    for bytes in &record.graph_records {
        common.graph.apply_dream_relation(
            &admission,
            reliquary_graph::dream_relation_in_frame(bytes, record.owner)?,
        )?;
    }
    common.refresh_dream_index(
        &record
            .lifecycles
            .iter()
            .map(|l| MemoryId(l.id))
            .collect::<Vec<_>>(),
    )?;
    common.dream_pairs.apply_batch(record)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
impl RelWriter {
    pub fn commit_dream_batch(&mut self, cut: DreamBatchCut) -> Result<bool, OwnerSessionError> {
        self.ensure_write_ready()?;
        self.commit_dream_batch_after_fence(cut)
    }

    pub fn commit_native_import_dream_batch(
        &mut self,
        import_id: &str,
        pass_id: &str,
        cut: DreamBatchCut,
    ) -> Result<bool, OwnerSessionError> {
        let frontier = self
            .initial_dream_frontier(pass_id)
            .ok_or(OwnerSessionError::StaleDreamCut)?;
        self.validate_native_import_memory(import_id, frontier.source.id)?;
        let expected = frontier
            .candidates
            .iter()
            .map(|c| c.id)
            .collect::<std::collections::BTreeSet<_>>();
        let evaluated = cut
            .pairs
            .iter()
            .map(|p| p.candidate)
            .collect::<std::collections::BTreeSet<_>>();
        let (expected_pass, publication) =
            Self::native_import_dream_identity(import_id, frontier.source.id);
        if pass_id != expected_pass
            || cut.publication_id != publication
            || cut.graph_version != frontier.graph_version
            || cut.observed.first() != Some(&frontier.source)
            || frontier
                .candidates
                .iter()
                .any(|c| !cut.observed.contains(c))
            || cut.pairs.iter().any(|p| p.source != frontier.source.id)
            || evaluated != expected
            || cut.pairs.len() != expected.len()
        {
            return Err(OwnerSessionError::StaleDreamCut);
        }
        if !self.dream_batch_published(publication) {
            let seeds: Vec<_> = std::iter::once(frontier.source.id.0)
                .chain(frontier.candidates.iter().map(|c| c.id.0))
                .collect();
            let (allowed, _) = self.session.common.graph.duplicate_context(&seeds, 128)?;
            if cut
                .observed
                .iter()
                .any(|observation| !allowed.contains(&observation.id.0))
            {
                return Err(OwnerSessionError::StaleDreamCut);
            }
        }
        self.commit_dream_batch_after_fence(cut)
    }

    fn commit_dream_batch_after_fence(
        &mut self,
        cut: DreamBatchCut,
    ) -> Result<bool, OwnerSessionError> {
        if self.container.writer_lease_proof().is_none()
            || self.container.writer_lease_proof() != self.session.writer_lease_proof
        {
            return Err(OwnerSessionError::StaleDreamCut);
        }
        let Some(record) = prepare(&self.session.common, self.owner_id(), &cut)? else {
            return Ok(false);
        };
        let result = (|| {
            self.container
                .append(&record.encode().map_err(OwnerSessionError::Record)?)?;
            self.container.sync()?;
            apply(&mut self.session.common, record)
        })();
        if let Err(e) = result {
            self.session.write_poisoned = true;
            return Err(e);
        }
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        Ok(true)
    }
}
impl PhyWriter {
    pub fn commit_dream_batch(&mut self, cut: DreamBatchCut) -> Result<bool, OwnerSessionError> {
        if self.poisoned
            || self.container.writer_lease_proof().is_none()
            || self.container.writer_lease_proof() != self.writer_lease_proof
        {
            return Err(OwnerSessionError::StaleDreamCut);
        }
        let Some(record) = prepare(&self.common, self.owner_id, &cut)? else {
            return Ok(false);
        };
        let result = (|| {
            self.container
                .append(&record.encode().map_err(OwnerSessionError::Record)?)?;
            self.container.sync()?;
            apply(&mut self.common, record)
        })();
        if let Err(e) = result {
            self.poisoned = true;
            return Err(e);
        }
        self.writer_lease_proof = self.container.writer_lease_proof();
        Ok(true)
    }
}

#[cfg(test)]
#[path = "dream_batch/tests.rs"]
mod tests;
