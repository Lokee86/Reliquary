use crate::MemoryId;

use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedGraphCutProof {
    pub graph_version: u64,
    pub community_generation: Option<u64>,
    pub community_graph_version: Option<u64>,
}

const MAGIC: &[u8; 8] = b"CVDREAM1";

#[cfg(feature = "oracle-support")]
pub fn historical_pass_value(pass: &InitialDreamPass) -> serde_json::Value {
    let InitialDreamPass {
        pass_id,
        source_id,
        first_revision,
        turn,
        accepted_settlement_turn,
        preexisting_ids,
        graph_version_before,
        roots,
    } = pass;
    serde_json::json!({
        "pass_id": pass_id, "source_id": source_id.0, "first_revision": first_revision,
        "turn": turn, "accepted_settlement_turn": accepted_settlement_turn,
        "preexisting_ids": preexisting_ids.iter().map(|id| id.0).collect::<Vec<_>>(),
        "graph_version_before": graph_version_before,
        "roots": roots.iter().map(|id| id.0).collect::<Vec<_>>(),
    })
}

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

#[derive(Clone, Debug, Default)]
pub struct DreamFreshnessJournal {
    owner_uuid: Option<[u8; 16]>,
    pending: BTreeMap<String, InitialDreamPass>,
    settled: BTreeSet<String>,
    pending_initializations: BTreeMap<MemoryId, (String, u64, bool)>,
    publication_births: BTreeSet<MemoryId>,
    accepted_graph_cuts: BTreeMap<String, (u64, AcceptedGraphCutProof)>,
}

impl DreamFreshnessJournal {
    /// Clock-neutral accepted resume state for the test-only equivalence oracle.
    #[cfg(feature = "oracle-support")]
    pub fn historical_oracle_value(&self) -> serde_json::Value {
        use serde_json::json;
        json!({
            "pending": self.pending.values().map(historical_pass_value).collect::<Vec<_>>(),
            "settled": self.settled,
            "pending_initializations": self.pending_initializations.iter().map(|(id, (mutation, turn, admitted))| json!([id.0, mutation, turn, admitted])).collect::<Vec<_>>(),
            "publication_births": self.publication_births.iter().map(|id| id.0).collect::<Vec<_>>(),
            "accepted_graph_cuts": self.accepted_graph_cuts.iter().map(|(id, (turn, proof))| {
                let AcceptedGraphCutProof { graph_version, community_generation, community_graph_version } = *proof;
                json!([id, turn, graph_version, community_generation, community_graph_version])
            }).collect::<Vec<_>>(),
        })
    }

    /// Read owner-validated replay coordinates without invoking semantic mappings.
    pub fn replay_entry(&self, payload: &[u8]) -> Result<Option<DreamEntry>, String> {
        if payload.get(..8) != Some(MAGIC.as_slice()) {
            return Ok(None);
        }
        let mut reader = Reader {
            bytes: payload,
            at: 8,
        };
        if self.owner_uuid != Some(reader.array::<16>()?) {
            return Err("Dream replay owner mismatch".into());
        }
        let entry = decode_entry(&mut reader)?;
        if reader.at != payload.len() {
            return Err("trailing Dream replay bytes".into());
        }
        Ok(Some(entry))
    }

    /// Re-encode caller-mapped coordinates through local retry and journal rules.
    pub fn encode_replay_entry(&self, entry: DreamEntry) -> Result<Vec<u8>, String> {
        let result = match entry {
            DreamEntry::Roots(id, roots) => self.roots_payload(&id, &roots)?,
            DreamEntry::Settled(id) => self.settled_payload(&id)?,
            DreamEntry::Accept(id, cut) => self.accept_payload(&id, cut)?,
            DreamEntry::AcceptGraph(id, cut, version, pinned) => {
                self.accept_graph_payload(&id, cut, version, pinned)?
            }
            DreamEntry::Intent(id, mutation, cut, admitted) => {
                self.publication_intent_payload(id, &mutation, cut, admitted)?
            }
            DreamEntry::Consume(id, mutation) => {
                if self
                    .pending_initialization(id)
                    .is_some_and(|(known, _, _)| known != mutation)
                {
                    return Err("Dream replay intent mutation mismatch".into());
                }
                self.consume_publication_intent_payload(id)?
                    .unwrap_or_default()
            }
            DreamEntry::ReplaySettled(id) => self.replay_settled_payload(&id)?,
            DreamEntry::ReplayBirth(id) => self.replay_publication_birth_payload(id)?,
        };
        let mut staged = self.clone();
        staged.ingest(&result)?;
        Ok(result)
    }

