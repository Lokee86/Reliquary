//! Bounded structural parsing for CVAFRS07; retained leaf resolution is separate.
use super::*;

fn increasing_index(
    r: &mut CompactReader<'_>,
    previous: &mut Option<usize>,
    count: usize,
) -> Result<usize, String> {
    let delta = usize::try_from(r.var()?).map_err(|_| "compact ordinal overflow")?;
    if delta == 0 {
        return Err("zero compact ordinal delta".into());
    }
    let index = match previous {
        Some(old) => old.checked_add(delta).ok_or("compact ordinal overflow")?,
        None => delta - 1,
    };
    if index >= count {
        return Err("compact ordinal outside dictionary".into());
    }
    *previous = Some(index);
    Ok(index)
}
fn ordered<T: Ord>(values: &[T], value: &T) -> Result<(), String> {
    if values.last().is_some_and(|old| old >= value) {
        return Err("unordered or duplicate compact dictionary".into());
    }
    Ok(())
}
pub(crate) fn decode(payload: &[u8]) -> Result<PendingSettlement, String> {
    if payload.len() < 329 || !payload.starts_with(MAGIC) {
        return Err("invalid compact settlement framing".into());
    }
    let end = payload.len() - 32;
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update(&payload[..end]);
    if hash.finalize().as_slice() != &payload[end..] {
        return Err("compact settlement checksum mismatch".into());
    }
    let mut r = CompactReader::new(&payload[..end]);
    r.take(8)?;
    let owner = r.array()?;
    let import_id = r.array()?;
    let start_evidence_digest = r.array()?;
    let corpus_digest = r.array()?;
    let manifest_digest = r.array()?;
    let input_digest = r.array()?;
    if r.array::<32>()? != crate::freshness::historical_contract::HISTORICAL_POLICY_DIGEST
        || r.array::<32>()?
            != crate::freshness::semantic_policy::SemanticPolicy::default()
                .digest()
                .map_err(|e| e.to_string())?
    {
        return Err("unsupported compact settlement policy".into());
    }
    let base_cut = r.u64()?;
    let settlement_cut = r.u64()?;
    let global_version = r.u64()?;
    let archive_version = r.u64()?;
    let graph_version = r.u64()?;
    let memory_graph_version = r.u64()?;
    if r.take(1)?[0] != 0 {
        return Err("invalid compact reserved tag".into());
    }
    let mut ids = Vec::new();
    for _ in 0..r.count(32)? {
        let id = MemoryId(r.array()?);
        ordered(&ids, &id)?;
        ids.push(id);
    }
    let mut texts = Vec::<String>::new();
    for _ in 0..r.count(2)? {
        let prefix = usize::try_from(r.var()?).map_err(|_| "compact text prefix overflow")?;
        let previous = texts.last().map_or(&[][..], |v| v.as_bytes());
        let prefix_bytes = previous
            .get(..prefix)
            .ok_or("compact text prefix outside predecessor")?;
        let suffix = r.count(1)?;
        let suffix = r.take(suffix)?;
        if prefix < previous.len() && suffix.first().is_some_and(|b| *b == previous[prefix]) {
            return Err("nonmaximal compact text prefix".into());
        }
        let mut text = prefix_bytes.to_vec();
        text.extend(suffix);
        let text = String::from_utf8(text).map_err(|_| "invalid compact UTF-8")?;
        ordered(&texts, &text)?;
        texts.push(text);
    }
    let mut sets = Vec::new();
    for _ in 0..r.count(1)? {
        let mut set = BTreeSet::new();
        let mut previous = None;
        for _ in 0..r.count(1)? {
            let index = increasing_index(&mut r, &mut previous, ids.len())?;
            set.insert(ids[index]);
        }
        ordered(&sets, &set)?;
        sets.push(set);
    }
    let born_memories = sets[r.index(sets.len())?].clone();
    let affected_recipients = sets[r.index(sets.len())?].clone();
    let mut source_nodes = BTreeSet::new();
    for _ in 0..r.count(2)? {
        let pair = (
            texts[r.index(texts.len())?].clone(),
            texts[r.index(texts.len())?].clone(),
        );
        if source_nodes.last().is_some_and(|old| old >= &pair) {
            return Err("unordered compact source identities".into());
        }
        source_nodes.insert(pair);
    }
    let mut accepted_events = BTreeMap::new();
    for _ in 0..r.count(41)? {
        let event_id = texts[r.index(texts.len())?].clone();
        if accepted_events
            .last_key_value()
            .is_some_and(|(old, _)| old >= &event_id)
        {
            return Err("unordered compact accepted event identities".into());
        }
        let event = AcceptedImportEventEvidence {
            event_id: event_id.clone(),
            payload_digest: r.array()?,
            accepted_turn: r.u64()?,
            affected_recipients: BTreeSet::new(),
        };
        accepted_events.insert(event_id, event);
    }
    let count = accepted_events.len();
    let bits = r.take((count + 7) / 8)?;
    if count % 8 != 0 && bits.last().is_some_and(|b| *b >> (count % 8) != 0) {
        return Err("nonzero compact bitmap padding".into());
    }
    let superseded = accepted_events
        .keys()
        .enumerate()
        .filter(|(i, _)| bits[i / 8] & (1 << (i % 8)) != 0)
        .map(|(_, id)| id.clone())
        .collect();
    let cohort = sets[r.index(sets.len())?].clone();
    let record_keys = sets[r.index(sets.len())?].clone();
    let mut anchors = Vec::new();
    let mut previous_anchor: Option<Vec<u8>> = None;
    for _ in 0..r.count(5)? {
        let anchor = r.record()?;
        let encoded = encoded_record(anchor);
        if previous_anchor.as_ref().is_some_and(|old| old >= &encoded) {
            return Err("unordered compact anchor dictionary".into());
        }
        previous_anchor = Some(encoded);
        anchors.push(anchor);
    }
    if r.count(1)? != record_keys.len() {
        return Err("compact anchor/key count mismatch".into());
    }
    let mut initial = BTreeMap::new();
    for id in record_keys {
        initial.insert(id, anchors[r.index(anchors.len())?]);
    }
    let mut contributions = Vec::new();
    let mut position = 0u64;
    for _ in 0..r.count(4)? {
        position = position
            .checked_add(r.var()?)
            .ok_or("compact position overflow")?;
        let mut effects = BTreeMap::new();
        let mut previous = None;
        for _ in 0..r.count(2)? {
            let index = increasing_index(&mut r, &mut previous, ids.len())?;
            let amount = r.signed()?;
            effects.insert(ids[index], amount);
        }
        contributions.push(CausalContribution { position, effects });
    }
    let mut corrections = BTreeMap::new();
    let mut previous = None;
    for _ in 0..r.count(2)? {
        let index = increasing_index(&mut r, &mut previous, ids.len())?;
        corrections.insert(ids[index], r.signed()?);
    }
    r.finish()?;
    let basis =
        HistoricalCausalBasis::from_canonical(initial, contributions, corrections, settlement_cut)?;
    let records = basis.reconstruct(settlement_cut)?;
    let value = SemanticImportFreshnessSettlement {
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
    value.validate()?;
    Ok(PendingSettlement {
        value,
        basis,
        raw: payload.to_vec(),
    })
}
