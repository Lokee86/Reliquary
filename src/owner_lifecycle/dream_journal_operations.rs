//! Bounded Dream journal access through the one mounted REL owner.
use crate::{InitialDreamPass, MemoryId};
use std::collections::BTreeSet;
impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn dream_accept_graph_payload(
        &self,
        pass_id: &str,
        turn: u64,
        graph: u64,
        proof: crate::FreshnessEventProof,
    ) -> Result<Vec<u8>, String> {
        self.dream_freshness
            .accept_graph_payload(pass_id, turn, graph, proof)
    }
    pub(crate) fn dream_accepted_graph_cut(
        &self,
        pass_id: &str,
    ) -> Option<(u64, crate::FreshnessEventProof)> {
        self.dream_freshness.accepted_graph_cut(pass_id)
    }
    pub(crate) fn dream_begin_is_retry(&self, pass: &InitialDreamPass) -> Result<bool, String> {
        self.dream_freshness.begin_is_retry(pass)
    }
    pub(crate) fn dream_has_any(&self) -> bool {
        self.dream_freshness.has_any()
    }
    pub(crate) fn dream_has_pending(&self) -> bool {
        self.dream_freshness.has_pending()
    }
    pub(crate) fn dream_ingest(&mut self, payload: &[u8]) -> Result<(), String> {
        self.dream_freshness.ingest(payload)
    }
    pub(crate) fn dream_ingest_materialized_begin(
        &mut self,
        owner: [u8; 16],
        pass: InitialDreamPass,
    ) -> Result<(), String> {
        self.dream_freshness.ingest_materialized_begin(owner, pass)
    }
    pub(crate) fn dream_is_publication_birth(&self, id: MemoryId) -> bool {
        self.dream_freshness.is_publication_birth(id)
    }
    pub(crate) fn dream_is_settled(&self, pass_id: &str) -> bool {
        self.dream_freshness.is_settled(pass_id)
    }
    pub(crate) fn dream_pending(&self, pass_id: &str) -> Option<InitialDreamPass> {
        self.dream_freshness.pending(pass_id)
    }
    pub(crate) fn dream_pending_for_source(&self, source: MemoryId) -> Option<InitialDreamPass> {
        self.dream_freshness.pending_for_source(source)
    }
    pub(crate) fn dream_pending_initialization(&self, id: MemoryId) -> Option<(String, u64, bool)> {
        self.dream_freshness.pending_initialization(id)
    }
    pub(crate) fn dream_publication_birth_ids(&self) -> Vec<MemoryId> {
        self.dream_freshness.publication_birth_ids()
    }
    pub(crate) fn dream_replay_publication_birth_payload(
        &self,
        id: MemoryId,
    ) -> Result<Vec<u8>, String> {
        self.dream_freshness.replay_publication_birth_payload(id)
    }
    pub(crate) fn dream_replay_settled_payload(&self, pass_id: &str) -> Result<Vec<u8>, String> {
        self.dream_freshness.replay_settled_payload(pass_id)
    }
    pub(crate) fn dream_roots_payload(
        &self,
        pass_id: &str,
        roots: &BTreeSet<MemoryId>,
    ) -> Result<Vec<u8>, String> {
        self.dream_freshness.roots_payload(pass_id, roots)
    }
    pub(crate) fn dream_settled_ids(&self) -> impl Iterator<Item = &str> {
        self.dream_freshness.settled_ids()
    }
    pub(crate) fn dream_settled_payload(&self, pass_id: &str) -> Result<Vec<u8>, String> {
        self.dream_freshness.settled_payload(pass_id)
    }
}
