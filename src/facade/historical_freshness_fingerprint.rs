//! Activity and exact stable-event cuts of the accepted Freshness owner.
use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::{Cva, FreshnessEventCut, MemoryId, freshness::FreshnessRecord};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[path = "historical_freshness_values.rs"]
mod values;

pub(crate) struct FreshnessCut {
    pub(crate) fingerprint: HistoricalFingerprint,
    pub(crate) records: BTreeMap<MemoryId, FreshnessRecord>,
}

pub(crate) fn freshness_at_turn(cva: &Cva, turn: u64) -> Result<FreshnessCut, String> {
    project(
        cva,
        &FreshnessEventCut {
            rel_turn: turn,
            event_id_inclusive: None,
            graph_version: u64::MAX,
        },
        true,
    )
}

pub(crate) fn freshness_at_event(
    cva: &Cva,
    cut: &FreshnessEventCut,
) -> Result<FreshnessCut, String> {
    if cut.graph_version > cva.memory_graph_version() {
        return Err("Freshness graph cut exceeds accepted head".into());
    }
    if cut
        .event_id_inclusive
        .as_ref()
        .is_some_and(|id| id.trim().is_empty())
    {
        return Err("Freshness event cut is blank".into());
    }
    project(cva, cut, false)
}

fn project(
    cva: &Cva,
    cut: &FreshnessEventCut,
    activity_only: bool,
) -> Result<FreshnessCut, String> {
    if cut.rel_turn > cva.rel_turn_count() {
        return Err("Freshness activity cut exceeds accepted head".into());
    }
    let state = cva.freshness.historical_oracle_state(cut)?;
    let crate::freshness_storage::FreshnessOracleState {
        policy,
        records,
        entries,
        settlements,
        activation,
    } = state;
    let mut projection = OwnerProjection::new(
        cva.owner_uuid()
            .ok_or("Freshness oracle needs owner UUID")?,
        "freshness",
        cut.rel_turn,
        None,
    )?;
    // Stable string identities have no numeric event clock. Bind the exact cut
    // explicitly instead of synthesizing an ordinal for schema 1's event slot.
    projection.insert(
        "cut",
        b"cut".to_vec(),
        json!({"mode": if activity_only {"activity"} else {"event"},
        "rel_turn": cut.rel_turn, "event_id_inclusive": &cut.event_id_inclusive,
        "graph_version": if activity_only {None} else {Some(cut.graph_version)}}),
    )?;
    projection.insert("policy", b"policy".to_vec(), values::policy_value(&policy))?;
    projection.insert("activation", b"current".to_vec(), activation)?;
    for (id, record) in &records {
        projection.insert("record", id.0, values::record_value(record))?;
    }
    let mut evidence = BTreeMap::<(String, Vec<u8>), Value>::new();
    for entry in entries {
        let (family, key, value) = values::entry_value(&entry);
        if let Some(previous) = evidence.insert((family, key), value.clone()) {
            if previous != value {
                return Err("conflicting Freshness evidence identity".into());
            }
        }
    }
    for ((family, key), value) in evidence {
        projection.insert(&family, key, value)?;
    }
    for settlement in settlements {
        let mut scope = cva
            .semantic_scope_for_import_settlement(&settlement)
            .map_err(|e| e.to_string())?;
        // Predecessors authenticate compatibility carriers separately; a
        // normalization must not change the accepted semantic fingerprint.
        let canonical =
            crate::freshness_storage::HistoricalImportFreshnessSettlement::from_semantic(
                settlement.as_semantic(scope),
            );
        projection.insert(
            "import_settlement",
            canonical.import_id,
            values::settlement_value(&canonical),
        )?;
    }
    Ok(FreshnessCut {
        fingerprint: projection.fingerprint()?,
        records,
    })
}

#[cfg(test)]
#[path = "historical_freshness_fingerprint_tests.rs"]
mod tests;
