//! Validation-only observation of the retained Insomnia and import event journal.
use super::{MAX_CLOCK_STATE_ENTRIES, MAX_FRAME_PAYLOAD_BYTES, sha256_bytes};
use crate::container::grouped_clock;
use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::imports::epoch_model::ImportEpochStatus;
use crate::insomnia::codec::{InsomniaRecord, decode_format, decode_record};
use crate::insomnia::completion::{InsomniaCompletion, decode_completion};
use crate::{Cva, InsomniaAttempt, InsomniaWork, MemoryRef};
use serde_json::{Value, json};

fn references(values: &[MemoryRef]) -> Vec<Value> {
    values
        .iter()
        .map(|reference| {
            json!({
                "owner_id": reference.owner_id,
                "memory_id": reference.memory_id.0,
            })
        })
        .collect()
}

fn work_value(work: InsomniaWork) -> Value {
    json!({
        "episode_id": work.episode_id.0,
        "priority": work.priority.tag(),
        "state": work.state.tag(),
        "attempt_count": work.attempt_count,
        "lease_owner": work.lease_owner,
        "lease_token": work.lease_token.map(|token| token.0),
        "lease_expires_ns": work.lease_expires_ns,
        "retry_after_ns": work.retry_after_ns,
        "last_error": work.last_error,
        "updated_at_ns": work.updated_at_ns,
    })
}

fn attempt_value(attempt: InsomniaAttempt) -> Value {
    json!({
        "episode_id": attempt.episode_id.0,
        "attempt": attempt.attempt,
        "state": attempt.state.tag(),
        "started_at_ns": attempt.started_at_ns,
        "completed_at_ns": attempt.completed_at_ns,
        "extractor_model": attempt.extractor_model,
        "extractor_version": attempt.extractor_version,
        "memory_ids": attempt.memory_ids.iter().map(|id| id.0).collect::<Vec<_>>(),
        "external_memory_refs": references(&attempt.external_memory_refs),
        "rejected_count": attempt.rejected_count,
        "error": attempt.error,
    })
}

fn completion_value(completion: InsomniaCompletion, payload: &[u8]) -> Result<Value, String> {
    let range = grouped_clock::insomnia_range(payload)
        .map_err(|_| "retained Insomnia grouped clock is invalid")?;
    let bodies = completion
        .bodies
        .iter()
        .map(|body| {
            json!({
                "body_id": body.id.0,
                "bytes": body.bytes.len(),
                "body_sha256": sha256_bytes(&body.bytes),
            })
        })
        .collect::<Vec<_>>();
    let records = completion
        .records
        .iter()
        .map(|record| {
            json!({
                "memory_id": record.id.0,
                "revision": record.revision,
                "body_id": record.body_id.0,
                "global_version": record.global_version,
                "memory_version": record.memory_version,
            })
        })
        .collect::<Vec<_>>();
    let routing = completion.routing_metadata.iter().map(|route| {
        let mentions = route.entity_mentions.iter().map(|mention| json!({
            "field": mention.field.tag(),
            "start_byte": mention.start_byte,
            "end_byte": mention.end_byte,
            "text": mention.text,
        })).collect::<Vec<_>>();
        json!({ "memory_id": route.memory_id.0, "body_id": route.body_id.0, "mentions": mentions })
    }).collect::<Vec<_>>();
    Ok(json!({
        "episode_id": completion.episode_id.0,
        "attempt": completion.attempt,
        "started_at_ns": completion.started_at_ns,
        "completed_at_ns": completion.completed_at_ns,
        "transaction_time_ns": range.and_then(|(_, _, time)| time),
        "extractor_model": completion.extractor_model,
        "extractor_version": completion.extractor_version,
        "rejected_count": completion.rejected_count,
        "memory_ids": completion.memory_ids.iter().map(|id| id.0).collect::<Vec<_>>(),
        "external_memory_refs": references(&completion.external_memory_refs),
        "global_start": range.map(|(start, _, _)| start),
        "global_count": range.map(|(_, count, _)| count),
        "bodies": bodies,
        "memory_revision_links": records,
        "routing": routing,
        "freshness_birth_turn": completion.freshness_birth_turn,
        "freshness_policy_version": completion.freshness_policy_version,
        "freshness_birth_memory_ids": completion.freshness_birth_memory_ids.iter()
            .map(|id| id.0).collect::<Vec<_>>(),
    }))
}

pub(super) fn observe(owner: &Cva) -> Result<HistoricalFingerprint, String> {
    let uuid = owner
        .owner_uuid()
        .ok_or("Insomnia witness needs owner UUID")?;
    let mut projection = OwnerProjection::new(uuid, "insomnia_import_journal", 0, None)?;
    let mut cursor = owner
        .container
        .physical_cursor_for_source_witness(MAX_FRAME_PAYLOAD_BYTES, MAX_CLOCK_STATE_ENTRIES)
        .map_err(|_| "retained Insomnia journal cursor could not be opened")?;
    let mut sequence = 0_u64;
    // A final InsomniaStore view collapses previous attempts. Retain the
    // ordered, typed accepted records from the same leased read-only owner.
    while let Some(frame) = cursor
        .next_frame()
        .map_err(|_| "retained Insomnia frame is invalid")?
    {
        let payload = &frame.payload;
        let (family, value) = if let Some(record) =
            decode_record(payload).map_err(|_| "retained Insomnia work or attempt is invalid")?
        {
            match record {
                InsomniaRecord::Work(work) => ("work", work_value(work)),
                InsomniaRecord::Attempt(attempt) => ("attempt", attempt_value(attempt)),
            }
        } else if let Some(completion) = decode_completion(payload)? {
            ("completion", completion_value(completion, payload)?)
        } else if decode_format(payload).map_err(|_| "retained Insomnia format is invalid")? {
            ("format", json!({ "schema": "CVAINSF1" }))
        } else {
            continue;
        };
        projection.insert(
            family,
            sequence.to_be_bytes(),
            json!({
                "accepted_journal_ordinal": sequence,
                "accepted_global_frontier": frame.latest_global,
                "record": value,
            }),
        )?;
        sequence += 1;
    }
    for status in owner.import_epochs.statuses() {
        let (start, complete) = match status {
            ImportEpochStatus::Open(start) => (start, None),
            ImportEpochStatus::Completed { start, complete } => (start, Some(complete)),
        };
        projection.insert(
            "import_epoch",
            start.import_id.as_bytes(),
            json!({
                "start": start,
                "completion": complete,
            }),
        )?;
    }
    projection.fingerprint()
}
