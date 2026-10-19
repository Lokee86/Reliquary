//! Deterministic test-only enumeration of valid historical owner cut positions.

use crate::{Cva, FreshnessEventCut, Phylactery};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HistoricalCutEnumeration {
    /// REL-only owner-local clocks are `None` for PHY.
    pub(crate) archive: Option<Vec<u64>>,
    pub(crate) memory: Vec<u64>,
    pub(crate) entity: Vec<u64>,
    pub(crate) relationship: Vec<u64>,
    pub(crate) graph: Vec<u64>,
    /// Full-Graph positions whose publication actually advances MemoryGraph.
    pub(crate) memory_graph: Vec<u64>,
    /// REL Vector Generation owner; PHY has no generation store.
    pub(crate) vector_generation: Option<Vec<u64>>,
    pub(crate) ego: Vec<u64>,
    /// Freshness is REL-only; each accepted stable event ID contributes its
    /// exact inclusive prefix at that event's proof graph cut. This is not a
    /// Cartesian product of event IDs and every possible Graph position.
    pub(crate) freshness_activity: Option<Vec<u64>>,
    pub(crate) freshness_events: Option<Vec<FreshnessEventCut>>,
    /// Global ordering positions, including the empty prefix at zero.
    pub(crate) global: Vec<u64>,
}

pub(crate) fn reliquary_cuts(cva: &mut Cva) -> Result<HistoricalCutEnumeration, String> {
    let archive_head = cva.archive_version();
    let memory_head = cva.memory_version();
    let entity_head = cva.entity_version();
    let relationship_head = cva.relationship_version();
    let vector_head = cva.vector_version();
    let ego_head = cva.ego_version();
    let graph_history =
        crate::historical_graph_fingerprint::GraphHistory::read(&cva.container, &cva.graph)?;
    let graph_head = cva.graph_version();
    let memory_graph = graph_history.memory_graph_positions();
    let freshness_events = freshness_event_cuts(cva)?;
    Ok(HistoricalCutEnumeration {
        archive: Some(positions(archive_head)),
        memory: positions(memory_head),
        entity: positions(entity_head),
        relationship: positions(relationship_head),
        graph: positions(graph_head),
        memory_graph,
        vector_generation: Some(positions(vector_head)),
        ego: positions(ego_head),
        freshness_activity: Some(positions(cva.rel_turn_count())),
        freshness_events: Some(freshness_events),
        global: positions(cva.latest_global_version()),
    })
}

pub(crate) fn phylactery_cuts(phy: &Phylactery) -> Result<HistoricalCutEnumeration, String> {
    let graph_history =
        crate::historical_graph_fingerprint::GraphHistory::read(&phy.container, &phy.graph)?;
    let graph_head = phy.graph_version();
    Ok(HistoricalCutEnumeration {
        archive: None,
        memory: positions(phy.memory_version()),
        entity: positions(phy.entity_version()),
        relationship: positions(phy.relationship_version()),
        graph: positions(graph_head),
        memory_graph: graph_history.memory_graph_positions(),
        vector_generation: None,
        ego: positions(phy.ego_version()),
        freshness_activity: None,
        freshness_events: None,
        global: positions(phy.latest_global_version()),
    })
}

fn positions(head: u64) -> Vec<u64> {
    (0..=head).collect()
}

fn freshness_event_cuts(cva: &Cva) -> Result<Vec<FreshnessEventCut>, String> {
    let mut events = std::collections::BTreeMap::<
        (u64, String),
        (
            FreshnessEventCut,
            crate::freshness_storage::FreshnessReplayEntry,
        ),
    >::new();
    for entry in cva.freshness.retained_history_entries()? {
        if let crate::freshness_storage::FreshnessReplayEntry::Event {
            event_id,
            source_turn,
            proof,
            ..
        } = &entry
        {
            if *source_turn > cva.rel_turn_count()
                || proof.graph_version > cva.memory_graph_version()
            {
                return Err("accepted Freshness event exceeds retained owner heads".into());
            }
            let cut = FreshnessEventCut {
                rel_turn: *source_turn,
                event_id_inclusive: Some(event_id.clone()),
                graph_version: proof.graph_version,
            };
            let key = (*source_turn, event_id.clone());
            if let Some((_, previous)) = events.get(&key) {
                // Match the canonical ordered-event behavior: identical accepted
                // receipts collapse; conflicting contents for one identity fail.
                if previous != &entry {
                    return Err("conflicting accepted Freshness event identity".into());
                }
            } else {
                events.insert(key, (cut, entry));
            }
        }
    }
    Ok(events.into_values().map(|(cut, _)| cut).collect())
}

#[path = "historical_cut_enumeration_tests.rs"]
mod tests;
