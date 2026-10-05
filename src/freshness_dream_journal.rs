use crate::MemoryId;
use std::collections::{BTreeMap, BTreeSet};

const MAGIC: &[u8; 8] = b"CVDREAM1";

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
pub(crate) struct DreamFreshnessJournal {
    owner_uuid: Option<[u8; 16]>,
    pending: BTreeMap<String, InitialDreamPass>,
    settled: BTreeSet<String>,
    pending_initializations: BTreeMap<MemoryId, (String, u64, bool)>,
    publication_births: BTreeSet<MemoryId>,
    accepted_graph_cuts: BTreeMap<String, (u64, crate::FreshnessEventProof)>,
}

impl DreamFreshnessJournal {
    pub(crate) fn new(owner_uuid: Option<[u8; 16]>) -> Self {
        Self {
            owner_uuid,
            ..Self::default()
        }
    }

    pub(crate) fn bind_owner(&mut self, owner: Option<[u8; 16]>) -> Result<(), String> {
        if self.owner_uuid.is_some() && self.owner_uuid != owner {
            return Err("Dream Freshness pass belongs to another owner".into());
        }
        if !self.pending.is_empty() && owner.is_none() {
            return Err("legacy owner cannot contain Dream Freshness passes".into());
        }
        self.owner_uuid = owner;
        Ok(())
    }

    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), String> {
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
            DreamEntry::Begin(pass) => match self.pending.get(&pass.pass_id) {
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
            },
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

    pub(crate) fn begin_payload(&self, pass: &InitialDreamPass) -> Result<Vec<u8>, String> {
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
                return Ok(Vec::new());
            }
            return Err("Dream Freshness pass ID collision".into());
        }
        let owner = self
            .owner_uuid
            .ok_or("legacy owner requires explicit Freshness migration")?;
        let mut out = header(owner, 0);
        put_bytes(&mut out, pass.pass_id.as_bytes());
        out.extend_from_slice(&pass.source_id.0);
        out.extend_from_slice(&pass.first_revision.to_le_bytes());
        out.extend_from_slice(&pass.turn.to_le_bytes());
        out.extend_from_slice(&pass.graph_version_before.to_le_bytes());
        put_ids(&mut out, &pass.preexisting_ids);
        Ok(out)
    }

    pub(crate) fn roots_payload(
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

    pub(crate) fn accept_payload(&self, pass_id: &str, turn: u64) -> Result<Vec<u8>, String> {
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

    pub(crate) fn accepted_graph_cut(
        &self,
        pass_id: &str,
    ) -> Option<(u64, crate::FreshnessEventProof)> {
        self.accepted_graph_cuts.get(pass_id).copied()
    }

    pub(crate) fn accept_graph_payload(
        &self,
        pass_id: &str,
        turn: u64,
        graph: u64,
        proof: crate::FreshnessEventProof,
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

    pub(crate) fn settled_payload(&self, pass_id: &str) -> Result<Vec<u8>, String> {
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

    pub(crate) fn pending_for_source(&self, source: MemoryId) -> Option<InitialDreamPass> {
        self.pending
            .values()
            .find(|pass| pass.source_id == source)
            .cloned()
    }
    pub(crate) fn pending(&self, pass_id: &str) -> Option<InitialDreamPass> {
        self.pending.get(pass_id).cloned()
    }
    pub(crate) fn is_settled(&self, pass_id: &str) -> bool {
        self.settled.contains(pass_id)
    }
    pub(crate) fn has_pending(&self) -> bool {
        !self.pending.is_empty() || !self.pending_initializations.is_empty()
    }
    pub(crate) fn settled_ids(&self) -> impl Iterator<Item = &str> {
        self.settled.iter().map(String::as_str)
    }
    pub(crate) fn replay_settled_payload(&self, pass_id: &str) -> Result<Vec<u8>, String> {
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
    pub(crate) fn is_publication_birth(&self, id: MemoryId) -> bool {
        self.pending_initializations.contains_key(&id) || self.publication_births.contains(&id)
    }
    pub(crate) fn publication_birth_ids(&self) -> Vec<MemoryId> {
        self.publication_births.iter().copied().collect()
    }
    pub(crate) fn replay_publication_birth_payload(&self, id: MemoryId) -> Result<Vec<u8>, String> {
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
    pub(crate) fn has_any(&self) -> bool {
        !self.pending.is_empty()
            || !self.settled.is_empty()
            || !self.pending_initializations.is_empty()
            || !self.publication_births.is_empty()
    }
    pub(crate) fn pending_initializations(&self) -> Vec<(MemoryId, String, u64, bool)> {
        self.pending_initializations
            .iter()
            .map(|(id, (mutation, turn, admitted))| (*id, mutation.clone(), *turn, *admitted))
            .collect()
    }
    pub(crate) fn derive_publication_birth(&mut self, id: MemoryId) {
        self.publication_births.insert(id);
    }
    pub(crate) fn pending_initialization(&self, id: MemoryId) -> Option<(String, u64, bool)> {
        self.pending_initializations.get(&id).cloned()
    }

    pub(crate) fn publication_intent_payload(
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

    pub(crate) fn consume_publication_intent_payload(
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

enum DreamEntry {
    Begin(InitialDreamPass),
    Roots(String, BTreeSet<MemoryId>),
    Settled(String),
    Accept(String, u64),
    AcceptGraph(String, u64, u64, crate::FreshnessEventProof),
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
        0 => DreamEntry::Begin(r.pass()?),
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
                crate::FreshnessEventProof {
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
    fn pass(&mut self) -> Result<InitialDreamPass, String> {
        let pass_id =
            String::from_utf8(self.bytes()?).map_err(|_| "invalid Dream pass ID UTF-8")?;
        let source_id = MemoryId(self.array()?);
        let first_revision = self.u64()?;
        let turn = self.u64()?;
        let graph_version_before = self.u64()?;
        let preexisting_ids = self.ids()?;
        if pass_id.is_empty() || first_revision != 1 || preexisting_ids.contains(&source_id) {
            return Err("invalid initial Dream Freshness pass".into());
        }
        Ok(InitialDreamPass {
            pass_id,
            source_id,
            first_revision,
            turn,
            accepted_settlement_turn: None,
            preexisting_ids,
            graph_version_before,
            roots: BTreeSet::new(),
        })
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
        journal
            .ingest(&journal.begin_payload(&pass).unwrap())
            .unwrap();
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
}
