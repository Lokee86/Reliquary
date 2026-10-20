//! Strict ordinal Graph carriers; no clock, table or publication authority.
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::{GraphError, GraphRelationOrigin, SemanticGraphRelationKind};
const MUTATION: &[u8; 8] = b"CVAGCM01";
const BATCH: &[u8; 8] = b"CVAGCB01";
const BODY: usize = 12;

pub(crate) fn is_compact_record(bytes: &[u8]) -> Result<bool, GraphError> {
    Ok(decode(bytes)?.is_some())
}

pub(crate) fn encode_mutation(value: GraphOrdinalMutation) -> Vec<u8> {
    let mut out = MUTATION.to_vec();
    write_body(&mut out, value);
    out
}

pub(crate) fn encode_batch(values: &[GraphOrdinalMutation]) -> Result<Vec<u8>, GraphError> {
    let count =
        u32::try_from(values.len()).map_err(|_| corrupt("compact Graph batch too large"))?;
    let mut out = BATCH.to_vec();
    out.extend_from_slice(&count.to_le_bytes());
    for value in values {
        write_body(&mut out, *value);
    }
    Ok(out)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Option<Vec<GraphOrdinalMutation>>, GraphError> {
    if bytes.starts_with(b"CVAGCM") {
        if !bytes.starts_with(MUTATION) || bytes.len() != 8 + BODY {
            return Err(corrupt("unsupported or malformed compact Graph mutation"));
        }
        return Ok(Some(vec![read_body(&bytes[8..])?]));
    }
    if bytes.starts_with(b"CVAGCB") {
        if !bytes.starts_with(BATCH) || bytes.len() < 12 {
            return Err(corrupt("unsupported or malformed compact Graph batch"));
        }
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        if count != (bytes.len() - 12) / BODY || (bytes.len() - 12) % BODY != 0 {
            return Err(corrupt("compact Graph batch count or length"));
        }
        // Empty unmarked batches are representable; the owner rejects an
        // empty published transaction, matching existing V1 admission.
        let mut out = Vec::with_capacity(count);
        for body in bytes[12..].chunks_exact(BODY) {
            out.push(read_body(body)?);
        }
        return Ok(Some(out));
    }
    Ok(None)
}

fn write_body(out: &mut Vec<u8>, value: GraphOrdinalMutation) {
    out.extend_from_slice(&value.source.to_le_bytes());
    out.extend_from_slice(&value.target.to_le_bytes());
    out.extend_from_slice(&value.kind.code().to_le_bytes());
    out.push(u8::from(value.active));
    out.push(value.origin.code());
}
fn read_body(body: &[u8]) -> Result<GraphOrdinalMutation, GraphError> {
    let code = u16::from_le_bytes(body[8..10].try_into().unwrap());
    let kind =
        SemanticGraphRelationKind::from_code(code).ok_or(GraphError::UnknownRelationKind(code))?;
    let active = match body[10] {
        0 => false,
        1 => true,
        _ => return Err(corrupt("compact Graph active flag")),
    };
    let origin =
        GraphRelationOrigin::from_code(body[11]).ok_or_else(|| corrupt("compact Graph origin"))?;
    Ok(GraphOrdinalMutation {
        source: u32::from_le_bytes(body[..4].try_into().unwrap()),
        target: u32::from_le_bytes(body[4..8].try_into().unwrap()),
        kind,
        active,
        origin,
    })
}
fn corrupt(message: &'static str) -> GraphError {
    GraphError::CorruptRecord(message)
}
