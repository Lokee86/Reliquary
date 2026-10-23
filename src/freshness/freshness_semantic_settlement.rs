//! Canonical semantic import settlement. Physical coordinates are construction inputs only.
//! Runtime adoption follows the codec gate in the staged import migration.
use super::{JournalEntry, put_event};
use crate::{
    MemoryId,
    freshness::{FreshnessPolicy, FreshnessRecord},
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const SEMANTIC_SETTLEMENT_MAGIC: &[u8; 8] = super::compact_settlement::MAGIC;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedImportEventEvidence {
    pub event_id: String,
    pub payload_digest: [u8; 32],
    pub accepted_turn: u64,
    pub affected_recipients: BTreeSet<MemoryId>,
}

impl AcceptedImportEventEvidence {
    /// Hash the canonical immutable event encoding, domain-separated from journal
    /// and settlement hashes. Derived postimages, frame offsets and batch framing
    /// are excluded; effects, provenance, graph proof and F8 evidence are included.
    pub(super) fn from_event(owner: [u8; 16], event: &JournalEntry) -> Result<Self, String> {
        let JournalEntry::Event {
            event_id,
            turn,
            provenance,
            proof,
            effects,
            semantic,
            ..
        } = event
        else {
            return Err("import evidence requires an accepted event".into());
        };
        if event_id.trim().is_empty()
            || owner == [0; 16]
            || super::is_semantic_producer(provenance.producer_kind) != semantic.is_some()
        {
            return Err("invalid import event digest input".into());
        }
        let mut bytes = b"reliquary-import-accepted-event-v1".to_vec();
        bytes.extend(owner);
        put_event(
            &mut bytes,
            event_id,
            *turn,
            provenance,
            *proof,
            effects,
            &BTreeMap::new(),
            semantic.as_ref(),
        );
        Ok(Self {
            event_id: event_id.clone(),
            payload_digest: Sha256::digest(bytes).into(),
            accepted_turn: *turn,
            affected_recipients: effects.keys().copied().collect(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ImportSemanticScope {
    pub owner: [u8; 16],
    pub import_id: [u8; 32],
    pub start_evidence_digest: [u8; 32],
    pub born_memories: BTreeSet<MemoryId>,
    pub affected_recipients: BTreeSet<MemoryId>,
    pub source_nodes: BTreeSet<(String, String)>,
    pub base_cut: u64,
    pub settlement_cut: u64,
    /// Every accepted in-epoch event relevant to the replacement population,
    /// including retained live events as well as provisional events.
    pub accepted_events: BTreeMap<String, AcceptedImportEventEvidence>,
    pub superseded: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SemanticImportFreshnessSettlement {
    pub scope: ImportSemanticScope,
    pub corpus_digest: [u8; 32],
    pub manifest_digest: [u8; 32],
    pub input_digest: [u8; 32],
    pub global_version: u64,
    pub archive_version: u64,
    pub graph_version: u64,
    pub memory_graph_version: u64,
    pub cohort: BTreeSet<MemoryId>,
    pub records: BTreeMap<MemoryId, FreshnessRecord>,
}

impl SemanticImportFreshnessSettlement {
    pub(crate) fn validate(&self) -> Result<(), String> {
        let scope = &self.scope;
        if scope.owner == [0; 16]
            || scope.import_id == [0; 32]
            || scope.start_evidence_digest == [0; 32]
            || self.manifest_digest == [0; 32]
            || self.input_digest == [0; 32]
            || scope.base_cut > scope.settlement_cut
            || self.archive_version > self.global_version
            || self.graph_version > self.global_version
            || self.memory_graph_version > self.graph_version
            || !scope.born_memories.is_subset(&self.cohort)
            || !self.cohort.is_subset(&scope.affected_recipients)
            || self.records.keys().any(|id| !self.cohort.contains(id))
            || scope.source_nodes.iter().any(|(conversation, node)| {
                conversation.trim().is_empty() || node.trim().is_empty()
            })
        {
            return Err("invalid semantic import identity/cohort".into());
        }
        for (id, event) in &scope.accepted_events {
            if id != &event.event_id
                || id.trim().is_empty()
                || event.payload_digest == [0; 32]
                || event.accepted_turn < scope.base_cut
                || event.accepted_turn > scope.settlement_cut
                || !event
                    .affected_recipients
                    .is_subset(&scope.affected_recipients)
            {
                return Err("invalid accepted import event evidence".into());
            }
        }
        if scope
            .superseded
            .iter()
            .any(|id| !scope.accepted_events.contains_key(id))
        {
            return Err("superseded import event has no accepted evidence".into());
        }
        for record in self.records.values() {
            if record.admitted_at_turn.is_none() {
                if *record != FreshnessRecord::created() {
                    return Err("invalid unadmitted semantic import record".into());
                }
                continue;
            }
            let mut settled = *record;
            settled
                .settle_with_policy(scope.settlement_cut, &FreshnessPolicy::default_v1())
                .map_err(|e| e.to_string())?;
            if settled != *record
                || record
                    .admitted_at_turn
                    .is_some_and(|at| at == 0 || at > scope.settlement_cut)
            {
                return Err("semantic import record is not canonical at settlement cut".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "freshness_semantic_settlement_tests.rs"]
mod tests;
