//! Single current semantic settlement wire format; activation follows C3 verification.
use super::compact_settlement_wire::{
    CompactReader, encoded_record, put_count, put_signed, put_var,
};
use super::semantic_settlement::{
    AcceptedImportEventEvidence, ImportSemanticScope, SemanticImportFreshnessSettlement,
};
use crate::MemoryId;
use crate::freshness::historical_causal_basis::{CausalContribution, HistoricalCausalBasis};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
pub(super) const MAGIC: &[u8; 8] = b"CVAFRS07";
const DOMAIN: &[u8] = b"reliquary-semantic-import-settlement-v7";

// Prefix dictionaries and repeated text references must not amplify a small
// frame into unbounded owned UTF-8 allocations. Count exact semantic copies.
const MAX_EXPANDED_TEXT_BYTES: usize = 64 * 1024 * 1024;
struct TextBudget {
    remaining: usize,
}
impl TextBudget {
    fn new() -> Self {
        Self {
            remaining: MAX_EXPANDED_TEXT_BYTES,
        }
    }
    fn charge(&mut self, bytes: usize) -> Result<(), String> {
        self.remaining = self
            .remaining
            .checked_sub(bytes)
            .ok_or("compact expanded text limit exceeded")?;
        Ok(())
    }
}

fn put_set(
    out: &mut Vec<u8>,
    values: &BTreeSet<MemoryId>,
    indexes: &BTreeMap<MemoryId, usize>,
) -> Result<(), String> {
    put_count(out, values.len())?;
    let mut previous = None;
    for value in values {
        let index = *indexes
            .get(value)
            .ok_or("missing compact Memory dictionary entry")?;
        let delta = previous.map_or(index + 1, |old| index - old);
        put_var(out, delta as u64);
        previous = Some(index);
    }
    Ok(())
}

pub(super) fn encode(
    value: &SemanticImportFreshnessSettlement,
    basis: &HistoricalCausalBasis,
) -> Result<Vec<u8>, String> {
    value.validate()?;
    if basis.reconstruct(value.scope.settlement_cut)? != value.records {
        return Err("causal basis does not reproduce accepted settlement".into());
    }
    let s = &value.scope;
    let record_keys: BTreeSet<_> = value.records.keys().copied().collect();
    if basis.initial().keys().copied().collect::<BTreeSet<_>>() != record_keys {
        return Err("causal initial/output key mismatch".into());
    }
    let ids: BTreeSet<_> = s
        .affected_recipients
        .iter()
        .chain(s.born_memories.iter())
        .chain(value.cohort.iter())
        .chain(record_keys.iter())
        .copied()
        .collect();
    let indexes: BTreeMap<_, _> = ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
    let texts: BTreeSet<_> = s
        .source_nodes
        .iter()
        .flat_map(|(c, n)| [c.as_str(), n.as_str()])
        .chain(s.accepted_events.keys().map(String::as_str))
        .collect();
    let mut text_budget = TextBudget::new();
    for text in &texts {
        text_budget.charge(text.len())?;
    }
    for (conversation, node) in &s.source_nodes {
        text_budget.charge(conversation.len())?;
        text_budget.charge(node.len())?;
    }
    for id in s.accepted_events.keys() {
        text_budget.charge(id.len())?;
        text_budget.charge(id.len())?;
    }
    for id in &s.superseded {
        text_budget.charge(id.len())?;
    }
    let text_indexes: BTreeMap<_, _> = texts
        .iter()
        .enumerate()
        .map(|(i, &text)| (text, i))
        .collect();
    let sets: BTreeSet<_> = [
        s.born_memories.clone(),
        s.affected_recipients.clone(),
        value.cohort.clone(),
        record_keys.clone(),
    ]
    .into_iter()
    .collect();
    let set_indexes: BTreeMap<_, _> = sets.iter().enumerate().map(|(i, set)| (set, i)).collect();
    let mut out = MAGIC.to_vec();
    out.extend(s.owner);
    for digest in [
        s.import_id,
        s.start_evidence_digest,
        value.corpus_digest,
        value.manifest_digest,
        value.input_digest,
        crate::freshness::historical_contract::HISTORICAL_POLICY_DIGEST,
        crate::freshness::semantic_policy::SemanticPolicy::default()
            .digest()
            .map_err(|e| e.to_string())?,
    ] {
        out.extend(digest);
    }
    for cut in [
        s.base_cut,
        s.settlement_cut,
        value.global_version,
        value.archive_version,
        value.graph_version,
        value.memory_graph_version,
    ] {
        out.extend(cut.to_le_bytes());
    }
    out.push(0);
    put_count(&mut out, ids.len())?;
    for id in &ids {
        out.extend(id.0);
    }
    put_count(&mut out, texts.len())?;
    let mut previous: &[u8] = &[];
    for text in &texts {
        let bytes = text.as_bytes();
        let prefix = previous
            .iter()
            .zip(bytes)
            .take_while(|(a, b)| a == b)
            .count();
        put_count(&mut out, prefix)?;
        put_count(&mut out, bytes.len() - prefix)?;
        out.extend(&bytes[prefix..]);
        previous = bytes;
    }
    put_count(&mut out, sets.len())?;
    for set in &sets {
        put_set(&mut out, set, &indexes)?;
    }
    put_var(&mut out, set_indexes[&s.born_memories] as u64);
    put_var(&mut out, set_indexes[&s.affected_recipients] as u64);
    put_count(&mut out, s.source_nodes.len())?;
    for (c, n) in &s.source_nodes {
        put_var(&mut out, text_indexes[c.as_str()] as u64);
        put_var(&mut out, text_indexes[n.as_str()] as u64);
    }
    put_count(&mut out, s.accepted_events.len())?;
    for event in s.accepted_events.values() {
        put_var(&mut out, text_indexes[event.event_id.as_str()] as u64);
        out.extend(event.payload_digest);
        out.extend(event.accepted_turn.to_le_bytes());
    }
    let mut bits = vec![0; (s.accepted_events.len() + 7) / 8];
    for (i, id) in s.accepted_events.keys().enumerate() {
        if s.superseded.contains(id) {
            bits[i / 8] |= 1 << (i % 8);
        }
    }
    out.extend(bits);
    put_var(&mut out, set_indexes[&value.cohort] as u64);
    put_var(&mut out, set_indexes[&record_keys] as u64);
    let anchors: BTreeSet<_> = basis
        .initial()
        .values()
        .map(|&record| encoded_record(record))
        .collect();
    let anchor_indexes: BTreeMap<_, _> = anchors.iter().enumerate().map(|(i, a)| (a, i)).collect();
    put_count(&mut out, anchors.len())?;
    for anchor in &anchors {
        put_count(&mut out, anchor.len())?;
        out.extend(anchor);
    }
    put_count(&mut out, record_keys.len())?;
    for key in &record_keys {
        put_var(
            &mut out,
            anchor_indexes[&encoded_record(basis.initial()[key])] as u64,
        );
    }
    put_count(&mut out, basis.contributions().len())?;
    let mut previous = 0;
    for contribution in basis.contributions() {
        put_var(&mut out, contribution.position - previous);
        previous = contribution.position;
        put_count(&mut out, contribution.effects.len())?;
        let mut previous = None;
        for (recipient, &amount) in &contribution.effects {
            let index = *indexes
                .get(recipient)
                .ok_or("unknown causal wire recipient")?;
            put_var(
                &mut out,
                previous.map_or(index + 1, |old| index - old) as u64,
            );
            previous = Some(index);
            put_signed(&mut out, amount);
        }
    }
    put_count(&mut out, basis.corrections().len())?;
    let mut previous = None;
    for (recipient, &amount) in basis.corrections() {
        let index = *indexes
            .get(recipient)
            .ok_or("unknown correction wire recipient")?;
        put_var(
            &mut out,
            previous.map_or(index + 1, |old| index - old) as u64,
        );
        previous = Some(index);
        put_signed(&mut out, amount);
    }
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update(&out);
    out.extend(hash.finalize());
    Ok(out)
}

