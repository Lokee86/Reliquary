//! Current Graph-owned publication bodies; mappings stay with GraphStore.
use crate::GraphError;
use crate::graph_compact_contract::GraphOrdinalMutation;
use reliquary_container::ObjectRef;

const SINGLE: &[u8; 8] = b"CVAGCM02";
const BATCH: &[u8; 8] = b"CVAGCB02";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphTransaction {
    Single(GraphOrdinalMutation),
    Batch(Vec<GraphOrdinalMutation>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphPublication {
    pub graph_version: u64,
    pub transaction: GraphTransaction,
}
impl GraphPublication {
    pub fn mutations(&self) -> &[GraphOrdinalMutation] {
        match &self.transaction {
            GraphTransaction::Single(value) => std::slice::from_ref(value),
            GraphTransaction::Batch(values) => values,
        }
    }
    pub fn is_single(&self) -> bool {
        matches!(self.transaction, GraphTransaction::Single(_))
    }
}

pub fn encode(publication: &GraphPublication) -> Result<Vec<u8>, GraphError> {
    if publication.graph_version == 0 {
        return Err(GraphError::InvalidGraphVersion);
    }
    let mut out = match &publication.transaction {
        GraphTransaction::Single(_) => SINGLE.to_vec(),
        GraphTransaction::Batch(_) => BATCH.to_vec(),
    };
    out.extend_from_slice(&publication.graph_version.to_le_bytes());
    if let GraphTransaction::Batch(values) = &publication.transaction {
        if values.is_empty() {
            return Err(GraphError::InvalidGraphVersion);
        }
        let count = u32::try_from(values.len())
            .map_err(|_| corrupt("Graph publication batch too large"))?;
        out.extend_from_slice(&count.to_le_bytes());
    }
    for value in publication.mutations() {
        super::write_body(&mut out, *value);
    }
    Ok(out)
}

pub fn decode(bytes: &[u8], global_version: u64) -> Result<Option<GraphPublication>, GraphError> {
    let single = if bytes.starts_with(b"CVAGCM") {
        true
    } else if bytes.starts_with(b"CVAGCB") {
        false
    } else if bytes.starts_with(b"CVAG") {
        return Err(corrupt("nonpublication or unsupported Graph body"));
    } else {
        return Ok(None);
    };
    if bytes.get(..8)
        != Some(if single {
            SINGLE.as_slice()
        } else {
            BATCH.as_slice()
        })
        || bytes.len() < 16
    {
        return Err(corrupt("unsupported or truncated Graph publication"));
    }
    let graph_version = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    if graph_version == 0 || global_version == 0 {
        return Err(GraphError::InvalidGraphVersion);
    }
    let transaction = if single {
        if bytes.len() != 16 + super::BODY {
            return Err(corrupt("Graph single publication length"));
        }
        GraphTransaction::Single(super::read_body(&bytes[16..])?)
    } else {
        let count = bytes
            .get(16..20)
            .ok_or(corrupt("short Graph publication count"))?;
        let count = u32::from_le_bytes(count.try_into().unwrap()) as usize;
        let remaining = bytes.len() - 20;
        if count == 0 || count != remaining / super::BODY || remaining % super::BODY != 0 {
            return Err(corrupt("Graph publication batch count or length"));
        }
        let values = bytes[20..]
            .chunks_exact(super::BODY)
            .map(super::read_body)
            .collect::<Result<Vec<_>, _>>()?;
        GraphTransaction::Batch(values)
    };
    Ok(Some(GraphPublication {
        graph_version,
        transaction,
    }))
}

pub fn version_at(
    object: ObjectRef,
    payload: &[u8],
) -> Result<Option<crate::graph_codec::GraphVersionPayload>, GraphError> {
    let Some(envelope) = reliquary_container::publication_envelope::decode(payload)? else {
        return Ok(None);
    };
    let Some(publication) = decode(envelope.body, envelope.global_version)? else {
        return Ok(None);
    };
    Ok(Some(crate::graph_codec::GraphVersionPayload {
        global_version: envelope.global_version,
        graph_version: publication.graph_version,
        mutation: object,
    }))
}

/// Coordinates alone are not semantic admission or ordinal-table authority.
pub fn coordinates(payload: &[u8]) -> Result<Option<(u64, u64)>, GraphError> {
    let Some(envelope) = reliquary_container::publication_envelope::decode(payload)? else {
        return Ok(None);
    };
    Ok(decode(envelope.body, envelope.global_version)?
        .map(|publication| (envelope.global_version, publication.graph_version)))
}

fn corrupt(message: &'static str) -> GraphError {
    GraphError::CorruptRecord(message)
}

#[cfg(test)]
#[path = "graph_publication_codec_tests.rs"]
mod tests;
