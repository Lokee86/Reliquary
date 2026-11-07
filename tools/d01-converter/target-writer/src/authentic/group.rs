use crate::{
    Failure,
    authentic_values::{Body, Memory, Routing},
    transfer::decode_hex,
};
use reliquary_container::Container;
use reliquary_memory_core::{
    MemoryId, MemoryRecord, encode_retained_memory_revision, encode_routing_metadata,
};
use reliquary_owner_session::records::insomnia_completion::{
    GroupedExternalMemoryRef, GroupedInsomniaCompletion, GroupedMemoryBody,
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Group {
    layout: u8,
    episode_id: String,
    attempt: u32,
    started_at_ns: i64,
    completed_at_ns: i64,
    transaction_time_ns: Option<i64>,
    extractor_model: String,
    extractor_version: String,
    rejected_count: u32,
    global_version_start: u64,
    global_version_count: usize,
    memory_ids: Vec<String>,
    external_memory_refs: Vec<External>,
    freshness_birth: Option<Birth>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct External {
    owner_id: String,
    memory_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Birth {
    activity: u64,
    policy_version: u64,
    memory_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Revision {
    record: Memory,
    body: Body,
}

fn invalid(reason: &'static str) -> Failure {
    (4, reason)
}

fn ids(list: Vec<String>) -> Result<Vec<[u8; 32]>, Failure> {
    list.iter()
        .map(|value| decode_hex(value).map_err(invalid))
        .collect()
}

pub(super) fn append(
    output: &mut Container,
    memory_state: &mut BTreeMap<MemoryId, MemoryRecord>,
    group: Group,
    bodies: Vec<Body>,
    revisions: Vec<Revision>,
    routing: Vec<Routing>,
) -> Result<(), Failure> {
    if group.global_version_count != revisions.len()
        || (group.global_version_count > 0
            && output.latest_version().checked_add(1) != Some(group.global_version_start))
    {
        return Err(invalid("retained grouped Memory range differs"));
    }
    let mut encoded = Vec::with_capacity(revisions.len());
    let mut postimages = Vec::with_capacity(revisions.len());
    for (index, Revision { record, body }) in revisions.into_iter().enumerate() {
        if group.global_version_start.checked_add(index as u64) != Some(record.global_version)
            || group.transaction_time_ns != record.transaction_time_ns
        {
            return Err(invalid("retained grouped Memory binding differs"));
        }
        let record = record.into_native().map_err(invalid)?;
        let (body_id, body) = body.into_native().map_err(invalid)?;
        if record.body_id != body_id || memory_state.contains_key(&record.id) {
            return Err(invalid(
                "retained grouped Memory is not a first publication",
            ));
        }
        encoded.push(
            encode_retained_memory_revision(None, &record, &body)
                .map_err(|_| invalid("invalid grouped retained Memory revision"))?,
        );
        postimages.push(record);
    }
    let bodies = bodies
        .into_iter()
        .map(|body| {
            let (id, body) = body.into_native().map_err(invalid)?;
            let native = GroupedMemoryBody::from_content(&body.title, &body.content);
            if native.id != id {
                return Err(invalid("retained grouped Memory body ID differs"));
            }
            Ok(native)
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    let routing_metadata = routing
        .into_iter()
        .map(|route| {
            encode_routing_metadata(&route.into_native().map_err(invalid)?)
                .map_err(|_| invalid("invalid grouped retained routing"))
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    let external_memory_refs = group
        .external_memory_refs
        .into_iter()
        .map(|reference| {
            Ok(GroupedExternalMemoryRef {
                owner_id: reference.owner_id,
                memory_id: decode_hex(&reference.memory_id).map_err(invalid)?,
            })
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    let (freshness_birth_turn, freshness_policy_version, freshness_birth_memory_ids) =
        match group.freshness_birth {
            Some(birth) => (
                Some(birth.activity),
                Some(birth.policy_version),
                ids(birth.memory_ids)?,
            ),
            None => (None, None, Vec::new()),
        };
    let completion = GroupedInsomniaCompletion {
        episode_id: decode_hex(&group.episode_id).map_err(invalid)?,
        attempt: group.attempt,
        started_at_ns: group.started_at_ns,
        completed_at_ns: group.completed_at_ns,
        transaction_time_ns: group.transaction_time_ns.unwrap_or_default(),
        extractor_model: group.extractor_model,
        extractor_version: group.extractor_version,
        rejected_count: group.rejected_count,
        memory_ids: ids(group.memory_ids)?,
        external_memory_refs,
        global_version_start: group.global_version_start,
        bodies,
        memory_records: encoded,
        routing_metadata,
        freshness_birth_turn,
        freshness_policy_version,
        freshness_birth_memory_ids,
    };
    let bytes = completion
        .encode_retained(group.layout, group.transaction_time_ns)
        .map_err(|error| {
            eprintln!("retained completion admission: {error}");
            invalid("invalid retained grouped Insomnia value")
        })?;
    output
        .append(&bytes)
        .map_err(|_| invalid("could not write retained grouped Insomnia value"))?;
    output
        .commit_embedded_version_range_at(
            group.global_version_start,
            group.global_version_count,
            group.transaction_time_ns,
        )
        .map_err(|_| invalid("could not retain grouped Insomnia range"))?;
    for record in postimages {
        memory_state.insert(record.id, record);
    }
    Ok(())
}