/// Unresolved frames never escape as accepted semantic settlements. Only the
/// Freshness owner can supply independently decoded retained journal leaves.
pub(super) struct PendingSettlement {
    value: SemanticImportFreshnessSettlement,
    basis: HistoricalCausalBasis,
}
impl PendingSettlement {
    pub(super) fn event_references(&self) -> &BTreeMap<String, AcceptedImportEventEvidence> {
        &self.value.scope.accepted_events
    }
    pub(super) fn binding(&self) -> super::import_settlement_storage::SettlementBinding {
        super::import_settlement_storage::SettlementBinding {
            owner: self.value.scope.owner,
            import_id: self.value.scope.import_id,
            start_evidence_digest: self.value.scope.start_evidence_digest,
            global_version: self.value.global_version,
            archive_version: self.value.archive_version,
            graph_version: self.value.graph_version,
            memory_graph_version: self.value.memory_graph_version,
        }
    }

    pub(super) fn resolve(
        mut self,
        leaves: &BTreeMap<String, AcceptedImportEventEvidence>,
    ) -> Result<(SemanticImportFreshnessSettlement, HistoricalCausalBasis), String> {
        for (id, event) in &mut self.value.scope.accepted_events {
            let leaf = leaves
                .get(id)
                .ok_or("missing retained settlement event leaf")?;
            if leaf.event_id != *id
                || leaf.payload_digest != event.payload_digest
                || leaf.accepted_turn != event.accepted_turn
            {
                return Err("changed retained settlement event leaf".into());
            }
            event.affected_recipients = leaf.affected_recipients.clone();
        }
        self.value.validate()?;
        Ok((self.value, self.basis))
    }
}

#[path = "compact_settlement_decode.rs"]
mod decoding;
pub(super) use decoding::decode;

#[cfg(test)]
#[path = "compact_settlement_tests.rs"]
mod tests;