    /// Return the pass ID only for a well-framed settled journal event. The
    /// concrete REL replay coordinator owns retaining completed proof state.
    pub fn settled_pass_id(payload: &[u8]) -> Result<Option<String>, String> {
        if payload.get(..8) != Some(MAGIC.as_slice()) {
            return Ok(None);
        }
        let mut reader = Reader {
            bytes: payload,
            at: 24,
        };
        let entry = decode_entry(&mut reader)?;
        if reader.at != payload.len() {
            return Err("trailing Dream Freshness journal bytes".into());
        }
        Ok(match entry {
            DreamEntry::Settled(pass_id) => Some(pass_id),
            _ => None,
        })
    }

    pub fn new(owner_uuid: Option<[u8; 16]>) -> Self {
        Self {
            owner_uuid,
            ..Self::default()
        }
    }

    pub fn bind_owner(&mut self, owner: Option<[u8; 16]>) -> Result<(), String> {
        if self.owner_uuid.is_some() && self.owner_uuid != owner {
            return Err("Dream Freshness pass belongs to another owner".into());
        }
        if !self.pending.is_empty() && owner.is_none() {
            return Err("legacy owner cannot contain Dream Freshness passes".into());
        }
        self.owner_uuid = owner;
        Ok(())
    }

    pub fn ingest(&mut self, payload: &[u8]) -> Result<(), String> {
        if payload.get(..8) != Some(MAGIC.as_slice()) {
            return Ok(());
        }
        let mut r = Reader {
            bytes: payload,
            at: 8,
        };
        let owner = r.array::<16>()?;
        if self.owner_uuid.is_some_and(|expected| expected != owner) {
            return Err("Dream Freshness pass owner mismatch".into());
        }
        let entry = decode_entry(&mut r)?;
        if r.at != payload.len() {
            return Err("trailing Dream Freshness journal bytes".into());
        }
        match entry {
            DreamEntry::Roots(id, roots) => {
                let pass = self
                    .pending
                    .get_mut(&id)
                    .ok_or("Dream Freshness roots have no begun pass")?;
                if roots
                    .iter()
                    .any(|root| !pass.preexisting_ids.contains(root))
                {
                    return Err("Dream Freshness roots must be pre-existing Memories".into());
                }
                pass.roots.extend(roots);
            }
            DreamEntry::Settled(id) => {
                if !self.pending.contains_key(&id) {
                    return Err("settled Dream Freshness pass was not begun".into());
                }
                if self.pending[&id].accepted_settlement_turn.is_none() {
                    return Err("Dream settlement lacks its durable acceptance cut".into());
                }
                self.pending.remove(&id);
                self.settled.insert(id);
            }
            DreamEntry::Accept(id, turn) => {
                let pass = self
                    .pending
                    .get_mut(&id)
                    .ok_or("Dream settlement acceptance has no begun pass")?;
                if turn < pass.turn {
                    return Err("Dream settlement precedes its begun pass".into());
                }
                match pass.accepted_settlement_turn {
                    Some(existing) if existing == turn => {}
                    Some(_) => return Err("Dream settlement acceptance cut cannot change".into()),
                    None => pass.accepted_settlement_turn = Some(turn),
                }
            }
            DreamEntry::AcceptGraph(id, turn, graph, proof) => {
                let pass = self
                    .pending
                    .get_mut(&id)
                    .ok_or("Dream settlement acceptance has no begun pass")?;
                if turn < pass.turn
                    || graph < pass.graph_version_before
                    || proof.community_generation.is_some()
                        != proof.community_graph_version.is_some()
                    || proof
                        .community_graph_version
                        .is_some_and(|version| version != proof.graph_version)
                {
                    return Err("invalid Dream accepted graph cut".into());
                }
                if pass
                    .accepted_settlement_turn
                    .is_some_and(|existing| existing != turn)
                    || self
                        .accepted_graph_cuts
                        .get(&id)
                        .is_some_and(|existing| *existing != (graph, proof))
                {
                    return Err("Dream accepted graph cut cannot change".into());
                }
                pass.accepted_settlement_turn = Some(turn);
                self.accepted_graph_cuts.insert(id, (graph, proof));
            }
            DreamEntry::Intent(id, mutation_id, turn, admit_at_birth) => {
                match self.pending_initializations.get(&id) {
                    Some(existing) if existing == &(mutation_id.clone(), turn, admit_at_birth) => {}
                    Some(_) => return Err("conflicting Freshness publication intent".into()),
                    None => {
                        self.pending_initializations
                            .insert(id, (mutation_id, turn, admit_at_birth));
                    }
                }
            }
            DreamEntry::Consume(id, mutation_id) => match self.pending_initializations.get(&id) {
                Some((existing, _, _)) if existing == &mutation_id => {
                    self.pending_initializations.remove(&id);
                    self.publication_births.insert(id);
                }
                None => {}
                _ => return Err("Freshness publication intent receipt mismatch".into()),
            },
            DreamEntry::ReplaySettled(id) => {
                if id.is_empty() {
                    return Err("empty settled pass ID".into());
                }
                self.pending.remove(&id);
                self.settled.insert(id);
            }
            DreamEntry::ReplayBirth(id) => {
                self.publication_births.insert(id);
            }
        }
        self.owner_uuid.get_or_insert(owner);
        Ok(())
    }

