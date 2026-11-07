use crate::{
    Failure,
    authentic_values::{Body, GraphChange, Memory, Routing},
    transfer::decimal,
};
use reliquary_container::{Container, ObjectRef};
use reliquary_memory_core::{
    MemoryId, MemoryRecord, encode_retained_memory_body, encode_retained_memory_revision,
    encode_routing_metadata,
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Event {
    Frame {
        source_ordinal: String,
        body_hex: String,
    },
    Allocate {
        source_ordinal: String,
        global_version: String,
        transaction_time_ns: Option<String>,
        body_hex: String,
    },
    BindReserved {
        source_ordinal: String,
        global_version: String,
        body_hex: String,
    },
    Reservation {
        source_ordinal: String,
        global_version: String,
        transaction_time_ns: Option<String>,
    },
    Group {
        source_ordinal: String,
        global_version_start: String,
        global_version_count: String,
        transaction_time_ns: Option<String>,
        body_hex: String,
    },
    MemoryBody {
        source_ordinal: String,
        body: Body,
    },
    MemoryRevision {
        source_ordinal: String,
        record: Memory,
        body: Body,
        bound_reservation: bool,
    },
    MemoryRouting {
        source_ordinal: String,
        routing: Routing,
    },
    Graph {
        source_ordinal: String,
        graph_version: String,
        global_version: String,
        bound_reservation: bool,
        transaction_time_ns: Option<String>,
        changes: Vec<GraphChange>,
    },
    VectorReceipt {
        source_ordinal: String,
        global_version: String,
        vector_version: String,
        generation_source_ordinal: String,
    },
    Embedded {
        source_ordinal: String,
        carrier: String,
    },
}

impl Event {
    pub(super) fn ordinal(&self) -> Result<u64, Failure> {
        let value = match self {
            Self::Frame { source_ordinal, .. }
            | Self::Allocate { source_ordinal, .. }
            | Self::BindReserved { source_ordinal, .. }
            | Self::Reservation { source_ordinal, .. }
            | Self::Group { source_ordinal, .. }
            | Self::MemoryBody { source_ordinal, .. }
            | Self::MemoryRevision { source_ordinal, .. }
            | Self::MemoryRouting { source_ordinal, .. }
            | Self::Graph { source_ordinal, .. }
            | Self::VectorReceipt { source_ordinal, .. }
            | Self::Embedded { source_ordinal, .. } => source_ordinal,
        };
        number(value)
    }
}

pub(super) fn invalid(reason: &'static str) -> Failure {
    (4, reason)
}

pub(super) fn number<T: std::str::FromStr + ToString>(value: &str) -> Result<T, Failure> {
    decimal(value).map_err(invalid)
}

fn time(value: Option<String>) -> Result<Option<i64>, Failure> {
    value.as_deref().map(number).transpose()
}

fn payload(value: &str) -> Result<Vec<u8>, Failure> {
    if value.len() < 16
        || !value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
    {
        return Err(invalid("invalid canonical native body hex"));
    }
    let data = hex::decode(value).map_err(|_| invalid("invalid native body hex"))?;
    if data.starts_with(b"RELVERS")
        || data.starts_with(b"RELPUB")
        || !(data.starts_with(b"REL") || data.starts_with(b"CVDREAM"))
    {
        return Err(invalid("target transfer requires an owner-native body"));
    }
    Ok(data)
}

fn archive_publication_body(value: &str) -> Result<Vec<u8>, Failure> {
    let body = payload(value)?;
    if !body.starts_with(b"RELAPB01") {
        return Err(invalid(
            "raw clocked transfer requires a native Archive publication",
        ));
    }
    Ok(body)
}

fn admitted_plain_body(bytes: &[u8]) -> bool {
    matches!(
        &bytes[..8],
        b"RELMFMT1"
            | b"RELDRM01"
            | b"RELDRP02"
            | b"CVDREAM1"
            | b"CVDREAM2"
            | b"RELPRC01"
            | b"RELFILE1"
            | b"RELFRP01"
            | b"RELFRS02"
            | b"RELHFRS1"
            | b"RELINSW1"
            | b"RELINSW2"
            | b"RELINSR1"
            | b"RELCOM02"
            | b"RELPVFM1"
            | b"RELPVEC1"
            | b"RELCPFM1"
            | b"RELCPRO1"
            | b"RELCPRT1"
            | b"RELMVFM1"
            | b"RELMVEC1"
            | b"RELAVFM1"
            | b"RELAVEC1"
            | b"RELVGFM1"
            | b"RELVGEN1"
    )
}

fn append_publication(
    output: &mut Container,
    global: u64,
    bound: bool,
    transaction_time: Option<i64>,
    body: &[u8],
) -> Result<(), Failure> {
    if bound {
        if transaction_time.is_some() && output.transaction_time_ns(global) != transaction_time {
            return Err(invalid("publication time differs from its reservation"));
        }
        output
            .append_bound_publication_envelope(global, body)
            .map_err(|_| invalid("native bound publication could not be written"))?;
    } else {
        if output.latest_version().checked_add(1) != Some(global) {
            return Err(invalid(
                "native allocation differs from source global order",
            ));
        }
        output
            .append_publication_envelope(body, transaction_time)
            .map_err(|_| invalid("native allocated publication could not be written"))?;
    }
    Ok(())
}

pub(super) fn append(
    event: Event,
    output: &mut Container,
    memories: &mut BTreeMap<MemoryId, MemoryRecord>,
    generations: &mut BTreeMap<u64, ObjectRef>,
) -> Result<bool, Failure> {
    match event {
        Event::Frame {
            source_ordinal,
            body_hex,
        } => {
            let bytes = payload(&body_hex)?;
            if !admitted_plain_body(&bytes) {
                return Err(invalid("unclassified plain native carrier"));
            }
            let target = output
                .append(&bytes)
                .map_err(|_| invalid("native frame could not be written"))?;
            if bytes.starts_with(b"RELVGEN1")
                && generations
                    .insert(number(&source_ordinal)?, target)
                    .is_some()
            {
                return Err(invalid("duplicate relocated vector generation"));
            }
        }
        Event::Allocate {
            global_version,
            transaction_time_ns,
            body_hex,
            ..
        } => {
            append_publication(
                output,
                number(&global_version)?,
                false,
                time(transaction_time_ns)?,
                &archive_publication_body(&body_hex)?,
            )?;
        }
        Event::BindReserved {
            global_version,
            body_hex,
            ..
        } => {
            append_publication(
                output,
                number(&global_version)?,
                true,
                None,
                &archive_publication_body(&body_hex)?,
            )?;
        }
        Event::Reservation {
            global_version,
            transaction_time_ns,
            ..
        } => {
            if output.latest_version().checked_add(1) != Some(number(&global_version)?) {
                return Err(invalid(
                    "native reservation differs from source global order",
                ));
            }
            output
                .reserve_version(time(transaction_time_ns)?)
                .map_err(|_| invalid("native reservation could not be written"))?;
        }
        Event::Group {
            global_version_start,
            global_version_count,
            transaction_time_ns,
            body_hex,
            ..
        } => {
            let start = number(&global_version_start)?;
            let count = number(&global_version_count)?;
            let transaction_time = time(transaction_time_ns)?;
            let bytes = payload(&body_hex)?;
            if !bytes.starts_with(b"RELINSC7") && !bytes.starts_with(b"RELINSC8") {
                return Err(invalid("unsupported grouped conversion carrier"));
            }
            output
                .append(&bytes)
                .map_err(|_| invalid("native group could not be written"))?;
            output
                .commit_embedded_version_range_at(start, count, transaction_time)
                .map_err(|_| invalid("native grouped range differs from source global order"))?;
        }
        Event::MemoryBody { body, .. } => {
            let (id, body) = body.into_native().map_err(invalid)?;
            let bytes = encode_retained_memory_body(id, &body)
                .map_err(|_| invalid("invalid retained Memory body"))?;
            output
                .append(&bytes)
                .map_err(|_| invalid("native Memory body could not be written"))?;
        }
        Event::MemoryRevision {
            record,
            body,
            bound_reservation,
            ..
        } => {
            let global = record.global_version;
            let stamp = record.transaction_time_ns;
            let record = record.into_native().map_err(invalid)?;
            let (body_id, body) = body.into_native().map_err(invalid)?;
            if record.body_id != body_id {
                return Err(invalid("retained Memory revision body differs"));
            }
            let bytes = encode_retained_memory_revision(memories.get(&record.id), &record, &body)
                .map_err(|_| invalid("invalid retained Memory revision"))?;
            append_publication(output, global, bound_reservation, stamp, &bytes)?;
            memories.insert(record.id, record);
        }
        Event::MemoryRouting { routing, .. } => {
            let bytes = encode_routing_metadata(&routing.into_native().map_err(invalid)?)
                .map_err(|_| invalid("invalid retained Memory routing"))?;
            output
                .append(&bytes)
                .map_err(|_| invalid("native Memory routing could not be written"))?;
        }
        Event::Graph {
            graph_version,
            global_version,
            bound_reservation,
            transaction_time_ns,
            changes,
            ..
        } => {
            let changes = changes
                .into_iter()
                .map(GraphChange::into_native)
                .collect::<Result<Vec<_>, _>>()
                .map_err(invalid)?;
            let bytes = reliquary_graph::encode_clocked_graph_body(
                output.owner_id(),
                number(&graph_version)?,
                &changes,
            )
            .map_err(|_| invalid("invalid retained Graph publication"))?;
            append_publication(
                output,
                number(&global_version)?,
                bound_reservation,
                time(transaction_time_ns)?,
                &bytes,
            )?;
        }
        Event::VectorReceipt {
            source_ordinal,
            global_version,
            vector_version,
            generation_source_ordinal,
        } => {
            let generation = number(&generation_source_ordinal)?;
            if generation >= number(&source_ordinal)? {
                return Err(invalid(
                    "vector generation receipt requires an earlier source object",
                ));
            }
            let relocated = generations
                .get(&generation)
                .ok_or_else(|| invalid("vector generation receipt backing is unrelocated"))?;
            let body = reliquary_vector_store::generation::encode_generation_receipt(
                number(&vector_version)?,
                *relocated,
            );
            append_publication(output, number(&global_version)?, true, None, &body)?;
        }
        Event::Embedded { carrier, .. } => {
            if ![
                "CVAGFMT1", "CVAGNODE", "CVAGNOD2", "CVAAFMT2", "CVACONT1", "PRJFILE1", "CVAENTF1",
                "CVAINSF1", "CVAAVFM1", "CVAVGFM2",
            ]
            .contains(&carrier.as_str())
            {
                return Err(invalid("unreviewed embedded source carrier"));
            }
            return Ok(false);
        }
    }
    Ok(true)
}
