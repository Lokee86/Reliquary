use crate::{FreshnessEventProof, MemoryId};
use reliquary_dream_store::{MemoryId as OwnerMemoryId, freshness_journal as owner};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitialDreamPass {
    pub pass_id: String,
    pub source_id: MemoryId,
    pub first_revision: u64,
    pub turn: u64,
    pub accepted_settlement_turn: Option<u64>,
    pub preexisting_ids: BTreeSet<MemoryId>,
    pub graph_version_before: u64,
    pub roots: BTreeSet<MemoryId>,
}

fn oid(id: MemoryId) -> OwnerMemoryId {
    OwnerMemoryId(id.0)
}
fn rid(id: OwnerMemoryId) -> MemoryId {
    MemoryId(id.0)
}
fn opass(p: InitialDreamPass) -> owner::InitialDreamPass {
    owner::InitialDreamPass {
        pass_id: p.pass_id,
        source_id: oid(p.source_id),
        first_revision: p.first_revision,
        turn: p.turn,
        accepted_settlement_turn: p.accepted_settlement_turn,
        preexisting_ids: p.preexisting_ids.into_iter().map(oid).collect(),
        graph_version_before: p.graph_version_before,
        roots: p.roots.into_iter().map(oid).collect(),
    }
}
fn opass_ref(p: &InitialDreamPass) -> owner::InitialDreamPass {
    owner::InitialDreamPass {
        pass_id: p.pass_id.clone(),
        source_id: oid(p.source_id),
        first_revision: p.first_revision,
        turn: p.turn,
        accepted_settlement_turn: p.accepted_settlement_turn,
        preexisting_ids: p.preexisting_ids.iter().copied().map(oid).collect(),
        graph_version_before: p.graph_version_before,
        roots: p.roots.iter().copied().map(oid).collect(),
    }
}
fn rpass(p: owner::InitialDreamPass) -> InitialDreamPass {
    InitialDreamPass {
        pass_id: p.pass_id,
        source_id: rid(p.source_id),
        first_revision: p.first_revision,
        turn: p.turn,
        accepted_settlement_turn: p.accepted_settlement_turn,
        preexisting_ids: p.preexisting_ids.into_iter().map(rid).collect(),
        graph_version_before: p.graph_version_before,
        roots: p.roots.into_iter().map(rid).collect(),
    }
}
fn oproof(p: FreshnessEventProof) -> owner::AcceptedGraphCutProof {
    owner::AcceptedGraphCutProof {
        graph_version: p.graph_version,
        community_generation: p.community_generation,
        community_graph_version: p.community_graph_version,
    }
}
fn rproof(p: owner::AcceptedGraphCutProof) -> FreshnessEventProof {
    FreshnessEventProof {
        graph_version: p.graph_version,
        community_generation: p.community_generation,
        community_graph_version: p.community_graph_version,
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DreamFreshnessJournal {
    inner: owner::DreamFreshnessJournal,
}
impl DreamFreshnessJournal {
    pub(crate) fn new(owner_uuid: Option<[u8; 16]>) -> Self {
        Self {
            inner: owner::DreamFreshnessJournal::new(owner_uuid),
        }
    }
    pub(crate) fn bind_owner(&mut self, owner_uuid: Option<[u8; 16]>) -> Result<(), String> {
        self.inner.bind_owner(owner_uuid)
    }
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), String> {
        self.inner.ingest(payload)
    }
    pub(crate) fn ingest_materialized_begin(
        &mut self,
        owner_uuid: [u8; 16],
        pass: InitialDreamPass,
    ) -> Result<(), String> {
        self.inner
            .ingest_materialized_begin(owner_uuid, opass(pass))
    }
    pub(crate) fn begin_is_retry(&self, pass: &InitialDreamPass) -> Result<bool, String> {
        self.inner.begin_is_retry(&opass_ref(pass))
    }
    pub(crate) fn roots_payload(
        &self,
        pass_id: &str,
        roots: &BTreeSet<MemoryId>,
    ) -> Result<Vec<u8>, String> {
        self.inner
            .roots_payload(pass_id, &roots.iter().copied().map(oid).collect())
    }
    pub(crate) fn accept_payload(&self, pass_id: &str, turn: u64) -> Result<Vec<u8>, String> {
        self.inner.accept_payload(pass_id, turn)
    }
    pub(crate) fn accepted_graph_cut(&self, pass_id: &str) -> Option<(u64, FreshnessEventProof)> {
        self.inner
            .accepted_graph_cut(pass_id)
            .map(|(g, p)| (g, rproof(p)))
    }
    pub(crate) fn accept_graph_payload(
        &self,
        pass_id: &str,
        turn: u64,
        graph: u64,
        proof: FreshnessEventProof,
    ) -> Result<Vec<u8>, String> {
        self.inner
            .accept_graph_payload(pass_id, turn, graph, oproof(proof))
    }
    pub(crate) fn settled_payload(&self, pass_id: &str) -> Result<Vec<u8>, String> {
        self.inner.settled_payload(pass_id)
    }
    pub(crate) fn pending_for_source(&self, source: MemoryId) -> Option<InitialDreamPass> {
        self.inner.pending_for_source(oid(source)).map(rpass)
    }
    pub(crate) fn pending(&self, pass_id: &str) -> Option<InitialDreamPass> {
        self.inner.pending(pass_id).map(rpass)
    }
    pub(crate) fn is_settled(&self, pass_id: &str) -> bool {
        self.inner.is_settled(pass_id)
    }
    pub(crate) fn has_pending(&self) -> bool {
        self.inner.has_pending()
    }
    pub(crate) fn has_any(&self) -> bool {
        self.inner.has_any()
    }
    pub(crate) fn replay_settled_payload(&self, pass_id: &str) -> Result<Vec<u8>, String> {
        self.inner.replay_settled_payload(pass_id)
    }
    pub(crate) fn is_publication_birth(&self, id: MemoryId) -> bool {
        self.inner.is_publication_birth(oid(id))
    }
    pub(crate) fn publication_birth_ids(&self) -> Vec<MemoryId> {
        self.inner
            .publication_birth_ids()
            .into_iter()
            .map(rid)
            .collect()
    }
    pub(crate) fn replay_publication_birth_payload(&self, id: MemoryId) -> Result<Vec<u8>, String> {
        self.inner.replay_publication_birth_payload(oid(id))
    }
    pub(crate) fn pending_initializations(&self) -> Vec<(MemoryId, String, u64, bool)> {
        self.inner
            .pending_initializations()
            .into_iter()
            .map(|(i, m, t, a)| (rid(i), m, t, a))
            .collect()
    }
    pub(crate) fn derive_publication_birth(&mut self, id: MemoryId) {
        self.inner.derive_publication_birth(oid(id))
    }
    pub(crate) fn pending_initialization(&self, id: MemoryId) -> Option<(String, u64, bool)> {
        self.inner.pending_initialization(oid(id))
    }
    pub(crate) fn publication_intent_payload(
        &self,
        id: MemoryId,
        mutation: &str,
        turn: u64,
        admit: bool,
    ) -> Result<Vec<u8>, String> {
        self.inner
            .publication_intent_payload(oid(id), mutation, turn, admit)
    }
    pub(crate) fn consume_publication_intent_payload(
        &self,
        id: MemoryId,
    ) -> Result<Option<Vec<u8>>, String> {
        self.inner.consume_publication_intent_payload(oid(id))
    }
    #[cfg(test)]
    pub(crate) fn historical_oracle_value(&self) -> serde_json::Value {
        self.inner.historical_oracle_value()
    }
    pub(crate) fn replay_payload(
        &self,
        payload: &[u8],
        mut turn: impl FnMut(u64) -> Result<u64, String>,
        mut graph: impl FnMut(u64) -> Result<u64, String>,
        mut proof: impl FnMut(FreshnessEventProof) -> Result<FreshnessEventProof, String>,
    ) -> Result<Vec<u8>, String> {
        let Some(mut entry) = self.inner.replay_entry(payload)? else {
            return Ok(Vec::new());
        };
        entry = match entry {
            owner::DreamEntry::Accept(id, cut) => owner::DreamEntry::Accept(id, turn(cut)?),
            owner::DreamEntry::AcceptGraph(id, cut, version, pinned) => {
                owner::DreamEntry::AcceptGraph(
                    id,
                    turn(cut)?,
                    graph(version)?,
                    oproof(proof(rproof(pinned))?),
                )
            }
            owner::DreamEntry::Intent(id, mutation, cut, admitted) => {
                owner::DreamEntry::Intent(id, mutation, turn(cut)?, admitted)
            }
            other => other,
        };
        self.inner.encode_replay_entry(entry)
    }
    pub(crate) fn settled_pass_id(payload: &[u8]) -> Result<Option<String>, String> {
        owner::DreamFreshnessJournal::settled_pass_id(payload)
    }
    pub(crate) fn settled_ids(&self) -> impl Iterator<Item = &str> {
        self.inner.settled_ids()
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DirectBirthPrefix {
    inner: owner::DirectBirthPrefix,
}
impl DirectBirthPrefix {
    pub(crate) fn new(owner_uuid: [u8; 16]) -> Self {
        Self {
            inner: owner::DirectBirthPrefix::new(owner_uuid),
        }
    }
    pub(crate) fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.inner.owner_uuid()
    }
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), String> {
        self.inner.ingest(payload)
    }
    pub(crate) fn pending_intents(&self) -> impl Iterator<Item = (MemoryId, &str)> {
        self.inner
            .pending_intents()
            .iter()
            .map(|(id, mutation)| (rid(*id), mutation.as_str()))
    }
    pub(crate) fn pending_initializations(&self) -> Vec<(MemoryId, String, u64, bool)> {
        self.inner
            .pending_initializations()
            .into_iter()
            .map(|(id, mutation, turn, admitted)| (rid(id), mutation, turn, admitted))
            .collect()
    }
    pub(crate) fn consumed_intents(&self) -> impl Iterator<Item = (MemoryId, &BTreeSet<String>)> {
        self.inner
            .consumed_intents()
            .iter()
            .map(|(id, mutations)| (rid(*id), mutations))
    }
    pub(crate) fn replay_births(&self) -> impl Iterator<Item = MemoryId> + '_ {
        self.inner.replay_births().iter().copied().map(rid)
    }
}
pub(crate) fn begin_comparison_carrier(owner_uuid: [u8; 16], pass: &InitialDreamPass) -> Vec<u8> {
    owner::begin_comparison_carrier(owner_uuid, &opass_ref(pass))
}
#[cfg(test)]
pub(crate) fn historical_pass_value(pass: &InitialDreamPass) -> serde_json::Value {
    owner::historical_pass_value(&opass_ref(pass))
}