    /// Admit a decoded current Begin after its population authority has been
    /// materialized by REL at the physical marker. This deliberately follows
    /// the exact accepted Begin identity rules rather than going through the writer.
    pub fn ingest_materialized_begin(
        &mut self,
        owner: [u8; 16],
        pass: InitialDreamPass,
    ) -> Result<(), String> {
        if self.owner_uuid.is_some_and(|expected| expected != owner) {
            return Err("Dream Freshness pass owner mismatch".into());
        }
        if pass.pass_id.is_empty()
            || pass.first_revision != 1
            || pass.preexisting_ids.contains(&pass.source_id)
        {
            return Err("invalid initial Dream Freshness pass".into());
        }
        self.admit_begin(pass)?;
        self.owner_uuid.get_or_insert(owner);
        Ok(())
    }

    fn admit_begin(&mut self, pass: InitialDreamPass) -> Result<(), String> {
        match self.pending.get(&pass.pass_id) {
            Some(existing)
                if existing.source_id == pass.source_id
                    && existing.first_revision == pass.first_revision
                    && existing.turn == pass.turn
                    && existing.preexisting_ids == pass.preexisting_ids
                    && existing.graph_version_before == pass.graph_version_before => {}
            Some(_) => return Err("conflicting Dream Freshness pass identity".into()),
            None => {
                self.pending.insert(pass.pass_id.clone(), pass);
            }
        }
        Ok(())
    }

    pub fn begin_is_retry(&self, pass: &InitialDreamPass) -> Result<bool, String> {
        if self.settled.contains(&pass.pass_id) {
            return Err("Dream Freshness pass already settled".into());
        }
        if let Some(existing) = self.pending.get(&pass.pass_id) {
            if existing.source_id == pass.source_id
                && existing.first_revision == pass.first_revision
                && existing.turn == pass.turn
                && existing.preexisting_ids == pass.preexisting_ids
                && existing.graph_version_before == pass.graph_version_before
            {
                return Ok(true);
            }
            return Err("Dream Freshness pass ID collision".into());
        }
        self.owner_uuid
            .ok_or("Dream pass requires an owner identity")?;
        Ok(false)
    }

    pub fn roots_payload(
        &self,
        pass_id: &str,
        roots: &BTreeSet<MemoryId>,
    ) -> Result<Vec<u8>, String> {
        let pass = self
            .pending
            .get(pass_id)
            .ok_or("Dream Freshness pass is not pending")?;
        if roots.iter().any(|id| !pass.preexisting_ids.contains(id)) {
            return Err("Dream Freshness roots must be pre-existing Memories".into());
        }
        let owner = self
            .owner_uuid
            .ok_or("legacy owner requires explicit Freshness migration")?;
        let mut out = header(owner, 1);
        put_bytes(&mut out, pass_id.as_bytes());
        put_ids(&mut out, roots);
        Ok(out)
    }

    pub fn accept_payload(&self, pass_id: &str, turn: u64) -> Result<Vec<u8>, String> {
        let pass = self
            .pending
            .get(pass_id)
            .ok_or("Dream Freshness pass is not pending")?;
        if turn < pass.turn {
            return Err("Dream settlement precedes its begun pass".into());
        }
        if let Some(existing) = pass.accepted_settlement_turn {
            if existing != turn {
                return Err("Dream settlement acceptance cut cannot change".into());
            }
            return Ok(Vec::new());
        }
        let owner = self
            .owner_uuid
            .ok_or("legacy owner requires explicit Freshness migration")?;
        let mut out = header(owner, 3);
        put_bytes(&mut out, pass_id.as_bytes());
        out.extend_from_slice(&turn.to_le_bytes());
        Ok(out)
    }

