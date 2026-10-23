use crate::interaction_stream_error::InteractionStreamError;
use crate::interaction_stream_store::InteractionStreamStore;
use crate::{Container, InteractionStreamRecord, InteractionStreamStatus};
use std::collections::HashMap;

pub(crate) fn merge_interaction_streams(
    left: Vec<InteractionStreamRecord>,
    right: Vec<InteractionStreamRecord>,
) -> Result<(Vec<InteractionStreamRecord>, bool), InteractionStreamError> {
    let left_map = keyed(left);
    let mut merged = left_map.clone();
    for incoming in right {
        match merged.get(&incoming.message_id) {
            None => {
                merged.insert(incoming.message_id.clone(), incoming);
            }
            Some(current) => {
                let next = merge_record(current, &incoming)?;
                merged.insert(next.message_id.clone(), next);
            }
        }
    }
    let changed = merged != left_map;
    let mut records = merged.into_values().collect::<Vec<_>>();
    records.sort_by(|a, b| {
        a.session_id
            .cmp(&b.session_id)
            .then(a.timestamp_ns.cmp(&b.timestamp_ns))
            .then(a.message_id.cmp(&b.message_id))
    });
    Ok((records, changed))
}

pub(crate) fn replay_interaction_streams(
    store: &mut InteractionStreamStore,
    container: &mut Container,
    records: Vec<InteractionStreamRecord>,
) -> Result<(), InteractionStreamError> {
    for record in records {
        store.put(container, record)?;
    }
    Ok(())
}

fn keyed(records: Vec<InteractionStreamRecord>) -> HashMap<String, InteractionStreamRecord> {
    records
        .into_iter()
        .map(|record| (record.message_id.clone(), record))
        .collect()
}

fn merge_record(
    left: &InteractionStreamRecord,
    right: &InteractionStreamRecord,
) -> Result<InteractionStreamRecord, InteractionStreamError> {
    if left.session_id != right.session_id
        || left.parent_message_id != right.parent_message_id
        || left.role != right.role
        || left.timestamp_ns != right.timestamp_ns
    {
        return Err(stream_conflict(
            "interaction stream metadata diverged during reconciliation",
        ));
    }
    let mut merged = if left.content.len() >= right.content.len() {
        if !left.content.starts_with(&right.content) {
            return Err(stream_conflict(
                "interaction stream text diverged during reconciliation",
            ));
        }
        left.clone()
    } else {
        if !right.content.starts_with(&left.content) {
            return Err(stream_conflict(
                "interaction stream text diverged during reconciliation",
            ));
        }
        right.clone()
    };
    if left.status == InteractionStreamStatus::Interrupted
        || right.status == InteractionStreamStatus::Interrupted
    {
        merged.status = InteractionStreamStatus::Interrupted;
    }
    Ok(merged)
}

fn stream_conflict(message: &str) -> InteractionStreamError {
    InteractionStreamError::Invalid(message.into())
}
