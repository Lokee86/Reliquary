//! Canonical semantic import settlement. Physical coordinates are construction inputs only.
//! Runtime adoption follows the codec gate in the staged import migration.
use super::{JournalEntry, Reader, put_bytes, put_event, put_record};
use crate::{
    MemoryId,
    freshness::{FreshnessPolicy, FreshnessRecord},
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const SEMANTIC_SETTLEMENT_MAGIC: &[u8; 8] = b"CVAFRS06";

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
    pub predecessor_digest: Option<[u8; 32]>,
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

fn count(out: &mut Vec<u8>, value: usize) -> Result<(), String> {
    let value = u32::try_from(value).map_err(|_| "semantic import collection too large")?;
    out.extend(value.to_le_bytes());
    Ok(())
}

fn text(out: &mut Vec<u8>, value: &str) -> Result<(), String> {
    u32::try_from(value.len()).map_err(|_| "semantic import string too large")?;
    put_bytes(out, value.as_bytes());
    Ok(())
}

fn ids(out: &mut Vec<u8>, values: &BTreeSet<MemoryId>) -> Result<(), String> {
    count(out, values.len())?;
    for id in values {
        out.extend(id.0);
    }
    Ok(())
}

fn read_text(r: &mut Reader<'_>) -> Result<String, String> {
    String::from_utf8(r.bytes()?).map_err(|_| "invalid semantic import UTF-8".into())
}

fn insert_ordered<T: Ord + Clone>(set: &mut BTreeSet<T>, value: T) -> Result<(), String> {
    if set.last().is_some_and(|old| old >= &value) {
        return Err("unordered or duplicate semantic import identity".into());
    }
    set.insert(value);
    Ok(())
}

fn read_count(r: &mut Reader<'_>, minimum_bytes: usize) -> Result<u32, String> {
    let count = r.u32()?;
    if count as usize > (r.bytes.len() - r.at) / minimum_bytes {
        return Err("semantic import count exceeds remaining bytes".into());
    }
    Ok(count)
}

fn read_ids(r: &mut Reader<'_>) -> Result<BTreeSet<MemoryId>, String> {
    let mut values = BTreeSet::new();
    for _ in 0..read_count(r, 32)? {
        insert_ordered(&mut values, MemoryId(r.array()?))?;
    }
    Ok(values)
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
            || scope.predecessor_digest == Some([0; 32])
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

    pub(crate) fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let s = &self.scope;
        let mut out = SEMANTIC_SETTLEMENT_MAGIC.to_vec();
        out.extend(s.owner);
        for digest in [
            s.import_id,
            s.start_evidence_digest,
            self.corpus_digest,
            self.manifest_digest,
            self.input_digest,
            crate::freshness::historical_planner::HISTORICAL_POLICY_DIGEST,
            crate::freshness::semantic_policy::SemanticPolicy::default()
                .digest()
                .map_err(|e| e.to_string())?,
        ] {
            out.extend(digest);
        }
        for value in [
            s.base_cut,
            s.settlement_cut,
            self.global_version,
            self.archive_version,
            self.graph_version,
            self.memory_graph_version,
        ] {
            out.extend(value.to_le_bytes());
        }
        match s.predecessor_digest {
            None => out.push(0),
            Some(digest) => {
                out.push(1);
                out.extend(digest);
            }
        }
        ids(&mut out, &s.born_memories)?;
        ids(&mut out, &s.affected_recipients)?;
        count(&mut out, s.source_nodes.len())?;
        for (conversation, node) in &s.source_nodes {
            text(&mut out, conversation)?;
            text(&mut out, node)?;
        }
        count(&mut out, s.accepted_events.len())?;
        for event in s.accepted_events.values() {
            text(&mut out, &event.event_id)?;
            out.extend(event.payload_digest);
            out.extend(event.accepted_turn.to_le_bytes());
            ids(&mut out, &event.affected_recipients)?;
        }
        count(&mut out, s.superseded.len())?;
        for id in &s.superseded {
            text(&mut out, id)?;
        }
        ids(&mut out, &self.cohort)?;
        count(&mut out, self.records.len())?;
        for (id, record) in &self.records {
            out.extend(id.0);
            put_record(&mut out, *record);
        }
        out.extend(Sha256::digest(&out));
        Ok(out)
    }

    pub(crate) fn decode(payload: &[u8]) -> Result<Self, String> {
        if payload.len() < 40 || !payload.starts_with(SEMANTIC_SETTLEMENT_MAGIC) {
            return Err("invalid semantic import settlement framing".into());
        }
        let end = payload.len() - 32;
        if Sha256::digest(&payload[..end]).as_slice() != &payload[end..] {
            return Err("semantic import settlement checksum mismatch".into());
        }
        // Reader checks each extent before allocating; collection counts never
        // reserve capacity from untrusted input.
        let mut r = Reader {
            bytes: &payload[..end],
            at: 8,
        };
        let owner = r.array()?;
        let import_id = r.array()?;
        let start_evidence_digest = r.array()?;
        let corpus_digest = r.array()?;
        let manifest_digest = r.array()?;
        let input_digest = r.array()?;
        if r.array::<32>()? != crate::freshness::historical_planner::HISTORICAL_POLICY_DIGEST
            || r.array::<32>()?
                != crate::freshness::semantic_policy::SemanticPolicy::default()
                    .digest()
                    .map_err(|e| e.to_string())?
        {
            return Err("unsupported semantic import policy".into());
        }
        let base_cut = r.u64()?;
        let settlement_cut = r.u64()?;
        let global_version = r.u64()?;
        let archive_version = r.u64()?;
        let graph_version = r.u64()?;
        let memory_graph_version = r.u64()?;
        let predecessor_digest = match r.u8()? {
            0 => None,
            1 => Some(r.array()?),
            _ => return Err("invalid semantic import predecessor tag".into()),
        };
        let born_memories = read_ids(&mut r)?;
        let affected_recipients = read_ids(&mut r)?;
        let mut source_nodes = BTreeSet::new();
        for _ in 0..read_count(&mut r, 8)? {
            let source = (read_text(&mut r)?, read_text(&mut r)?);
            insert_ordered(&mut source_nodes, source)?;
        }
        let mut accepted_events = BTreeMap::new();
        for _ in 0..read_count(&mut r, 48)? {
            let event_id = read_text(&mut r)?;
            if accepted_events
                .last_key_value()
                .is_some_and(|(old, _)| old >= &event_id)
            {
                return Err("unordered or duplicate accepted import event".into());
            }
            let event = AcceptedImportEventEvidence {
                event_id: event_id.clone(),
                payload_digest: r.array()?,
                accepted_turn: r.u64()?,
                affected_recipients: read_ids(&mut r)?,
            };
            accepted_events.insert(event_id, event);
        }
        let mut superseded = BTreeSet::new();
        for _ in 0..read_count(&mut r, 4)? {
            insert_ordered(&mut superseded, read_text(&mut r)?)?;
        }
        let cohort = read_ids(&mut r)?;
        let mut records = BTreeMap::new();
        for _ in 0..read_count(&mut r, 36)? {
            let id = MemoryId(r.array()?);
            if records.last_key_value().is_some_and(|(old, _)| *old >= id) {
                return Err("unordered or duplicate semantic import record".into());
            }
            records.insert(id, r.record()?);
        }
        if r.at != end {
            return Err("trailing semantic import settlement bytes".into());
        }
        let settlement = Self {
            scope: ImportSemanticScope {
                owner,
                import_id,
                start_evidence_digest,
                born_memories,
                affected_recipients,
                source_nodes,
                base_cut,
                settlement_cut,
                accepted_events,
                superseded,
                predecessor_digest,
            },
            corpus_digest,
            manifest_digest,
            input_digest,
            global_version,
            archive_version,
            graph_version,
            memory_graph_version,
            cohort,
            records,
        };
        settlement.validate()?;
        Ok(settlement)
    }
}

#[cfg(test)]
#[path = "freshness_semantic_settlement_tests.rs"]
mod tests;