    pub fn accepted_graph_cut(&self, pass_id: &str) -> Option<(u64, AcceptedGraphCutProof)> {
        self.accepted_graph_cuts.get(pass_id).copied()
    }

    pub fn accept_graph_payload(
        &self,
        pass_id: &str,
        turn: u64,
        graph: u64,
        proof: AcceptedGraphCutProof,
    ) -> Result<Vec<u8>, String> {
        let pass = self
            .pending
            .get(pass_id)
            .ok_or("Dream pass is not pending")?;
        if let Some(existing) = pass.accepted_settlement_turn {
            return if existing == turn && self.accepted_graph_cuts.contains_key(pass_id) {
                Ok(Vec::new())
            } else {
                Err("Dream accepted settlement provenance is incomplete or conflicts".into())
            };
        }
        if turn < pass.turn
            || graph < pass.graph_version_before
            || proof.community_generation.is_some() != proof.community_graph_version.is_some()
            || proof
                .community_graph_version
                .is_some_and(|version| version != proof.graph_version)
        {
            return Err("invalid Dream accepted graph proof".into());
        }
        let mut out = header(self.owner_uuid.ok_or("Dream acceptance lacks owner")?, 8);
        put_bytes(&mut out, pass_id.as_bytes());
        out.extend_from_slice(&turn.to_le_bytes());
        out.extend_from_slice(&graph.to_le_bytes());
        out.extend_from_slice(&proof.graph_version.to_le_bytes());
        match proof.community_generation {
            Some(generation) => {
                out.push(1);
                out.extend_from_slice(&generation.to_le_bytes());
                out.extend_from_slice(&proof.community_graph_version.unwrap().to_le_bytes());
            }
            None => out.push(0),
        }
        Ok(out)
    }

    pub fn settled_payload(&self, pass_id: &str) -> Result<Vec<u8>, String> {
        let pass = self
            .pending
            .get(pass_id)
            .ok_or("Dream Freshness pass is not pending")?;
        if pass.accepted_settlement_turn.is_none() {
            return Err("Dream settlement acceptance cut is required".into());
        }
        let owner = self
            .owner_uuid
            .ok_or("legacy owner requires explicit Freshness migration")?;
        let mut out = header(owner, 2);
        put_bytes(&mut out, pass_id.as_bytes());
        Ok(out)
    }

    pub fn pending_for_source(&self, source: MemoryId) -> Option<InitialDreamPass> {
        self.pending
            .values()
            .find(|pass| pass.source_id == source)
            .cloned()
    }
    pub fn pending(&self, pass_id: &str) -> Option<InitialDreamPass> {
        self.pending.get(pass_id).cloned()
    }
    pub fn is_settled(&self, pass_id: &str) -> bool {
        self.settled.contains(pass_id)
    }
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty() || !self.pending_initializations.is_empty()
    }
    pub fn settled_ids(&self) -> impl Iterator<Item = &str> {
        self.settled.iter().map(String::as_str)
    }
    pub fn replay_settled_payload(&self, pass_id: &str) -> Result<Vec<u8>, String> {
        if pass_id.is_empty() {
            return Err("empty settled pass ID".into());
        }
        if self.settled.contains(pass_id) {
            return Ok(Vec::new());
        }
        let owner = self
            .owner_uuid
            .ok_or("legacy owner requires explicit Freshness migration")?;
        let mut out = header(owner, 6);
        put_bytes(&mut out, pass_id.as_bytes());
        Ok(out)
    }
    pub fn is_publication_birth(&self, id: MemoryId) -> bool {
        self.pending_initializations.contains_key(&id) || self.publication_births.contains(&id)
    }
    pub fn publication_birth_ids(&self) -> Vec<MemoryId> {
        self.publication_births.iter().copied().collect()
    }
    pub fn replay_publication_birth_payload(&self, id: MemoryId) -> Result<Vec<u8>, String> {
        if self.publication_births.contains(&id) {
            return Ok(Vec::new());
        }
        let owner = self
            .owner_uuid
            .ok_or("legacy owner requires explicit Freshness migration")?;
        let mut out = header(owner, 7);
        out.extend_from_slice(&id.0);
        Ok(out)
    }
    pub fn has_any(&self) -> bool {
        !self.pending.is_empty()
            || !self.settled.is_empty()
            || !self.pending_initializations.is_empty()
            || !self.publication_births.is_empty()
    }
    pub fn pending_initializations(&self) -> Vec<(MemoryId, String, u64, bool)> {
        self.pending_initializations
            .iter()
            .map(|(id, (mutation, turn, admitted))| (*id, mutation.clone(), *turn, *admitted))
            .collect()
    }
    pub fn derive_publication_birth(&mut self, id: MemoryId) {
        self.publication_births.insert(id);
    }
    pub fn pending_initialization(&self, id: MemoryId) -> Option<(String, u64, bool)> {
        self.pending_initializations.get(&id).cloned()
    }

    pub fn publication_intent_payload(
        &self,
        id: MemoryId,
        mutation_id: &str,
        turn: u64,
        admit_at_birth: bool,
    ) -> Result<Vec<u8>, String> {
        if let Some(existing) = self.pending_initializations.get(&id) {
            if existing == &(mutation_id.to_owned(), turn, admit_at_birth) {
                return Ok(Vec::new());
            }
            return Err("conflicting Freshness publication intent".into());
        }
        let owner = self
            .owner_uuid
            .ok_or("legacy owner requires explicit Freshness migration")?;
        let mut out = header(owner, 4);
        out.extend_from_slice(&id.0);
        put_bytes(&mut out, mutation_id.as_bytes());
        out.extend_from_slice(&turn.to_le_bytes());
        out.push(u8::from(admit_at_birth));
        Ok(out)
    }

    pub fn consume_publication_intent_payload(
        &self,
        id: MemoryId,
    ) -> Result<Option<Vec<u8>>, String> {
        let Some((mutation_id, _, _)) = self.pending_initializations.get(&id) else {
            return Ok(None);
        };
        let owner = self
            .owner_uuid
            .ok_or("legacy owner requires explicit Freshness migration")?;
        let mut out = header(owner, 5);
        out.extend_from_slice(&id.0);
        put_bytes(&mut out, mutation_id.as_bytes());
        Ok(Some(out))
    }
}

/// Direct birth evidence visible at one physical Dream-journal prefix. Memory
/// validates the first-publication identity and mutation; REL combines those
/// Memory facts with this journal-owned evidence.
#[derive(Clone, Debug, Default)]
pub struct DirectBirthPrefix {
    journal: DreamFreshnessJournal,
    pending_intents: BTreeMap<MemoryId, String>,
    consumed_intents: BTreeMap<MemoryId, BTreeSet<String>>,
    replay_births: BTreeSet<MemoryId>,
}

impl DirectBirthPrefix {
    pub fn new(owner_uuid: [u8; 16]) -> Self {
        Self {
            journal: DreamFreshnessJournal::new(Some(owner_uuid)),
            ..Self::default()
        }
    }

    pub fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.journal.owner_uuid
    }

    /// Ingest one physically ordered Dream payload. Callers stop before the
    /// Begin frame whose population is being reconstructed.
    pub fn ingest(&mut self, payload: &[u8]) -> Result<(), String> {
        if payload.get(..8) != Some(MAGIC.as_slice()) {
            return Ok(());
        }
        let owner: [u8; 16] = payload
            .get(8..24)
            .ok_or("truncated Dream Freshness header")?
            .try_into()
            .expect("owner length checked");
        let mut reader = Reader {
            bytes: payload,
            at: 24,
        };
        let entry = decode_entry(&mut reader)?;
        if reader.at != payload.len() {
            return Err("trailing Dream Freshness journal bytes".into());
        }

        let direct_birth_entry = matches!(
            &entry,
            DreamEntry::Intent(..) | DreamEntry::Consume(..) | DreamEntry::ReplayBirth(..)
        );
        if self
            .journal
            .owner_uuid
            .is_some_and(|expected| expected != owner)
        {
            return Err("Dream Freshness pass owner mismatch".into());
        }
        if !direct_birth_entry {
            return Ok(());
        }
        let matching_pending = match &entry {
            DreamEntry::Consume(id, mutation) => self.pending_intents.get(id) == Some(mutation),
            _ => false,
        };
        self.journal.ingest(payload)?;
        match &entry {
            DreamEntry::Intent(id, mutation, _, _) => {
                self.pending_intents.insert(*id, mutation.clone());
            }
            DreamEntry::Consume(id, mutation) => {
                if matching_pending {
                    self.pending_intents.remove(id);
                    self.consumed_intents
                        .entry(*id)
                        .or_default()
                        .insert(mutation.clone());
                }
            }
            DreamEntry::ReplayBirth(id) => {
                self.replay_births.insert(*id);
            }
            _ => {}
        }
        Ok(())
    }

    pub fn pending_intents(&self) -> &BTreeMap<MemoryId, String> {
        &self.pending_intents
    }

    pub fn pending_initializations(&self) -> Vec<(MemoryId, String, u64, bool)> {
        self.journal.pending_initializations()
    }

    pub fn consumed_intents(&self) -> &BTreeMap<MemoryId, BTreeSet<String>> {
        &self.consumed_intents
    }

    pub fn replay_births(&self) -> &BTreeSet<MemoryId> {
        &self.replay_births
    }
}

/// Stable semantic comparison identity for completed-import evidence. This
/// carrier is never appended or admitted as a physical Dream Begin record.
pub fn begin_comparison_carrier(owner: [u8; 16], pass: &InitialDreamPass) -> Vec<u8> {
    let mut out = header(owner, 0);
    put_bytes(&mut out, pass.pass_id.as_bytes());
    out.extend_from_slice(&pass.source_id.0);
    out.extend_from_slice(&pass.first_revision.to_le_bytes());
    out.extend_from_slice(&pass.turn.to_le_bytes());
    out.extend_from_slice(&pass.graph_version_before.to_le_bytes());
    put_ids(&mut out, &pass.preexisting_ids);
    out
}

pub enum DreamEntry {
    Roots(String, BTreeSet<MemoryId>),
    Settled(String),
    Accept(String, u64),
    AcceptGraph(String, u64, u64, AcceptedGraphCutProof),
    Intent(MemoryId, String, u64, bool),
    Consume(MemoryId, String),
    ReplaySettled(String),
    ReplayBirth(MemoryId),
}

// Decode the entire payload before changing owner-local projections.
fn decode_entry(r: &mut Reader<'_>) -> Result<DreamEntry, String> {
    fn identity(r: &mut Reader<'_>) -> Result<String, String> {
        let value =
            String::from_utf8(r.bytes()?).map_err(|_| "invalid Dream journal identity UTF-8")?;
        if value.trim().is_empty() {
            return Err("empty Dream journal identity".into());
        }
        Ok(value)
    }
    Ok(match r.u8()? {
        1 => DreamEntry::Roots(identity(r)?, r.ids()?),
        2 => DreamEntry::Settled(identity(r)?),
        3 => DreamEntry::Accept(identity(r)?, r.u64()?),
        4 => {
            let id = MemoryId(r.array()?);
            let mutation = identity(r)?;
            let turn = r.u64()?;
            let admitted = match r.u8()? {
                0 => false,
                1 => true,
                _ => return Err("invalid Dream publication admission flag".into()),
            };
            DreamEntry::Intent(id, mutation, turn, admitted)
        }
        5 => DreamEntry::Consume(MemoryId(r.array()?), identity(r)?),
        6 => DreamEntry::ReplaySettled(identity(r)?),
        7 => DreamEntry::ReplayBirth(MemoryId(r.array()?)),
        8 => {
            let id = identity(r)?;
            let turn = r.u64()?;
            let graph = r.u64()?;
            let graph_version = r.u64()?;
            let (community_generation, community_graph_version) = match r.u8()? {
                0 => (None, None),
                1 => (Some(r.u64()?), Some(r.u64()?)),
                _ => return Err("invalid Dream community proof flag".into()),
            };
            DreamEntry::AcceptGraph(
                id,
                turn,
                graph,
                AcceptedGraphCutProof {
                    graph_version,
                    community_generation,
                    community_graph_version,
                },
            )
        }
        _ => return Err("unknown Dream Freshness journal entry".into()),
    })
}

fn header(owner: [u8; 16], kind: u8) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&owner);
    out.push(kind);
    out
}
fn put_bytes(out: &mut Vec<u8>, value: &[u8]) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value);
}
fn put_ids(out: &mut Vec<u8>, ids: &BTreeSet<MemoryId>) {
    out.extend_from_slice(&(ids.len() as u32).to_le_bytes());
    for id in ids {
        out.extend_from_slice(&id.0);
    }
}
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self
            .at
            .checked_add(n)
            .ok_or("Dream Freshness record overflow")?;
        let b = self
            .bytes
            .get(self.at..end)
            .ok_or("truncated Dream Freshness record")?;
        self.at = end;
        Ok(b)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().expect("length checked"))
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn bytes(&mut self) -> Result<Vec<u8>, String> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
    fn ids(&mut self) -> Result<BTreeSet<MemoryId>, String> {
        let n = self.u32()? as usize;
        let mut ids = BTreeSet::new();
        for _ in 0..n {
            if !ids.insert(MemoryId(self.array()?)) {
                return Err("duplicate Memory ID in Dream Freshness pass".into());
            }
        }
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_receipts_leave_birth_and_pass_projections_unchanged() {
        let owner = [7; 16];
        let id = MemoryId([9; 32]);
        let mut journal = DreamFreshnessJournal::new(Some(owner));
        let valid = journal
            .publication_intent_payload(id, "birth", 4, false)
            .unwrap();
        let mut trailing = valid.clone();
        trailing.push(0);
        assert!(journal.ingest(&trailing).is_err());
        assert!(journal.pending_initialization(id).is_none());
        let mut invalid_flag = valid.clone();
        *invalid_flag.last_mut().unwrap() = 2;
        assert!(journal.ingest(&invalid_flag).is_err());
        assert!(journal.pending_initialization(id).is_none());
        journal.ingest(&valid).unwrap();
        let consume = journal
            .consume_publication_intent_payload(id)
            .unwrap()
            .unwrap();
        for length in 8..consume.len() {
            assert!(journal.ingest(&consume[..length]).is_err());
            assert!(journal.pending_initialization(id).is_some());
            assert!(!journal.publication_births.contains(&id));
        }
        let mut trailing_consume = consume.clone();
        trailing_consume.push(0);
        assert!(journal.ingest(&trailing_consume).is_err());
        assert!(journal.pending_initialization(id).is_some());
        assert!(!journal.publication_births.contains(&id));
        journal.ingest(&consume).unwrap();
        assert!(journal.publication_births.contains(&id));
    }

    #[test]
    fn settlement_cannot_precede_the_original_pass() {
        let owner = [1; 16];
        let pass = InitialDreamPass {
            pass_id: "pass".into(),
            source_id: MemoryId([2; 32]),
            first_revision: 1,
            turn: 8,
            accepted_settlement_turn: None,
            preexisting_ids: BTreeSet::new(),
            graph_version_before: 0,
            roots: BTreeSet::new(),
        };
        let mut journal = DreamFreshnessJournal::new(Some(owner));
        journal.ingest_materialized_begin(owner, pass).unwrap();
        assert!(journal.accept_payload("pass", 7).is_err());
        let mut forged = header(owner, 3);
        put_bytes(&mut forged, b"pass");
        forged.extend_from_slice(&7u64.to_le_bytes());
        assert!(journal.ingest(&forged).is_err());
        assert_eq!(
            journal.pending("pass").unwrap().accepted_settlement_turn,
            None
        );
    }

    #[test]
    fn direct_birth_prefix_keeps_intent_consume_and_replay_facts_at_the_cut() {
        let owner = [4; 16];
        let id = MemoryId([5; 32]);
        let mut source = DreamFreshnessJournal::new(Some(owner));
        let intent = source
            .publication_intent_payload(id, "first-mutation", 3, false)
            .unwrap();
        source.ingest(&intent).unwrap();
        let consume = source
            .consume_publication_intent_payload(id)
            .unwrap()
            .unwrap();
        let replay = DreamFreshnessJournal::new(Some(owner))
            .replay_publication_birth_payload(id)
            .unwrap();

        let mut before_consume = DirectBirthPrefix::new(owner);
        before_consume.ingest(&intent).unwrap();
        assert_eq!(
            before_consume.pending_intents().get(&id).unwrap(),
            "first-mutation"
        );
        assert!(before_consume.consumed_intents().is_empty());
        assert!(before_consume.replay_births().is_empty());

        before_consume.ingest(&consume).unwrap();
        assert!(before_consume.pending_intents().is_empty());
        assert!(before_consume.consumed_intents()[&id].contains("first-mutation"));

        before_consume.ingest(&replay).unwrap();
        assert!(before_consume.replay_births().contains(&id));
        before_consume.ingest(&replay).unwrap();
        assert_eq!(before_consume.replay_births().len(), 1);
    }

    #[test]
    fn direct_birth_prefix_does_not_promote_a_mismatched_consume() {
        let owner = [6; 16];
        let id = MemoryId([8; 32]);
        let mut source = DreamFreshnessJournal::new(Some(owner));
        let intent = source
            .publication_intent_payload(id, "expected", 3, false)
            .unwrap();
        let mut wrong_consume = header(owner, 5);
        wrong_consume.extend_from_slice(&id.0);
        put_bytes(&mut wrong_consume, b"other");

        let mut prefix = DirectBirthPrefix::new(owner);
        prefix.ingest(&intent).unwrap();
        assert!(prefix.ingest(&wrong_consume).is_err());
        assert_eq!(prefix.pending_intents().get(&id).unwrap(), "expected");
        assert!(prefix.consumed_intents().is_empty());
    }

    #[test]
    fn direct_birth_prefix_retains_every_consumed_mutation_for_an_identity() {
        let owner = [10; 16];
        let id = MemoryId([11; 32]);
        let mut source = DreamFreshnessJournal::new(Some(owner));
        let first = source
            .publication_intent_payload(id, "accepted-first", 1, false)
            .unwrap();
        source.ingest(&first).unwrap();
        let first_receipt = source
            .consume_publication_intent_payload(id)
            .unwrap()
            .unwrap();
        source.ingest(&first_receipt).unwrap();
        let retry = source
            .publication_intent_payload(id, "later-retry", 2, false)
            .unwrap();
        source.ingest(&retry).unwrap();
        let retry_receipt = source
            .consume_publication_intent_payload(id)
            .unwrap()
            .unwrap();

        let mut prefix = DirectBirthPrefix::new(owner);
        for payload in [&first, &first_receipt, &retry, &retry_receipt] {
            prefix.ingest(payload).unwrap();
        }
        assert_eq!(prefix.consumed_intents()[&id].len(), 2);
        assert!(prefix.consumed_intents()[&id].contains("accepted-first"));
        assert!(prefix.consumed_intents()[&id].contains("later-retry"));
    }

    #[test]
    fn direct_birth_prefix_rejects_foreign_owner_and_truncation_without_facts() {
        let owner = [12; 16];
        let id = MemoryId([13; 32]);
        let foreign_payload = DreamFreshnessJournal::new(Some([14; 16]))
            .publication_intent_payload(id, "mutation", 1, false)
            .unwrap();
        let mut prefix = DirectBirthPrefix::new(owner);
        assert!(prefix.ingest(&foreign_payload).is_err());
        assert!(prefix.pending_intents().is_empty());

        let payload = DreamFreshnessJournal::new(Some(owner))
            .publication_intent_payload(id, "mutation", 1, false)
            .unwrap();
        for length in MAGIC.len()..payload.len() {
            let mut prefix = DirectBirthPrefix::new(owner);
            assert!(prefix.ingest(&payload[..length]).is_err());
            assert!(prefix.pending_intents().is_empty());
            assert!(prefix.consumed_intents().is_empty());
            assert!(prefix.replay_births().is_empty());
        }
    }
    #[test]
    fn graph_cut_wire_and_replay_preserve_proof_and_reject_conflicts() {
        let owner = [1; 16];
        let mut journal = DreamFreshnessJournal::new(Some(owner));
        journal
            .ingest_materialized_begin(
                owner,
                InitialDreamPass {
                    pass_id: "p".into(),
                    source_id: MemoryId([2; 32]),
                    first_revision: 1,
                    turn: 3,
                    accepted_settlement_turn: None,
                    preexisting_ids: BTreeSet::new(),
                    graph_version_before: 4,
                    roots: BTreeSet::new(),
                },
            )
            .unwrap();
        let proof = AcceptedGraphCutProof {
            graph_version: 7,
            community_generation: Some(9),
            community_graph_version: Some(7),
        };
        let bytes = journal.accept_graph_payload("p", 5, 6, proof).unwrap();
        let mut golden = b"CVDREAM1".to_vec();
        golden.extend_from_slice(&owner);
        golden.push(8);
        golden.extend_from_slice(&1u32.to_le_bytes());
        golden.push(b'p');
        golden.extend_from_slice(&5u64.to_le_bytes());
        golden.extend_from_slice(&6u64.to_le_bytes());
        golden.extend_from_slice(&7u64.to_le_bytes());
        golden.push(1);
        golden.extend_from_slice(&9u64.to_le_bytes());
        golden.extend_from_slice(&7u64.to_le_bytes());
        assert_eq!(bytes, golden);
        let entry = journal.replay_entry(&bytes).unwrap().unwrap();
        assert_eq!(journal.encode_replay_entry(entry).unwrap(), bytes);
        let mut invalid = bytes.clone();
        let n = invalid.len();
        invalid[n - 8..].copy_from_slice(&8u64.to_le_bytes());
        assert!(journal.ingest(&invalid).is_err());
        assert_eq!(journal.pending("p").unwrap().accepted_settlement_turn, None);
        journal.ingest(&bytes).unwrap();
        journal.ingest(&bytes).unwrap();
        assert_eq!(journal.accepted_graph_cut("p"), Some((6, proof)));
        let mut conflict = bytes.clone();
        let n = conflict.len();
        conflict[n - 16..n - 8].copy_from_slice(&10u64.to_le_bytes());
        assert!(journal.ingest(&conflict).is_err());
        assert_eq!(journal.accepted_graph_cut("p"), Some((6, proof)));
    }
}
